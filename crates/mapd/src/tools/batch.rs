//! `batch`: many edit tools in one transaction. Each step runs as its own tool would, but on a
//! copy of the edits the batch builds up (later steps see what earlier ones did); at the end
//! the whole lot is saved, logged and sent to the app once. A step that fails changes nothing.

use std::cell::RefCell;

use serde_json::{Value, json};
use worldgen::world::Edits;

use crate::Shared;
use crate::worker::NO_WORLD;
use crate::tools::{Run, schema, text};

/// Steps one batch may hold.
const MAX_STEPS: usize = 5_000;

struct Staged {
    edits: Edits,
    changes: Vec<Value>,
}

tokio::task_local! {
    /// The edits the running batch builds up (only inside `batch`).
    static STAGED: RefCell<Option<Staged>>;
}

/// Whether this task is running a batch's steps.
pub fn active() -> bool {
    STAGED.try_with(|s| s.borrow().is_some()).unwrap_or(false)
}

/// Read the batch's edits.
pub fn with_staged<R>(f: impl FnOnce(&Edits) -> R) -> Option<R> {
    STAGED.with(|s| s.borrow().as_ref().map(|st| f(&st.edits)))
}

/// One step's edit: on the batch's copy; the generator follows, so later steps can read what
/// it made (a created site's id, a renamed place).
pub async fn stage(app: &crate::AppState, f: impl FnOnce(&mut Edits) -> Result<Value, String>) -> Result<Value, String> {
    let (ops, change) = STAGED.with(|s| -> Result<_, String> {
        let mut s = s.borrow_mut();
        let st = s.as_mut().expect("inside a batch");
        let mut edits = st.edits.clone();
        let change = f(&mut edits)?;
        let ops = st.edits.diff(&edits);
        if !ops.is_empty() {
            st.edits = edits;
            st.changes.push(change.clone());
        }
        Ok((ops, change))
    })?;
    if !ops.is_empty() {
        app.worker.run(move |g| g.apply_ops(&ops)).await?;
    }
    Ok(change)
}

pub fn list() -> Vec<Value> {
    vec![json!({
        "name": "batch",
        "title": "Many edits at once",
        "description": "Run many edit tools (set_feature, set_site, set_npc, remove, set_objects...) as one change: each step sees those before it; saved and shown once. If a step fails nothing changes and the error names it. Steps are {tool, arguments}; the result lists their results in order.",
        "inputSchema": schema(json!({
            "steps": {
                "type": "array",
                "items": { "type": "object", "properties": { "tool": { "type": "string", "description": "A tool's name" }, "arguments": { "type": "object", "default": {}, "description": "That tool's arguments" } }, "required": ["tool"] },
                "description": "The tools to run, in order (up to 5,000)",
            },
        }), &["steps"]),
    })]
}

pub async fn call(app: &Shared, name: &str, a: &Value) -> Option<Result<Vec<Value>, String>> {
    (name == "batch").then_some(())?;
    Some(run(app, a).await.map(text))
}

/// The index of a staged run's error that no step made (another world open, the save).
pub(crate) const NOT_A_STEP: usize = usize::MAX;

