//! `batch`: many edit tools in one transaction. Each step runs as its own tool would, but on a
//! copy of the edits the batch builds up (later steps see what earlier ones did); at the end
//! the whole lot is saved, logged and sent to the app once. A step that fails changes nothing.

use std::cell::RefCell;

use serde_json::{Value, json};
use worldgen::world::Edits;

use crate::Shared;
use crate::tools::{schema, text};

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
        "description": "Run many edit tools (rename_feature, annotate_feature, update_feature, hide_feature, create_feature, create_npc, create_plot, place_objects...) as one change: each step sees what the steps before it did, and the whole batch is saved and shown in the app once. If a step fails, nothing is changed and the error names the step. Each step is {tool, arguments}; the result lists each step's result in order.",
        "inputSchema": schema(json!({
            "steps": {
                "type": "array",
                "items": { "type": "object", "properties": { "tool": { "type": "string" }, "arguments": { "type": "object" } }, "required": ["tool"] },
                "description": "The tools to run, in order (up to 5,000)",
            },
        }), &["steps"]),
    })]
}

pub async fn call(app: &Shared, name: &str, a: &Value) -> Option<Result<Vec<Value>, String>> {
    (name == "batch").then_some(())?;
    Some(run(app, a).await.map(text))
}

async fn run(app: &Shared, a: &Value) -> Result<Value, String> {
    let steps = a["steps"].as_array().ok_or("give steps: a list of {tool, arguments}")?.clone();
    if steps.is_empty() || steps.len() > MAX_STEPS {
        return Err(format!("a batch holds 1 to {MAX_STEPS} steps"));
    }
    if active() {
        return Err("a batch can't hold another batch".into());
    }
    // Nothing else changes the world until the batch is done.
    let _one = app.gate.lock().await;
    if let Some(want) = crate::tools::wanted()
        && app.current() != Some(want)
    {
        return Err(crate::tools::other_world(app.current(), want));
    }
    let start = app.with_edits(Edits::clone).ok_or("no world is open: open the map app first")?;
    let done = STAGED
        .scope(RefCell::new(Some(Staged { edits: start, changes: Vec::new() })), async {
            let mut results = Vec::with_capacity(steps.len());
            for (i, step) in steps.iter().enumerate() {
                let tool = step["tool"].as_str().ok_or_else(|| format!("step {}: missing 'tool'", i + 1))?;
                if matches!(tool, "batch" | "render_view" | "focus_view") {
                    return Err(format!("step {}: {tool} can't be used in a batch", i + 1));
                }
                let args = step.get("arguments").cloned().unwrap_or_else(|| json!({}));
                let out = Box::pin(crate::tools::call(app, tool, args)).await.map_err(|e| format!("step {} ({tool}) failed: {e}", i + 1))?;
                let first = out.first().and_then(|c| c["text"].as_str()).map(|t| serde_json::from_str(t).unwrap_or_else(|_| json!(t))).unwrap_or(Value::Null);
                results.push(first);
            }
            let staged = STAGED.with(|s| s.borrow_mut().take()).expect("staged edits");
            Ok::<_, String>((results, staged))
        })
        .await;
    let (results, staged) = match done {
        Ok(d) => d,
        Err(e) => {
            // The generator had the steps so far: back to the world as it is.
            if let Some(edits) = app.with_edits(Edits::clone) {
                app.worker.run(move |g| g.set_edits(edits)).await;
            }
            return Err(format!("{e}. Nothing was changed."));
        }
    };
    let n = staged.changes.len();
    let change = json!({ "tool": "batch", "count": n, "changes": staged.changes });
    let edits = staged.edits;
    app.edit_now("agent", 0, move |e| {
        *e = edits;
        Ok(change)
    })
    .await?;
    Ok(json!({ "steps": results.len(), "changes": n, "results": results }))
}
