//! `mapd`: the map's local server. Serves the app, keeps the open world on disk, runs the
//! generator natively, and lets agents read and edit the world over MCP (`/mcp`), with every
//! change pushed live to the open app (`/ws`). Listens on 127.0.0.1 only.
//!
//! `cargo run --release -p mapd -- [--port 7777] [--dir worlds] [--app app/dist]`
//! Claude Code: `claude mcp add --transport http worldspring http://127.0.0.1:7777/mcp`

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
use std::sync::atomic::{AtomicU64, Ordering};
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
    /// Messages to every connected app (JSON text).
    pub to_apps: broadcast::Sender<String>,
    /// Requests waiting on an app's answer (screenshots).
    pub pending: Mutex<HashMap<u64, oneshot::Sender<Value>>>,
    pub next_id: AtomicU64,
}

pub type Shared = Arc<AppState>;

impl AppState {
    fn next(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Change the open world's edits: `f` edits a copy (and says what it did); the result is
    /// saved, logged, given to the generator and sent, as the ops it made, to every app (except
    /// `from`).
    pub async fn edit(&self, author: &str, from: u64, f: impl FnOnce(&mut Edits) -> Result<Value, String>) -> Result<Value, String> {
        let (hash, edits, ops, change) = {
            let mut w = self.world.lock().unwrap();
            let Some((hash, file)) = w.as_mut() else { return Err("no world is open: open the map app first".into()) };
            let mut edits = file.edits.clone();
            let change = f(&mut edits)?;
            let ops = file.edits.diff(&edits);
            if ops.is_empty() {
                return Ok(change);
            }
            file.edits = edits.clone();
            self.store.save(*hash, file);
            (*hash, edits, ops, change)
        };
        self.store.log(hash, json!({ "time": now_secs(), "author": author, "change": change, "ops": ops }));
        self.worker.run(move |g| g.set_edits(edits)).await;
        let _ = self.to_apps.send(json!({ "type": "ops", "ops": ops, "change": change, "author": author, "from": from }).to_string());
        Ok(change)
    }

    /// Ask the open app for something (a screenshot) and wait for its answer.
    pub async fn ask_app(&self, mut msg: Value, timeout_s: u64) -> Result<Value, String> {
        if self.to_apps.receiver_count() == 0 {
            return Err("the map app is not open: open it (it connects to mapd) and try again".into());
        }
        let id = self.next();
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id, tx);
        msg["id"] = json!(id);
        let _ = self.to_apps.send(msg.to_string());
        match tokio::time::timeout(std::time::Duration::from_secs(timeout_s), rx).await {
            Ok(Ok(v)) => Ok(v),
            _ => {
                self.pending.lock().unwrap().remove(&id);
                Err("the map app did not answer in time".into())
            }
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

/// One connected app: it says which world it has open; edits flow both ways; it answers
/// screenshot requests.
async fn client(app: Shared, mut socket: WebSocket) {
    let me = app.next();
    let mut rx = app.to_apps.subscribe();
    let _ = socket.send(Message::Text(json!({ "type": "welcome", "client": me }).to_string().into())).await;
    loop {
        tokio::select! {
            out = rx.recv() => match out {
                Ok(text) => {
                    // Not echoed back to the app the change came from.
                    let from = serde_json::from_str::<Value>(&text).ok().and_then(|v| v["from"].as_u64());
                    if from != Some(me) && socket.send(Message::Text(text.into())).await.is_err() {
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
}

async fn on_app_message(app: &Shared, me: u64, m: Value) -> Option<Value> {
    match m["type"].as_str()? {
        // The app's open world: mapd's copy of the same world keeps its edits (agents may
        // have changed it meanwhile); a world mapd hasn't seen is taken as it is.
        "hello" => {
            let file: WorldFile = serde_json::from_value(m["world"].clone()).ok()?;
            let world = World::new(file.clone()).ok()?;
            let hash = world.hash;
            let stored = app.store.load(hash);
            let open = stored.unwrap_or(file);
            app.store.save(hash, &open);
            *app.world.lock().unwrap() = Some((hash, open.clone()));
            let f = open.clone();
            let loaded = app.worker.run(move |g| g.load(f)).await;
            if let Err(e) = loaded {
                return Some(json!({ "type": "error", "message": e }));
            }
            Some(json!({ "type": "world", "world": open }))
        }
        // The user changed something in the app: its ops apply to mapd's copy, so whatever
        // agents changed meanwhile stays.
        "ops" => {
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
