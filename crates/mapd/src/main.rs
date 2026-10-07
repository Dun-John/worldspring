//! `mapd`: the map's local server. Serves the app, keeps the open world on disk, runs the
//! generator natively, and lets agents read and edit the world over MCP (`/mcp`), with every
//! change pushed live to the open app (`/ws`). Listens on 127.0.0.1 only.
//!
//! `cargo run --release -p mapd -- [--port 7777] [--dir worlds] [--app app/dist]`
//! Any MCP client, e.g. `claude mcp add --transport http worldspring http://127.0.0.1:7777/mcp`

mod batch;
mod build;
mod crossings;
mod design;
mod mcp;
mod notebook;
mod scatter;
mod store;
mod tools;
mod worker;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::{HeaderMap, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde_json::{Value, json};
use tokio::sync::{broadcast, oneshot};
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use worldgen::world::{EditOp, Edits};
use worldgen::{World, WorldFile};

use crate::store::Store;
use crate::worker::GenHandle;

pub struct AppState {
    pub worker: GenHandle,
    pub store: Store,
    /// The open world file (with its edits) and its hash.
    pub world: Mutex<Option<(u64, WorldFile)>>,
    /// Messages to connected apps (JSON text); `to` (a client), `world` (only apps showing it)
    /// and `from` (not back to it) pick who gets one.
    pub to_apps: broadcast::Sender<String>,
    /// Requests waiting on an app's answer (screenshots).
    pub pending: Mutex<HashMap<u64, oneshot::Sender<Value>>>,
    pub next_id: AtomicU64,
    /// The world each connected app (each browser tab) shows, by client id.
    pub tabs: Mutex<HashMap<u64, Option<u64>>>,
    /// One change to the open world at a time: a batch holds it for all its steps, and
    /// switching worlds waits for it.
    pub gate: tokio::sync::Mutex<()>,
    /// A save failed: the open world has changes only in memory (saved again shortly).
    pub unsaved: AtomicBool,
}

pub type Shared = Arc<AppState>;

/// A world hash as agents and apps see it.
pub fn hex(hash: u64) -> String {
    format!("{hash:016x}")
}

impl AppState {
    fn next(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    pub fn send(&self, msg: Value) {
        let _ = self.to_apps.send(msg.to_string());
    }

    /// The open world's hash.
    pub fn current(&self) -> Option<u64> {
        self.world.lock().unwrap().as_ref().map(|(h, _)| *h)
    }

    /// The tabs showing the open world.
    pub fn followers(&self) -> Vec<u64> {
        let cur = self.current();
        self.tabs.lock().unwrap().iter().filter(|(_, w)| cur.is_some() && **w == cur).map(|(id, _)| *id).collect()
    }

    /// If no tab shows the open world, ask the newest tab showing another to say hello again
    /// (mapd then follows it).
    fn handoff(&self) {
        if !self.followers().is_empty() {
            return;
        }
        let next = self.tabs.lock().unwrap().iter().filter(|(_, w)| w.is_some()).map(|(id, _)| *id).max();
        if let Some(next) = next {
            self.send(json!({ "type": "resync", "to": next }));
        }
    }

    /// Tell every tab whether it shows the open world (its changes reach mapd only then).
    fn announce(&self) {
        let cur = self.current();
        for (id, w) in self.tabs.lock().unwrap().iter() {
            if w.is_some() {
                self.send(json!({ "type": "follow", "to": id, "following": cur.is_some() && *w == cur }));
            }
        }
    }

    /// The open world's edits, read as they are now (inside a batch, as the batch has them).
    pub fn with_edits<R>(&self, f: impl FnOnce(&Edits) -> R) -> Option<R> {
        if batch::active() {
            return batch::with_staged(f);
        }
        self.world.lock().unwrap().as_ref().map(|(_, file)| f(&file.edits))
    }

    /// Change the open world's edits: `f` edits a copy (and says what it did); the result is
    /// saved, logged, given to the generator and sent, as the ops it made, to every app showing
    /// the world (except `from`). Inside a batch only the batch's copy changes.
    pub async fn edit(&self, author: &str, from: u64, f: impl FnOnce(&mut Edits) -> Result<Value, String>) -> Result<Value, String> {
        if batch::active() {
            return batch::stage(self, f).await;
        }
        let _one = self.gate.lock().await;
        self.edit_now(author, from, f).await
    }

    /// `edit`, with the gate already held.
    pub async fn edit_now(&self, author: &str, from: u64, f: impl FnOnce(&mut Edits) -> Result<Value, String>) -> Result<Value, String> {
        let (hash, edits, ops, change, text) = {
            let mut w = self.world.lock().unwrap();
            let Some((hash, file)) = w.as_mut() else { return Err("no world is open: open the map app first".into()) };
            // (The world may have changed while the call waited for the gate.)
            if let Some(want) = tools::wanted()
                && want != *hash
            {
                return Err(tools::other_world(Some(*hash), want));
            }
            let mut edits = file.edits.clone();
            let change = f(&mut edits)?;
            let ops = file.edits.diff(&edits);
            if ops.is_empty() {
                return Ok(change);
            }
            file.edits = edits.clone();
            (*hash, edits, ops, change, Store::text(file))
        };
        let saved = text.and_then(|t| self.store.write(hash, &t));
        self.store.log(hash, json!({ "time": now_secs(), "author": author, "change": change, "ops": ops }));
        self.worker.run(move |g| g.set_edits(edits)).await;
        self.send(json!({ "type": "ops", "ops": ops, "change": change, "author": author, "from": from, "world": hex(hash) }));
        match saved {
            Ok(()) => {
                if self.unsaved.swap(false, Ordering::Relaxed) {
                    self.send(json!({ "type": "saved" }));
                }
                Ok(change)
            }
            Err(e) => {
                self.unsaved.store(true, Ordering::Relaxed);
                eprintln!("mapd: could not save: {e}");
                self.send(json!({ "type": "error", "message": format!("could not save to disk: {e}"), "world": hex(hash), "from": from }));
                Err(format!("the change was made, but could not be saved to disk: {e}. mapd keeps it and tries again every few seconds"))
            }
        }
    }

    /// Save the open world again after a failed save (the gate held). True once it is saved.
    fn save_now(&self) -> bool {
        if !self.unsaved.load(Ordering::Relaxed) {
            return true;
        }
        let Some((hash, text)) = self.world.lock().unwrap().as_ref().map(|(h, f)| (*h, Store::text(f))) else { return true };
        match text.and_then(|t| self.store.write(hash, &t)) {
            Ok(()) => {
                self.unsaved.store(false, Ordering::Relaxed);
                println!("mapd: saved the changes kept in memory");
                // (To every tab: one that tried to switch to its own world meanwhile shows the notice too.)
                self.send(json!({ "type": "saved" }));
                true
            }
            Err(e) => {
                eprintln!("mapd: still cannot save: {e}");
                false
            }
        }
    }

    /// Ask an app showing the open world for something (a screenshot) and wait for its answer.
    pub async fn ask_app(&self, mut msg: Value, timeout_s: u64) -> Result<Value, String> {
        let Some(tab) = self.followers().into_iter().max() else { return Err(self.no_app()) };
        let id = self.next();
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id, tx);
        msg["id"] = json!(id);
        msg["to"] = json!(tab);
        self.send(msg);
        match tokio::time::timeout(std::time::Duration::from_secs(timeout_s), rx).await {
            Ok(Ok(v)) => Ok(v),
            _ => {
                self.pending.lock().unwrap().remove(&id);
                Err("the map app did not answer in time".into())
            }
        }
    }

