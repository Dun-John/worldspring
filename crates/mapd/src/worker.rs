//! The generator thread: one native `Executor` (world, T0, terrain cache) that every request
//! runs against in turn. Generation keeps thread-local caches (settlement layouts), so it all
//! happens on this one thread; handlers send it closures and await their results. Between
//! jobs it lays out every settlement ahead of time, so searching businesses is quick.

use std::sync::mpsc;

use worldgen::pipeline::Executor;
use worldgen::t0::remap::{self, Gone};
use worldgen::world::{EditOp, Edits};
use worldgen::{World, WorldFile, town};

pub struct Gen {
    pub ex: Option<Executor>,
    /// Next layout to lay out ahead of time.
    warm: usize,
}

impl Gen {
    /// Open a world: regenerate T0 if it is a different world, else just take its edits.
    pub fn load(&mut self, file: WorldFile) -> Result<(), String> {
        let world = World::new(file)?;
        if let Some(ex) = &mut self.ex
            && ex.world.hash == world.hash
        {
            set_edits(ex, world.file.edits);
            return Ok(());
        }
        let t = std::time::Instant::now();
        self.ex = Some(Executor::new(world));
        self.warm = 0;
        println!("mapd: world generated in {:.1} s", t.elapsed().as_secs_f64());
        Ok(())
    }

    pub fn set_edits(&mut self, edits: Edits) {
        if let Some(ex) = &mut self.ex {
            set_edits(ex, edits);
        }
    }

    /// Apply edit ops to the open world's edits (a batch's step).
    pub fn apply_ops(&mut self, ops: &[EditOp]) -> Result<(), String> {
        let Some(ex) = &mut self.ex else { return Ok(()) };
        // (Only created sites touch generation here.)
        let old = ops.iter().any(|op| matches!(op, EditOp::Set { field, .. } | EditOp::Unset { field, .. } if field == "created")).then(|| ex.world.file.edits.created.clone());
        let done = ops.iter().try_for_each(|op| ex.world.file.edits.apply(op).map(|_| ()));
        if let Some(old) = old {
            created_changed(ex, &old);
        }
        done
    }

    /// Remake the open world with another sketch (`file`: the world file with it, and the open
    /// world's edits), its edits following their places (`t0::remap::remake`); `removed`: the
    /// pins taken away since the last remake (strokes of the open world's sketch), whose
    /// settlements' changes are set aside, never handed on. The remade world replaces the open
    /// one here only once it is whole; `swap` runs then, on this thread, so that no job sees one
    /// world here and the other in `swap`'s state.
    pub fn remake(&mut self, file: WorldFile, removed: &[usize], swap: impl FnOnce(u64, &WorldFile)) -> Result<Remade, String> {
        let Some(old) = &self.ex else { return Err("no world is open: open the map app first".into()) };
        let started = std::time::Instant::now();
        let mut new = World::new(file)?;
        let gone: Vec<Gone> = removed
            .iter()
            .filter_map(|&k| {
                let li = old.t0.settlements.iter().position(|s| s.pin == Some(k as u32))?;
                let id = worldgen::agent::feature_of_layout(&old.world, &old.t0, li)?;
                let name = old.world.file.edits.renames.get(&id).cloned().or_else(|| old.t0.extra.as_ref()?.overlay.features.iter().find(|f| f.id == id).map(|f| f.name.clone())).unwrap_or_default();
                Some(Gone { id, name, li })
            })
            .collect();
        let before = old.t0.settlements.len();
        let (mut t0, followed) = remap::remake(&old.world, &old.t0, &new, &gone);
        let (mut kept, mut parked, mut gone_n, mut moved) = (0, 0, 0, 0);
        let all_stayed = followed.is_none();
        if let Some(f) = followed {
            (kept, parked, gone_n, moved) = (f.kept, f.parked, f.removed, f.moved.len());
            new.file.edits = f.edits;
            // (The edits followed: T0 takes them as they are now.)
            t0.apply_edits(&new);
        }
        let after = t0.settlements.len();
        swap(new.hash, &new.file);
        let out = Remade { file: new.file.clone(), hash: new.hash, gone, kept, parked, removed: gone_n, moved, all_stayed, before, after, seconds: started.elapsed().as_secs_f64() };
        self.ex = Some(Executor::from_parts(new, t0));
        self.warm = 0;
        println!("mapd: world remade in {:.1} s", out.seconds);
        Ok(out)
    }

