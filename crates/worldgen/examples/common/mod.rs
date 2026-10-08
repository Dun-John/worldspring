//! Shared by the QA examples: which world to look at, and `--json` output.
//!
//! A world argument is a seed (`1`) or a world file (`world.json`: the app's export, a saved
//! library world or mapd's `<dir>/<hash>/world.json`). A file made for another generator
//! version is generated with this one (said on stderr).
#![allow(dead_code)]

use worldgen::world::GEN_VERSION;
use worldgen::{World, WorldFile};

/// The command line without its flags, and the flags.
pub struct Args {
    pub pos: Vec<String>,
    /// `--json`: the results as one JSON value on stdout (progress and notes stay on stderr).
    pub json: bool,
}

pub fn args() -> Args {
    let all: Vec<String> = std::env::args().skip(1).collect();
    let json = all.iter().any(|a| a == "--json");
    Args { pos: all.into_iter().filter(|a| a != "--json").collect(), json }
}

impl Args {
    pub fn get(&self, i: usize) -> Option<&str> {
        self.pos.get(i).map(|s| s.as_str())
    }

    pub fn num(&self, i: usize, default: f64) -> f64 {
        self.get(i).and_then(|s| s.parse().ok()).unwrap_or(default)
    }
}

/// Whether an argument names a world (a seed, or a file that exists or ends in `.json`).
pub fn is_world(arg: &str) -> bool {
    arg.parse::<u32>().is_ok() || arg.ends_with(".json") || std::path::Path::new(arg).is_file()
}

/// The world file a seed or a path names.
pub fn world_file(arg: &str) -> WorldFile {
    if let Ok(seed) = arg.parse::<u32>() {
        return WorldFile { seed, ..Default::default() };
    }
    let text = std::fs::read_to_string(arg).unwrap_or_else(|e| panic!("{arg}: {e}"));
    let mut file: WorldFile = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{arg}: not a world file ({e})"));
    if file.gen_version != GEN_VERSION {
        eprintln!("{arg}: made for generator v{}, generated here with v{GEN_VERSION}", file.gen_version);
        file.gen_version = GEN_VERSION;
    }
    file
}

pub fn world(arg: &str) -> World {
    World::new(world_file(arg)).unwrap_or_else(|e| panic!("{arg}: {e}"))
}

/// A short label for a world argument: the seed, or the file's name (mapd's `world.json` by
/// its folder, the world's hash).
pub fn label(arg: &str) -> String {
    if let Ok(seed) = arg.parse::<u32>() {
        return format!("seed {seed}");
    }
    let path = std::path::Path::new(arg);
    let name = |p: &std::path::Path| p.file_name().map(|f| f.to_string_lossy().into_owned());
    match name(path) {
        Some(n) if n == "world.json" => path.parent().and_then(name).map_or(n, |d| format!("{d}/world.json")),
        Some(n) => n,
        None => arg.into(),
    }
}
