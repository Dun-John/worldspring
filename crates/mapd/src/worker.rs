//! The generator thread: one native `Executor` (world, T0, terrain cache) that every request
//! runs against in turn. Generation keeps thread-local caches (settlement layouts), so it all
//! happens on this one thread; handlers send it closures and await their results. Between
//! jobs it lays out every settlement ahead of time, so searching businesses is quick.

use std::sync::mpsc;

use worldgen::pipeline::Executor;
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
        let mut edits = std::mem::take(&mut ex.world.file.edits);
        let done = ops.iter().try_for_each(|op| edits.apply(op).map(|_| ()));
        set_edits(ex, edits);
        done
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

fn set_edits(ex: &mut Executor, edits: Edits) {
    ex.world.file.edits = edits;
    ex.t0.apply_edits(&ex.world);
    town::forget_from(ex.t0.settlements.len() + ex.t0.base_pois);
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