    fn warm_one(&mut self) -> bool {
        let Some(ex) = &self.ex else { return false };
        if self.warm >= town::layout_count(&ex.t0) {
            return false;
        }
        town::layout(&ex.world, &ex.t0, self.warm);
        self.warm += 1;
        true
    }
}

/// A world remade (`Gen::remake`): its file and hash, the settlements taken away on purpose,
/// how many changes followed their places to new numbers (`kept`), were set aside (`parked`; `removed`: with a
/// place taken away), how many features' changes moved to another, the settlements before and
/// after, and how long it took.
pub struct Remade {
    pub file: WorldFile,
    pub hash: u64,
    pub gone: Vec<Gone>,
    pub kept: usize,
    pub parked: usize,
    pub removed: usize,
    pub moved: usize,
    /// No change had to follow its place: every one stayed on it as it was.
    pub all_stayed: bool,
    pub before: usize,
    pub after: usize,
    pub seconds: f64,
}

fn set_edits(ex: &mut Executor, edits: Edits) {
    let old = std::mem::replace(&mut ex.world.file.edits, edits).created;
    created_changed(ex, &old);
}

/// Created sites applied again after the edits changed from `old`'s, and the cached layouts of
/// those that changed forgotten (the rest are kept).
fn created_changed(ex: &mut Executor, old: &[worldgen::world::Created]) {
    ex.t0.apply_edits(&ex.world);
    town::forget_created(old, &ex.world.file.edits.created, ex.t0.settlements.len() + ex.t0.base_pois);
}

type Job = Box<dyn FnOnce(&mut Gen) + Send>;

#[derive(Clone)]
pub struct GenHandle {
    tx: mpsc::Sender<Job>,
}

impl GenHandle {
    pub fn spawn() -> GenHandle {
        let (tx, rx) = mpsc::channel::<Job>();
        std::thread::Builder::new()
            .name("mapd-gen".into())
            .spawn(move || {
                let mut g = Gen { ex: None, warm: 0 };
                loop {
                    // Jobs first; layouts ahead of time only while idle.
                    let job = match rx.try_recv() {
                        Ok(j) => Some(j),
                        Err(mpsc::TryRecvError::Disconnected) => break,
                        Err(mpsc::TryRecvError::Empty) => {
                            if g.warm_one() {
                                continue;
                            }
                            match rx.recv() {
                                Ok(j) => Some(j),
                                Err(_) => break,
                            }
                        }
                    };
                    if let Some(j) = job {
                        j(&mut g);
                    }
                }
            })
            .expect("generator thread");
        GenHandle { tx }
    }

    /// Run `f` on the generator thread and wait for its result.
    pub async fn run<R: Send + 'static>(&self, f: impl FnOnce(&mut Gen) -> R + Send + 'static) -> R {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let _ = self.tx.send(Box::new(move |g: &mut Gen| {
            let _ = tx.send(f(g));
        }));
        rx.await.expect("generator thread alive")
    }

    /// Run `f` against the open world (an error if none is open yet).
    pub async fn with<R: Send + 'static>(&self, f: impl FnOnce(&Executor) -> Result<R, String> + Send + 'static) -> Result<R, String> {
        self.run(move |g| match &g.ex {
            Some(ex) => f(ex),
            None => Err("no world is open: open the map app (it connects to mapd and shares its world)".into()),
        })
        .await
    }
}