/// One step of a staged run: a tool called by name (a batch's), or a handler run directly (a merged tool's parts).
pub(crate) enum Step {
    Tool(String, Value),
    Handler(Run, &'static str, Value),
}

/// A step's reply: the JSON of its first text.
fn first(out: &[Value]) -> Value {
    out.first().and_then(|c| c["text"].as_str()).map(|t| serde_json::from_str(t).unwrap_or_else(|_| json!(t))).unwrap_or(Value::Null)
}

async fn step(app: &Shared, s: Step) -> Result<Value, String> {
    let out = match s {
        Step::Tool(tool, args) => Box::pin(crate::tools::call(app, &tool, args)).await?,
        Step::Handler(run, name, args) => Box::pin(crate::tools::run(app, run, name, &args)).await?,
    };
    Ok(first(&out))
}

/// Runs `steps` on a staged copy of the edits and saves them once as one change (`{"tool": "batch", ...}`: one
/// undo), or not at all: the error is `(the failed step's index, its error)`. Inside a batch, they are its steps.
async fn staged(app: &Shared, steps: Vec<Step>) -> Result<(Vec<Value>, usize, Value), (usize, String)> {
    if active() {
        let mut results = Vec::with_capacity(steps.len());
        for (i, s) in steps.into_iter().enumerate() {
            results.push(step(app, s).await.map_err(|e| (i, e))?);
        }
        return Ok((results, 0, Value::Null));
    }
    // Nothing else changes the world until the batch is done.
    let _one = app.gate.lock().await;
    if let Some(want) = crate::tools::wanted()
        && app.current() != Some(want)
    {
        return Err((NOT_A_STEP, crate::tools::other_world(app.current(), want)));
    }
    let start = app.with_edits(Edits::clone).ok_or((NOT_A_STEP, NO_WORLD.to_string()))?;
    let done = STAGED
        .scope(RefCell::new(Some(Staged { edits: start, changes: Vec::new() })), async {
            let mut results = Vec::with_capacity(steps.len());
            for (i, s) in steps.into_iter().enumerate() {
                results.push(step(app, s).await.map_err(|e| (i, e))?);
            }
            let staged = STAGED.with(|s| s.borrow_mut().take()).expect("staged edits");
            Ok::<_, (usize, String)>((results, staged))
        })
        .await;
    let (results, staged) = match done {
        Ok(d) => d,
        Err(e) => {
            // The generator had the steps so far: back to the world as it is.
            if let Some(edits) = app.with_edits(Edits::clone) {
                app.worker.run(move |g| g.set_edits(edits)).await;
            }
            return Err(e);
        }
    };
    let n = staged.changes.len();
    let change = json!({ "tool": "batch", "count": n, "changes": staged.changes });
    let edits = staged.edits;
    let made = app
        .edit_now("agent", 0, move |e| {
            *e = edits;
            Ok(change)
        })
        .await
        .map_err(|e| (NOT_A_STEP, e))?;
    Ok((results, n, made))
}

/// Handler calls as one change (`remove` across families, a `set_` tool's parts): one alone runs as it is; several
/// are staged like a batch's steps (all or none). Returns their replies in order.
pub(crate) async fn as_one(app: &Shared, steps: Vec<(Run, &'static str, Value)>) -> Result<Vec<Value>, (usize, String)> {
    let mut steps: Vec<Step> = steps.into_iter().map(|(r, n, a)| Step::Handler(r, n, a)).collect();
    if steps.len() == 1 {
        return Ok(vec![step(app, steps.remove(0)).await.map_err(|e| (0, e))?]);
    }
    staged(app, steps).await.map(|(r, _, _)| r)
}

async fn run(app: &Shared, a: &Value) -> Result<Value, String> {
    let steps = a["steps"].as_array().ok_or("give steps: a list of {tool, arguments}")?.clone();
    if steps.is_empty() || steps.len() > MAX_STEPS {
        return Err(format!("a batch holds 1 to {MAX_STEPS} steps"));
    }
    if active() {
        return Err("a batch can't hold another batch".into());
    }
    let mut list = Vec::with_capacity(steps.len());
    for (i, step) in steps.iter().enumerate() {
        let tool = step["tool"].as_str().ok_or_else(|| format!("step {}: missing 'tool'", i + 1))?;
        if matches!(tool, "batch" | "render_view" | "focus_view") {
            return Err(format!("step {}: {tool} can't be used in a batch", i + 1));
        }
        list.push(Step::Tool(tool.to_string(), step.get("arguments").cloned().unwrap_or_else(|| json!({}))));
    }
    let names: Vec<String> = steps.iter().map(|s| s["tool"].as_str().unwrap_or_default().to_string()).collect();
    let (results, n, made) = staged(app, list).await.map_err(|(i, e)| match names.get(i) {
        Some(t) => format!("step {} ({t}) failed: {e}. Nothing was changed.", i + 1),
        None => e,
    })?;
    let mut v = json!({ "steps": results.len(), "changes": n, "results": results });
    // (A failed save: the batch stands, once; see `edit_now`.)
    if made["saved"] == json!(false) {
        v["saved"] = json!(false);
        v["warning"] = made["warning"].clone();
    }
    Ok(v)
}
