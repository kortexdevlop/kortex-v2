use std::collections::BTreeMap;

use kortex_ai::chat::{Message, ToolCall};
use kortex_ai::consent::Kind;
use kortex_ai::stream::Turn;
use kortex_contract::ToolContract;
use kortex_mind::engine::{Conversation, Entry, MAX_CORRECTIONS, MAX_ROUNDS, Next, Saved, Stop};
use kortex_runtime::{Value, ValueMap};
use kortex_workflow::{RunError, catalogue_of, validate};

fn contracts() -> BTreeMap<String, ToolContract> {
    kortex_workflow::contracts_from_registry(include_str!("../../../registry/index.json"))
        .unwrap_or_default()
        .into_iter()
        .map(|c| (c.tool.id.clone(), c))
        .collect()
}

fn calls(list: &[(&str, &str)]) -> Turn {
    Turn {
        content: None,
        tool_calls: list
            .iter()
            .map(|(name, args)| ToolCall {
                id: "call_random_from_provider".into(),
                name: (*name).into(),
                arguments: (*args).into(),
            })
            .collect(),
        finish_reason: "tool_calls".into(),
    }
}

fn text(content: &str) -> Turn {
    Turn {
        content: Some(content.into()),
        tool_calls: vec![],
        finish_reason: "stop".into(),
    }
}

fn outputs(port: &str, value: &str) -> ValueMap {
    ValueMap::from([(port.to_owned(), Value::Text(value.into()))])
}

/// The gateway rejects a conversation where an assistant tool call has no tool answer.
fn assert_protocol_consistent(messages: &[Message]) {
    let mut open: Vec<String> = Vec::new();
    for message in messages {
        match message {
            Message::Assistant { tool_calls, .. } => {
                assert!(
                    open.is_empty(),
                    "tool calls {open:?} unanswered before an assistant turn"
                );
                open = tool_calls.iter().map(|c| c.id.clone()).collect();
            }
            Message::Tool { call_id, .. } => {
                assert!(
                    open.contains(call_id),
                    "tool answer {call_id} without a pending call"
                );
                open.retain(|id| id != call_id);
            }
            Message::User(_) | Message::System(_) => assert!(
                open.is_empty(),
                "tool calls {open:?} unanswered before a user turn"
            ),
        }
    }
}

#[test]
fn quoted_text_stays_local_and_the_model_sees_a_reference() {
    let mut c = Conversation::new("fr");
    assert!(!c.submit("   "));
    assert!(c.submit("Nettoie ce texte : '  a   b '"));
    assert_eq!(c.attachments().len(), 1);
    assert_eq!(c.attachments()[0].as_text(), Some("  a   b "));
    assert_eq!(
        c.transcript(),
        [Entry::User("Nettoie ce texte : '  a   b '".into())],
        "the user sees what they typed"
    );
    let request = c.request(vec![], false);
    let Message::System(system) = &request.messages[0] else {
        unreachable!("system first")
    };
    assert!(
        system.contains("@texte1") && !system.contains("a   b"),
        "metadata only: {system}"
    );
    assert_eq!(
        request.messages[1],
        Message::User("Nettoie ce texte : @texte1".into())
    );
}

