//! The loop that starts triggered workflows while Flow is open: every few seconds it asks which
//! triggers are due, starts a local webhook endpoint for each enabled webhook trigger (desktop),
//! and runs one workflow at a time through [`crate::headless`]. Nothing runs when Flow is closed.
use std::collections::{BTreeMap, VecDeque};
use std::sync::Mutex;

use dioxus::prelude::*;
use kortex_workflow::NodeKind;

use crate::triggers::Kind;
use crate::ui::Ctx;

/// Webhook calls waiting to run: (workflow id, body). Bounded so a flood cannot pile up.
static QUEUE: Mutex<VecDeque<(String, String)>> = Mutex::new(VecDeque::new());
const QUEUE_LIMIT: usize = 20;
const TICK_MS: u32 = 15_000;

#[cfg(not(target_arch = "wasm32"))]
thread_local! {
    static HOOKS: std::cell::RefCell<BTreeMap<String, (u16, String, kortex_platform::webhook::Hook)>> =
        const { std::cell::RefCell::new(BTreeMap::new()) };
}

pub(crate) fn has_ai(workflow: &kortex_workflow::Workflow) -> bool {
    workflow
        .nodes
        .iter()
        .any(|n| matches!(&n.kind, NodeKind::Logic { op, .. } if op == "ai"))
}

/// Keeps one local endpoint per enabled webhook trigger and none for the others.
#[cfg(not(target_arch = "wasm32"))]
fn sync_hooks(wanted: &BTreeMap<String, (u16, String)>) {
    HOOKS.with(|hooks| {
        let mut hooks = hooks.borrow_mut();
        hooks.retain(|id, (port, token, _)| {
            wanted.get(id).is_some_and(|(p, t)| p == port && t == token)
        });
        for (id, (port, token)) in wanted {
            if hooks.contains_key(id) {
                continue;
            }
            let (target, secret) = (id.clone(), token.clone());
            let started = kortex_platform::webhook::listen(*port, secret.clone(), move |body| {
                if let Ok(mut queue) = QUEUE.lock()
                    && queue.len() < QUEUE_LIMIT
                {
                    queue.push_back((target.clone(), body));
                }
            });
            if let Ok(hook) = started {
                hooks.insert(id.clone(), (*port, secret, hook));
            }
        }
    });
}

/// Whether the endpoint of a webhook trigger is actually listening.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn listening(id: &str) -> bool {
    HOOKS.with(|h| h.borrow().contains_key(id))
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn listening(_id: &str) -> bool {
    false
}

async fn tick(mut ctx: Ctx, opened: &mut bool, offset: &mut (u64, i32)) {
    if *ctx.auto_busy.peek() {
        return;
    }
    let now = kortex_platform::now_ms();
    if now.saturating_sub(offset.0) > 600_000 {
        *offset = (now, kortex_platform::document::utc_offset_minutes().await);
    }
    let library = ctx.library.peek().clone();
    let mut triggers = ctx.triggers.peek().clone();
    // A trigger of a workflow that was deleted goes away with it.
    let before = triggers.items.len();
    triggers.items.retain(|id, _| library.get(id).is_some());
    if triggers.items.len() != before {
        ctx.triggers.set(triggers.clone());
    }
    #[cfg(not(target_arch = "wasm32"))]
    sync_hooks(
        &triggers
            .items
            .iter()
            .filter(|(_, c)| c.enabled)
            .filter_map(|(id, c)| match &c.kind {
                Kind::Webhook { port, token } => Some((id.clone(), (*port, token.clone()))),
                _ => None,
            })
            .collect(),
    );
    let mut jobs: Vec<(String, BTreeMap<String, String>)> = triggers
        .items
        .iter()
        .filter(|(_, c)| c.due(now, offset.1, *opened))
        .map(|(id, _)| (id.clone(), BTreeMap::new()))
        .collect();
    *opened = true;
    let calls: Vec<(String, String)> = QUEUE
        .lock()
        .map(|mut q| q.drain(..).collect())
        .unwrap_or_default();
    for (id, body) in calls {
        if triggers.items.get(&id).is_some_and(|c| c.enabled) {
            jobs.push((id, BTreeMap::from([("payload".to_owned(), body)])));
        }
    }
    if jobs.is_empty() {
        return;
    }
    ctx.auto_busy.set(true);
    for (id, inputs) in jobs {
        let Some(saved) = library.get(&id) else {
            continue;
        };
        let label = format!("⏱ {}", saved.name);
        let outcome = crate::headless::run(&saved.workflow, &label, &inputs).await;
        ctx.runs.write().push(outcome.record.clone());
        if let Some(config) = ctx.triggers.write().items.get_mut(&id) {
            config.finished(outcome.record.at, outcome.ok, outcome.error);
        }
    }
    ctx.auto_busy.set(false);
}

/// Starts the loop for this window.
pub(crate) fn use_scheduler(ctx: Ctx) {
    use_future(move || async move {
        let (mut opened, mut offset) = (false, (0_u64, 0_i32));
        kortex_platform::document::sleep(1_500).await;
        loop {
            tick(ctx, &mut opened, &mut offset).await;
            kortex_platform::document::sleep(TICK_MS).await;
        }
    });
}
