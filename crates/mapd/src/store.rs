//! Worlds on disk: `<dir>/<world hash>/world.json` (the world file with its edits) and
//! `edits.jsonl` (every change: when, who, what), `<dir>/current` (the open world's hash), and
//! `<dir>/assets/<id>`: uploaded pictures (sprites, portraits) by content hash, shared by all
//! worlds.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;
use worldgen::WorldFile;

pub struct Store {
    dir: PathBuf,
}

impl Store {
    pub fn new(dir: PathBuf) -> Store {
        let _ = std::fs::create_dir_all(&dir);
        Store { dir }
    }

    fn world_dir(&self, hash: u64) -> PathBuf {
        self.dir.join(format!("{hash:016x}"))
    }

    pub fn load(&self, hash: u64) -> Option<WorldFile> {
        let d = self.world_dir(hash);
        recover(&d);
        let text = std::fs::read_to_string(d.join("world.json")).ok()?;
        serde_json::from_str(&text).ok()
    }

    /// Write the world file: to a `.tmp` first, then moved over `world.json`, so a crash never
    /// leaves half a file. An error says what could not be written (the copy in memory stands).
    pub fn save(&self, hash: u64, file: &WorldFile) -> Result<(), String> {
        self.write(hash, &Self::text(file)?)
    }

    /// A world file as saved.
    pub fn text(file: &WorldFile) -> Result<String, String> {
        serde_json::to_string_pretty(file).map_err(|e| e.to_string())
    }

    /// Save a world file's text (`text`).
    pub fn write(&self, hash: u64, text: &str) -> Result<(), String> {
        let d = self.world_dir(hash);
        std::fs::create_dir_all(&d).map_err(|e| format!("cannot make {}: {e}", d.display()))?;
        let tmp = d.join("world.json.tmp");
        std::fs::write(&tmp, text).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
        replace(&tmp, &d.join("world.json"))?;
        if let Err(e) = std::fs::write(self.dir.join("current"), format!("{hash:016x}")) {
            eprintln!("mapd: cannot note the open world in {}: {e}", self.dir.join("current").display());
        }
        Ok(())
    }

    /// Append a change to the world's edit log.
    pub fn log(&self, hash: u64, entry: Value) {
        let d = self.world_dir(hash);
        let _ = std::fs::create_dir_all(&d);
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(d.join("edits.jsonl")) {
            let _ = writeln!(f, "{entry}");
        }
    }

    /// An asset's id is the hex content hash the app gives it.
    pub fn asset_id_ok(id: &str) -> bool {
        (16..=64).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    }

    pub fn asset(&self, id: &str) -> Option<Vec<u8>> {
        Self::asset_id_ok(id).then(|| std::fs::read(self.dir.join("assets").join(id)).ok()).flatten()
    }

    pub fn has_asset(&self, id: &str) -> bool {
        Self::asset_id_ok(id) && self.dir.join("assets").join(id).exists()
    }

    pub fn put_asset(&self, id: &str, bytes: &[u8]) -> Result<(), String> {
        if !Self::asset_id_ok(id) {
            return Err("an asset id is 16–64 lowercase hex digits".into());
        }
        let d = self.dir.join("assets");
        let _ = std::fs::create_dir_all(&d);
        let tmp = d.join(format!("{id}.tmp"));
        std::fs::write(&tmp, bytes).and_then(|_| std::fs::rename(&tmp, d.join(id))).map_err(|e| e.to_string())
    }

    /// The world open last time.
    pub fn current(&self) -> Option<WorldFile> {
        let hash = u64::from_str_radix(std::fs::read_to_string(self.dir.join("current")).ok()?.trim(), 16).ok()?;
        self.load(hash)
    }
}

/// Move `tmp` over `dst`. A reader holding `dst` open (an editor, a sync client, a network
/// share) can make that fail for a moment: try again a few times, then (on Windows, where
/// replacing a file open elsewhere fails) remove `dst` first.
fn replace(tmp: &Path, dst: &Path) -> Result<(), String> {
    let mut err = None;
    for wait_ms in [0, 25, 50, 100, 200, 400, 800] {
        std::thread::sleep(Duration::from_millis(wait_ms));
        match std::fs::rename(tmp, dst) {
            Ok(()) => return Ok(()),
            Err(e) => err = Some(e),
        }
    }
    if cfg!(windows) && std::fs::remove_file(dst).is_ok() && std::fs::rename(tmp, dst).is_ok() {
        return Ok(());
    }
    Err(format!("cannot replace {} (it may be open elsewhere): {}", dst.display(), err.map(|e| e.to_string()).unwrap_or_default()))
}

/// A `world.json.tmp` newer than `world.json` (or with none) is a save that wrote its copy but
/// could not move it into place: take it, if it is a whole world file.
fn recover(d: &Path) {
    let (tmp, main) = (d.join("world.json.tmp"), d.join("world.json"));
    let Ok(t) = std::fs::metadata(&tmp).and_then(|m| m.modified()) else { return };
    let newer = std::fs::metadata(&main).and_then(|m| m.modified()).map(|m| t > m).unwrap_or(true);
    let whole = std::fs::read_to_string(&tmp).ok().is_some_and(|text| serde_json::from_str::<WorldFile>(&text).is_ok());
    if !(newer && whole) {
        return;
    }
    match replace(&tmp, &main) {
        Ok(()) => println!("mapd: recovered {} from a save that had not finished", main.display()),
        Err(e) => eprintln!("mapd: found a newer {} but could not recover it: {e}", tmp.display()),
    }
}
