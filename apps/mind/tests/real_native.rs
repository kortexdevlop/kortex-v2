//! The desktop path of Mind against the REAL model: native streaming with a device token, the
//! conversation loop, and real tool binaries run through their `--engine` mode. Ignored by
//! default: it needs a local api (`wrangler dev` in services/api, Workers AI behind it), a few
//! free neurons, and `cargo xtask build-desktop text.cleaner` / `text.case-converter`.
//!
//!   MIND_E2E_API=http://127.0.0.1:8787 cargo test -p kortex-mind --test real_native -- --ignored --nocapture
#![cfg(not(target_arch = "wasm32"))]
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use kortex_ai::stream::Event;
use kortex_contract::ToolContract;
use kortex_mind::client::{Auth, ask, credits};
use kortex_mind::engine::{Conversation, Next, select_tools};
use kortex_platform::native::{HttpRequest, http};
use kortex_runtime::{Value, block_on};
use kortex_workflow::Runner;
use kortex_workflow::process::ProcessRunner;

fn post(api: &str, path: &str, body: &serde_json::Value) -> (u16, serde_json::Value) {
    let url = format!("{api}{path}");
    let payload = body.to_string();
    let reply = http(HttpRequest {
        method: "POST",
        url: &url,
        headers: &[
            ("X-Kortex-Request", "1"),
            ("Content-Type", "application/json"),
        ],
        body: Some(payload.as_bytes()),
        timeout: Duration::from_secs(30),
        max_bytes: 65_536,
    })
    .unwrap_or_else(|e| unreachable!("{e}"));
    (
        reply.status,
        serde_json::from_slice(&reply.body).unwrap_or_default(),
    )
}

fn binary(id: &str) -> Option<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../dist/desktop");
    std::fs::read_dir(root)
        .ok()?
        .flatten()
        .filter_map(|target| std::fs::read_dir(target.path().join(id)).ok())
        .flat_map(|files| files.flatten().map(|f| f.path()))
        .find(|path| path.is_file())
}

#[test]
#[ignore = "needs a local api, the real model and built desktop binaries"]
fn a_desktop_conversation_runs_real_tools_with_the_real_model() {
    let api = std::env::var("MIND_E2E_API").unwrap_or_else(|_| "http://127.0.0.1:8787".into());
    assert!(
        kortex_platform::native::is_local_http(&api),
        "this test only talks to a local api"
    );
    let binaries: BTreeMap<String, PathBuf> = ["text.cleaner", "text.case-converter"]
        .iter()
        .filter_map(|id| binary(id).map(|p| ((*id).to_owned(), p)))
        .collect();
    assert_eq!(binaries.len(), 2, "build the desktop binaries first");
    let runner = ProcessRunner {
        binaries,
        timeout: Duration::from_secs(60),
    };

    // A throwaway local account and a device token that is never written anywhere.
    let email = format!("mind-native-{}@example.test", kortex_platform::now_ms());
    let password = format!("pw-{}", kortex_platform::now_ms() ^ 0x5eed);
    let auth_key = kortex_pkg::derive_auth_key(&email, &password, 5000);
    let credentials = serde_json::json!({"email": email, "auth_key": auth_key, "kdf": 1});
    assert_eq!(post(&api, "/v1/auth/register", &credentials).0, 201);
    let (status, device) = post(
        &api,
        "/v1/auth/device",
        &serde_json::json!({"email": email, "auth_key": auth_key, "kdf": 1, "name": "mind e2e", "platform": "test"}),
    );
    assert_eq!(status, 201);
    let auth = Auth::Device {
        api: api.clone(),
        token: device["token"].as_str().unwrap_or_default().to_owned(),
    };

    assert_eq!(block_on(credits(&auth)).map(|c| c.percent), Some(100));

    let contracts: BTreeMap<String, ToolContract> =
        kortex_workflow::contracts_from_registry(include_str!("../../../registry/index.json"))
            .unwrap_or_default()
            .into_iter()
            .map(|c| (c.tool.id.clone(), c))
            .collect();
    let message =
        "Nettoie le texte '  bonjour    le   monde  ' puis mets le résultat en majuscules.";
    let mut conversation = Conversation::new("fr");
    assert!(conversation.submit(message));
    let candidates = select_tools(&contracts, message, "fr", 8);
    let definitions: Vec<serde_json::Value> = candidates
        .iter()
        .filter_map(|id| contracts.get(id))
        .map(|c| kortex_ai::tools::definition(c, "fr"))
        .collect();

    let mut streamed = 0;
    let mut final_text = None;
    for _ in 0..6 {
        let request = conversation.request(definitions.clone(), true);
        let (turn, credits_after) = block_on(ask(&auth, &request, |event| {
            if matches!(event, Event::Delta { .. }) {
                streamed += 1;
            }
        }))
        .unwrap_or_else(|e| unreachable!("the gateway answered {e:?}"));
        assert!(
            credits_after.is_some(),
            "the done event carries the credits"
        );
        match conversation.receive(turn, &contracts) {
            Next::Answer => {
                final_text = Some(());
                break;
            }
            Next::Ask => {}
            Next::AwaitApproval => {
                // The user's approval is the test's decision here; tools run through `--engine`.
                for call in conversation
                    .approve(&[])
                    .unwrap_or_else(|e| unreachable!("{e:?}"))
                {
                    let result = runner.run(&call.tool, call.inputs.clone(), call.params.clone());
                    conversation.record_result(&call.call_id, result);
                }
                conversation.finish_round();
            }
            Next::Stop(stop) => unreachable!("the loop stopped: {stop:?}"),
        }
    }
    assert!(streamed > 0, "events arrived through the native stream");
    assert!(final_text.is_some(), "the model ended with an answer");
    let uppercased = ["c1", "c2", "c3"]
        .iter()
        .filter_map(|id| conversation.result(id))
        .any(|o| o.get("text") == Some(&Value::Text("BONJOUR LE MONDE".into())));
    assert!(
        uppercased,
        "the second real tool produced BONJOUR LE MONDE from the first one's output"
    );
    assert!(!conversation.export("x").nodes.is_empty());
    assert!(block_on(credits(&auth)).is_some_and(|c| c.percent <= 100));
}