#[test]
fn a_full_exchange_runs_two_chained_tools_without_the_model_seeing_the_data() {
    let all = contracts();
    let mut c = Conversation::new("fr");
    c.submit("Nettoie le texte '  bonjour    le   monde  ' puis mets-le en majuscules");
    assert_eq!(
        c.receive(calls(&[("text__cleaner", r#"{"text":"@texte1"}"#)]), &all),
        Next::AwaitApproval
    );
    let plan = c.pending().unwrap_or_default().to_vec();
    assert_eq!(
        (plan[0].call_id.as_str(), plan[0].tool.as_str()),
        ("c1", "text.cleaner"),
        "short ids replace the provider's"
    );
    assert_eq!(
        plan[0].refs,
        [("text".to_owned(), kortex_ai::plan::Reference::Text(1))]
    );
    let approved = c.approve(&[]).unwrap_or_else(|e| unreachable!("{e:?}"));
    assert_eq!(approved.len(), 1);
    c.record_result("c1", Ok(outputs("text", "bonjour le monde")));
    assert_eq!(c.finish_round(), Next::Ask);
    // The model's next request carries a summary with a reference, never the cleaned text.
    let second_request = c.request(vec![], false).messages;
    let summary = second_request
        .iter()
        .find_map(|m| {
            if let Message::Tool { content, .. } = m {
                Some(content.clone())
            } else {
                None
            }
        })
        .unwrap_or_default();
    assert!(
        summary.contains("@c1.text") && !summary.contains("bonjour le monde"),
        "{summary}"
    );
    assert_protocol_consistent(&second_request);
    assert_eq!(
        c.receive(
            calls(&[(
                "text__case-converter",
                r#"{"text":"@c1.text","case":"upper"}"#
            )]),
            &all
        ),
        Next::AwaitApproval
    );
    assert_eq!(c.pending().map(|p| p[0].call_id.as_str()), Some("c2"));
    let _ = c.approve(&[]);
    c.record_result("c2", Ok(outputs("text", "BONJOUR LE MONDE")));
    assert_eq!(c.finish_round(), Next::Ask);
    assert_eq!(c.receive(text("C'est fait."), &all), Next::Answer);
    assert_eq!(
        c.result("c2").and_then(|o| o.get("text")),
        Some(&Value::Text("BONJOUR LE MONDE".into())),
        "the real value is kept locally for display"
    );
    assert_protocol_consistent(&c.request(vec![], false).messages);
    // The whole exchange exports as a workflow Flow accepts.
    let workflow = c.export("Mind").unwrap_or_else(|e| unreachable!("{e:?}"));
    assert_eq!(
        workflow
            .nodes
            .iter()
            .map(|n| n.id.as_str())
            .collect::<Vec<_>>(),
        ["t1", "c1", "c2"]
    );
    let list: Vec<ToolContract> = all.values().cloned().collect();
    assert!(validate(&workflow, &catalogue_of(&list)).is_empty());
}

#[test]
fn an_invented_reference_is_corrected_through_the_model_and_then_gives_up() {
    let all = contracts();
    let mut c = Conversation::new("fr");
    c.submit("Nettoie 'x'");
    let bad = || calls(&[("text__cleaner", r#"{"text":"@call_0.text"}"#)]);
    assert_eq!(
        c.receive(bad(), &all),
        Next::Ask,
        "first mistake: the model gets the error and retries"
    );
    let messages = c.request(vec![], false).messages;
    let feedback = messages
        .iter()
        .rev()
        .find_map(|m| {
            if let Message::Tool { content, .. } = m {
                Some(content.clone())
            } else {
                None
            }
        })
        .unwrap_or_default();
    assert!(
        feedback.contains("unknown_reference") && feedback.contains("@texte1"),
        "{feedback}"
    );
    assert_protocol_consistent(&messages);
    assert_eq!(c.receive(bad(), &all), Next::Ask);
    assert_eq!(MAX_CORRECTIONS, 2);
    assert_eq!(c.receive(bad(), &all), Next::Stop(Stop::TooManyCorrections));
    assert!(
        c.pending().is_none(),
        "nothing is ever run from an invalid call"
    );
    assert_protocol_consistent(&c.request(vec![], false).messages);
    assert_eq!(
        c.transcript().last(),
        Some(&Entry::Stopped(Stop::TooManyCorrections))
    );
}

#[test]
fn a_valid_call_next_to_an_invalid_one_waits_for_the_corrected_turn() {
    let all = contracts();
    let mut c = Conversation::new("fr");
    c.submit("Nettoie 'x' et fais autre chose");
    let turn = calls(&[
        ("text__cleaner", r#"{"text":"@texte1"}"#),
        ("text__nope", "{}"),
    ]);
    assert_eq!(c.receive(turn, &all), Next::Ask);
    assert!(c.pending().is_none());
    let messages = c.request(vec![], false).messages;
    assert_protocol_consistent(&messages);
    let tools: Vec<String> = messages
        .iter()
        .filter_map(|m| {
            if let Message::Tool { content, .. } = m {
                Some(content.clone())
            } else {
                None
            }
        })
        .collect();
    assert_eq!(tools.len(), 2, "both calls are answered");
    assert!(tools[0].contains("Non exécuté") && tools[1].contains("unknown_tool"));
}

#[test]
fn a_correct_turn_resets_the_correction_budget() {
    let all = contracts();
    let mut c = Conversation::new("fr");
    c.submit("Nettoie 'x'");
    let bad = || calls(&[("text__cleaner", r#"{"text":"@texte9"}"#)]);
    assert_eq!(c.receive(bad(), &all), Next::Ask);
    assert_eq!(
        c.receive(calls(&[("text__cleaner", r#"{"text":"@texte1"}"#)]), &all),
        Next::AwaitApproval
    );
    c.record_result("c2", Ok(outputs("text", "x")));
    c.finish_round();
    assert_eq!(
        c.receive(bad(), &all),
        Next::Ask,
        "two more mistakes are allowed after a good turn"
    );
    assert_eq!(c.receive(bad(), &all), Next::Ask);
}

#[test]
fn the_number_of_tool_rounds_per_message_is_bounded() {
    let all = contracts();
    let mut c = Conversation::new("fr");
    c.submit("Nettoie 'x' sans fin");
    for round in 1..=MAX_ROUNDS {
        assert_eq!(
            c.receive(calls(&[("text__cleaner", r#"{"text":"@texte1"}"#)]), &all),
            Next::AwaitApproval,
            "round {round}"
        );
        c.record_result(&format!("c{round}"), Ok(outputs("text", "x")));
        c.finish_round();
    }
    assert_eq!(
        c.receive(calls(&[("text__cleaner", r#"{"text":"@texte1"}"#)]), &all),
        Next::Stop(Stop::TooManyRounds)
    );
    assert_protocol_consistent(&c.request(vec![], false).messages);
    // A new user message starts a fresh budget.
    c.submit("Encore une");
    assert_eq!(
        c.receive(calls(&[("text__cleaner", r#"{"text":"@texte1"}"#)]), &all),
        Next::AwaitApproval
    );
}

#[test]
fn refusing_a_plan_tells_the_model_and_runs_nothing() {
    let all = contracts();
    let mut c = Conversation::new("fr");
    c.submit("Nettoie 'x'");
    c.receive(calls(&[("text__cleaner", r#"{"text":"@texte1"}"#)]), &all);
    assert_eq!(c.refuse(), Next::Ask);
    assert!(c.pending().is_none() && c.result("c1").is_none());
    let messages = c.request(vec![], false).messages;
    assert_protocol_consistent(&messages);
    assert!(
        matches!(messages.last(), Some(Message::Tool { content, .. }) if content.contains("refused_by_user"))
    );
    assert_eq!(c.transcript().last(), Some(&Entry::Refused));
    assert!(
        c.export("x").map(|w| w.nodes.is_empty()).unwrap_or(false),
        "a refused call is not exported"
    );
}

#[test]
fn sensitive_network_or_write_tools_need_their_own_confirmation() {
    let mut all = contracts();
    if let Some(tool) = all.get_mut("text.cleaner") {
        tool.permissions.network = true;
    }
    let mut c = Conversation::new("fr");
    c.submit("Nettoie 'x'");
    c.receive(calls(&[("text__cleaner", r#"{"text":"@texte1"}"#)]), &all);
    assert_eq!(
        c.pending().map(|p| p[0].confirmations.clone()),
        Some(vec!["network"])
    );
    let missing = c
        .approve(&[])
        .err()
        .unwrap_or_else(|| unreachable!("must be refused"));
    assert_eq!(
        (missing.call_id.as_str(), missing.kind.as_str()),
        ("c1", "network")
    );
    assert!(
        c.approve(&["c1:sensitive".into()]).is_err(),
        "a different confirmation does not count"
    );
    assert!(c.approve(&["c1:network".into()]).is_ok());
    assert!(
        c.pending().is_some(),
        "approving does not clear the plan: results do"
    );
}

#[test]
fn a_failing_tool_is_reported_to_the_model_and_kept_out_of_the_export() {
    let all = contracts();
    let mut c = Conversation::new("fr");
    c.submit("Nettoie 'x'");
    c.receive(calls(&[("text__cleaner", r#"{"text":"@texte1"}"#)]), &all);
    c.record_result(
        "c1",
        Err(RunError {
            code: "empty_input".into(),
            detail: Some("no text".into()),
        }),
    );
    c.finish_round();
    let messages = c.request(vec![], false).messages;
    assert_protocol_consistent(&messages);
    assert!(
        matches!(messages.last(), Some(Message::Tool { content, .. }) if content.contains("empty_input") && content.contains("no text"))
    );
    assert!(
        matches!(c.transcript().last(), Some(Entry::Failed { code, .. }) if code == "empty_input")
    );
    assert!(
        c.export("x")
            .map(|w| w.nodes.iter().all(|n| n.id != "c1"))
            .unwrap_or(false)
    );
}

#[test]
fn files_are_references_too_and_blocks_the_workflow_export() {
    let all = contracts();
    let mut c = Conversation::new("fr");
    let n = c.attach_file("a.pdf", "application/pdf", b"%PDF-1.4".to_vec());
    assert_eq!(n, 1);
    c.submit("Fusionne mes PDF");
    assert!(
        matches!(c.request(vec![], false).messages[0].clone(), Message::System(s) if s.contains("@fichier1") && s.contains("a.pdf") && !s.contains("%PDF"))
    );
    c.receive(
        calls(&[("pdf__merge", r#"{"files":["@fichier1","@fichier1"]}"#)]),
        &all,
    );
    c.record_result("c1", Ok(ValueMap::new()));
    assert!(c.export("x").is_err(), "Flow v1 has no file nodes");
}

#[test]
fn sharing_content_is_an_explicit_per_attachment_choice() {
    let mut c = Conversation::new("fr");
    let n = c.attach_text("mon secret");
    let prompt = |c: &Conversation| match &c.request(vec![], false).messages[0] {
        Message::System(s) => s.clone(),
        _ => String::new(),
    };
    assert!(!prompt(&c).contains("mon secret"));
    c.set_share(&Kind::Text, n, true);
    assert!(prompt(&c).contains("mon secret"));
    c.set_share(&Kind::Text, n, false);
    assert!(
        !prompt(&c).contains("mon secret"),
        "withdrawing consent takes effect on the next request"
    );
    c.remove_attachment(&Kind::Text, n);
    assert!(c.attachments().is_empty());
}

#[test]
fn old_exchanges_are_dropped_whole_when_the_history_outgrows_the_gateway_limits() {
    let all = contracts();
    let mut c = Conversation::new("fr");
    for i in 0..30 {
        c.submit(&format!("question {i} {}", "mot ".repeat(400)));
        c.receive(text(&"réponse ".repeat(100)), &all);
    }
    let messages = c.request(vec![], false).messages;
    assert!(messages.len() <= 51, "{} messages", messages.len());
    assert!(
        matches!(messages[1], Message::User(_)),
        "the history starts on a user turn, not mid-exchange"
    );
    assert!(matches!(messages.last(), Some(Message::Assistant { .. })));
    assert!(
        messages
            .iter()
            .map(|m| match m {
                Message::System(t) | Message::User(t) => t.len(),
                Message::Assistant { content, .. } => content.as_deref().map_or(0, str::len),
                Message::Tool { content, .. } => content.len(),
            })
            .sum::<usize>()
            < 40_000
    );
    assert_protocol_consistent(&messages);
}

#[test]
fn a_saved_conversation_comes_back_without_file_contents_and_keeps_numbering() {
    let all = contracts();
    let mut c = Conversation::new("en");
    c.submit("Clean 'a b'");
    c.attach_file("secret.pdf", "application/pdf", vec![1, 2, 3]);
    c.receive(calls(&[("text__cleaner", r#"{"text":"@texte1"}"#)]), &all);
    c.record_result("c1", Ok(outputs("text", "a b")));
    c.finish_round();
    let json = serde_json::to_string(&c.saved()).unwrap_or_default();
    assert!(
        !json.contains("secret.pdf"),
        "file attachments are never persisted"
    );
    let restored = Conversation::restore(serde_json::from_str::<Saved>(&json).unwrap_or_default());
    assert_eq!(restored.transcript(), c.transcript());
    assert_eq!(
        restored.attachments().len(),
        1,
        "texts are kept so their references still resolve"
    );
    let mut restored = restored;
    restored.submit("Again 'c d'");
    restored.receive(calls(&[("text__cleaner", r#"{"text":"@texte2"}"#)]), &all);
    assert_eq!(
        restored.pending().map(|p| p[0].call_id.as_str()),
        Some("c2"),
        "ids continue where they stopped"
    );
    assert!(
        restored.export("x").is_ok(),
        "executed calls survive the reload"
    );
}

#[test]
fn candidate_tools_follow_the_request_and_never_leave_the_model_empty_handed() {
    use kortex_mind::engine::select_tools;
    let all = contracts();
    let ids = |q: &str| select_tools(&all, q, "fr", 8);
    let cleaning = ids("Nettoie le texte @texte1 puis mets-le en majuscules");
    assert!(
        cleaning.contains(&"text.cleaner".to_owned())
            && cleaning.contains(&"text.case-converter".to_owned()),
        "{cleaning:?}"
    );
    assert!(
        ids("génère un mot de passe sécurisé").contains(&"secure.password-generator".to_owned())
    );
    assert!(ids("fusionne mes pdf").contains(&"pdf.merge".to_owned()));
    assert!(ids("convertis cette image png en jpg").contains(&"image.convert".to_owned()));
    let vague = ids("aide-moi");
    assert!(
        vague.len() >= 4 && vague.len() <= 8,
        "popular tools pad a vague request: {vague:?}"
    );
    assert!(
        ids("@texte1 @texte2").len() >= 4,
        "references alone are not words of a request"
    );
    assert_eq!(
        select_tools(&all, "mot de passe", "en", 3).len(),
        3.max(select_tools(&all, "mot de passe", "en", 3).len())
            .min(6)
    );
}

#[test]
fn tools_already_used_stay_offered_for_the_next_steps() {
    let all = contracts();
    let mut c = Conversation::new("fr");
    c.submit("Nettoie 'x'");
    c.receive(calls(&[("text__cleaner", r#"{"text":"@texte1"}"#)]), &all);
    assert!(c.used_tools().is_empty(), "only executed calls count");
    c.record_result("c1", Ok(outputs("text", "x")));
    assert_eq!(c.used_tools(), ["text.cleaner"]);
}