    /// Why no app can show the open world.
    pub fn no_app(&self) -> String {
        if self.tabs.lock().unwrap().is_empty() {
            "the map app is not open: open it (it connects to mapd) and try again".into()
        } else {
            "no open map app shows mapd's world (the open tabs show other worlds): open that world in a tab, or choose \"Follow this tab\" in one".into()
        }
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Only pages served from this machine may talk to mapd (a web page elsewhere could
/// otherwise reach 127.0.0.1 through DNS rebinding). No Origin (curl, MCP clients) is fine.
pub fn origin_ok(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get("origin").and_then(|o| o.to_str().ok()) else { return true };
    let host = origin.split("://").nth(1).unwrap_or("").split(':').next().unwrap_or("");
    matches!(host, "localhost" | "127.0.0.1" | "[::1]")
}

async fn ws(State(app): State<Shared>, headers: HeaderMap, up: WebSocketUpgrade) -> Response {
    if !origin_ok(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    up.on_upgrade(move |socket| client(app, socket))
}

/// One connected app: it says which world it has open; edits flow both ways while that is
/// mapd's world; it answers screenshot requests.
async fn client(app: Shared, mut socket: WebSocket) {
    let me = app.next();
    let mut rx = app.to_apps.subscribe();
    app.tabs.lock().unwrap().insert(me, None);
    let _ = socket.send(Message::Text(json!({ "type": "welcome", "client": me }).to_string().into())).await;
    loop {
        tokio::select! {
            out = rx.recv() => match out {
                Ok(text) => {
                    if for_me(&app, me, &text) && socket.send(Message::Text(text.into())).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => break,
            },
            inc = socket.recv() => match inc {
                Some(Ok(Message::Text(text))) => {
                    if let Ok(m) = serde_json::from_str::<Value>(text.as_str()) {
                        if let Some(reply) = on_app_message(&app, me, m).await {
                            if socket.send(Message::Text(reply.to_string().into())).await.is_err() {
                                break;
                            }
                        }
                    }
                }
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                _ => {}
            },
        }
    }
    left(&app, me);
}

/// Whether a message to apps goes to this one: not back where it came from, only to the
/// client named, only to apps showing the world named.
fn for_me(app: &Shared, me: u64, text: &str) -> bool {
    let Ok(v) = serde_json::from_str::<Value>(text) else { return true };
    if v["from"].as_u64() == Some(me) || v["to"].as_u64().is_some_and(|to| to != me) {
        return false;
    }
    match v["world"].as_str() {
        Some(w) => app.tabs.lock().unwrap().get(&me).copied().flatten().is_some_and(|h| hex(h) == w),
        None => true,
    }
}

/// How long mapd waits for a tab showing its world to come back (a reload) before following
/// another tab's world.
const HANDOFF_WAIT: std::time::Duration = std::time::Duration::from_secs(20);

/// A tab closed. If no tab shows mapd's world any more and none comes back for a while, the
/// newest other one is asked to say hello again, and mapd follows it.
fn left(app: &Shared, me: u64) {
    app.tabs.lock().unwrap().remove(&me);
    if !app.followers().is_empty() {
        return;
    }
    let (app, cur) = (app.clone(), app.current());
    tokio::spawn(async move {
        tokio::time::sleep(HANDOFF_WAIT).await;
        if app.current() == cur {
            app.handoff();
        }
    });
}

async fn on_app_message(app: &Shared, me: u64, m: Value) -> Option<Value> {
    match m["type"].as_str()? {
        // The app's open world. mapd follows it if it has it open already, if no other tab
        // shows mapd's world, or if the user asks (`take`: "Follow this tab"); else the tab is
        // told it isn't followed. mapd's copy of a world keeps its edits (agents may have
        // changed it meanwhile); a world mapd hasn't seen is taken as it is.
        "hello" => {
            let file: WorldFile = serde_json::from_value(m["world"].clone()).ok()?;
            let world = World::new(file.clone()).ok()?;
            let hash = world.hash;
            let take = m["take"].as_bool().unwrap_or(false);
            let _one = app.gate.lock().await;
            let cur = app.current();
            let others = app.followers().into_iter().any(|id| id != me);
            app.tabs.lock().unwrap().insert(me, Some(hash));
            if cur != Some(hash) {
                if others && !take {
                    app.announce();
                    return None;
                }
                // (Changes kept in memory are saved before the world they belong to is left.)
                if !app.save_now() {
                    app.announce();
                    return Some(json!({ "type": "error", "message": "live sync cannot switch worlds yet: the open world's changes could not be saved to disk" }));
                }
            }
            let mine = app.world.lock().unwrap().as_ref().filter(|(h, _)| *h == hash).map(|(_, f)| f.clone());
            let switched = mine.is_none();
            let open = mine.or_else(|| app.store.load(hash)).unwrap_or(file);
            let saved = if switched { app.store.save(hash, &open) } else { Ok(()) };
            *app.world.lock().unwrap() = Some((hash, open.clone()));
            let f = open.clone();
            let loaded = app.worker.run(move |g| g.load(f)).await;
            app.announce();
            if let Err(e) = loaded {
                return Some(json!({ "type": "error", "message": e }));
            }
            if let Err(e) = saved {
                eprintln!("mapd: could not save: {e}");
                app.unsaved.store(true, Ordering::Relaxed);
                app.send(json!({ "type": "error", "message": format!("could not save to disk: {e}"), "to": me }));
            }
            Some(json!({ "type": "world", "world": open }))
        }
        // The user changed something in the app: its ops apply to mapd's copy, so whatever
        // agents changed meanwhile stays. (Only from a tab showing mapd's world.)
        "ops" => {
            if !app.followers().contains(&me) {
                // (The app keeps them for when mapd follows it again.)
                return Some(json!({ "type": "follow", "following": false, "refused": m["ops"] }));
            }
            let ops: Vec<EditOp> = serde_json::from_value(m["ops"].clone()).ok()?;
            let change = m["change"].clone();
            let done = app
                .edit("user", me, move |e| {
                    for op in &ops {
                        e.apply(op)?;
                    }
                    Ok(change)
                })
                .await;
            done.err().map(|e| json!({ "type": "error", "message": e }))
        }
        // Whole edits (older apps).
        "edits" => {
            if !app.followers().contains(&me) {
                return Some(json!({ "type": "follow", "following": false }));
            }
            let edits: Edits = serde_json::from_value(m["edits"].clone()).ok()?;
            let change = m["change"].clone();
            let _ = app
                .edit("user", me, move |e| {
                    *e = edits;
                    Ok(change)
                })
                .await;
            None
        }
        "answer" => {
            let id = m["id"].as_u64()?;
            if let Some(tx) = app.pending.lock().unwrap().remove(&id) {
                let _ = tx.send(m);
            }
            None
        }
        _ => None,
    }
}

/// The image type of an asset's bytes (pictures only: png, jpeg, webp, gif, svg).
pub fn image_type(b: &[u8]) -> Option<&'static str> {
    let head = String::from_utf8_lossy(&b[..b.len().min(256)]).to_ascii_lowercase();
    Some(match b {
        [0x89, b'P', b'N', b'G', ..] => "image/png",
        [0xff, 0xd8, 0xff, ..] => "image/jpeg",
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => "image/webp",
        [b'G', b'I', b'F', b'8', ..] => "image/gif",
        _ if head.contains("<svg") || head.starts_with("<?xml") => "image/svg+xml",
        _ => return None,
    })
}

async fn asset_get(State(app): State<Shared>, Path(id): Path<String>) -> Response {
    match app.store.asset(&id) {
        Some(bytes) => {
            let kind = image_type(&bytes).unwrap_or("application/octet-stream");
            ([(header::CONTENT_TYPE, kind), (header::CACHE_CONTROL, "public, max-age=31536000, immutable")], bytes).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn asset_put(State(app): State<Shared>, headers: HeaderMap, Path(id): Path<String>, body: Bytes) -> Response {
    if !origin_ok(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if image_type(&body).is_none() {
        return (StatusCode::UNSUPPORTED_MEDIA_TYPE, "assets are pictures: png, jpeg, webp, gif or svg").into_response();
    }
    match app.store.put_asset(&id, &body) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e).into_response(),
    }
}

async fn mcp_post(State(app): State<Shared>, headers: HeaderMap, body: String) -> Response {
    if !origin_ok(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    mcp::handle(&app, &headers, &body).await
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let port: u16 = arg("--port").and_then(|p| p.parse().ok()).unwrap_or(7777);
    let dir = PathBuf::from(arg("--dir").unwrap_or_else(|| "worlds".into()));
    let dist = PathBuf::from(arg("--app").unwrap_or_else(|| "app/dist".into()));

    let (to_apps, _) = broadcast::channel(256);
    let app: Shared = Arc::new(AppState {
        worker: GenHandle::spawn(),
        store: Store::new(dir),
        world: Mutex::new(None),
        to_apps,
        pending: Mutex::new(HashMap::new()),
        next_id: AtomicU64::new(1),
        tabs: Mutex::new(HashMap::new()),
        gate: tokio::sync::Mutex::new(()),
        unsaved: AtomicBool::new(false),
    });
    // After a failed save, try again every few seconds (the changes are kept in memory).
    let retry = app.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            if retry.unsaved.load(Ordering::Relaxed) {
                let _one = retry.gate.lock().await;
                // (A tab may have been waiting for the save to switch worlds.)
                if retry.save_now() {
                    retry.handoff();
                }
            }
        }
    });
    // Reopen the last world (generating it takes a few seconds, in the background).
    if let Some(file) = app.store.current()
        && let Ok(w) = World::new(file.clone())
    {
        *app.world.lock().unwrap() = Some((w.hash, file.clone()));
        let worker = app.worker.clone();
        tokio::spawn(async move {
            if let Err(e) = worker.run(move |g| g.load(file)).await {
                eprintln!("mapd: could not reopen the last world: {e}");
            }
        });
    }

    let index = dist.join("index.html");
    // Assets are fetched and uploaded by the app, which the dev server serves from another port.
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(|o, _| {
            let mut h = HeaderMap::new();
            h.insert("origin", o.clone());
            origin_ok(&h)
        }))
        .allow_methods([Method::GET, Method::PUT, Method::HEAD]);
    let assets = Router::new().route("/assets/{id}", get(asset_get).head(asset_get).put(asset_put)).layer(DefaultBodyLimit::max(16 << 20)).layer(cors);
    let mut router = Router::new().route("/mcp", post(mcp_post).get(|| async { StatusCode::METHOD_NOT_ALLOWED })).route("/ws", get(ws)).merge(assets);
    if index.exists() {
        router = router.fallback_service(ServeDir::new(&dist).fallback(ServeFile::new(index)));
    } else {
        router = router.fallback(|| async { (StatusCode::NOT_FOUND, "mapd: the app is not built (npm run build), or use the dev server (npm run dev)") });
    }
    let router = router.with_state(app);
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap_or_else(|e| panic!("mapd: cannot listen on {addr}: {e}"));
    println!("mapd: http://{addr} (MCP at http://{addr}/mcp)");
    axum::serve(listener, router).await.expect("server");
}
