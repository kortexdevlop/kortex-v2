//! The workflow builder against scripted model replies: what is accepted, what is repaired once
//! and what is refused. The real model is exercised by the browser recipe.
use std::cell::RefCell;

use kortex_ai::chat::Message;
use kortex_flow::builder::{MAX_NODES, build, catalogue_text, instruction, parse};
use kortex_flow::catalogue::ports;
use kortex_runtime::block_on;

const GOOD: &str = r#"Voici :
```json
{"name":"Nettoyer","nodes":[
 {"id":"in","type":"logic","op":"input","params":{"name":"texte"}},
 {"id":"c","type":"tool","tool":"text.cleaner","params":{}},
 {"id":"out","type":"logic","op":"output","params":{"name":"resultat"}}],
 "edges":[{"from":"in.text","to":"c.text"},{"from":"c.text","to":"out.text"}]}
```"#;

fn scripted(replies: &[&str]) -> (RefCell<Vec<Vec<Message>>>, RefCell<Vec<String>>) {
    (
        RefCell::new(Vec::new()),
        RefCell::new(replies.iter().rev().map(|s| (*s).to_owned()).collect()),
    )
}

#[test]
fn the_catalogue_text_lists_tools_and_logic_but_not_secret_cards() {
    let text = catalogue_text(ports());
    assert!(text.contains("- text.cleaner: in(text:text*)"));
    assert!(text.contains("- template:") && text.contains("- condition:"));
    assert!(
        !text.contains("connector.") && !text.contains("- http:") && !text.contains("workflow.")
    );
    assert!(instruction(ports(), "fr").contains("French"));
}

#[test]
fn a_good_reply_becomes_a_valid_laid_out_workflow() {
    let (seen, queue) = scripted(&[GOOD]);
    let built = block_on(build("nettoie un texte", "fr", ports(), |m| {
        seen.borrow_mut().push(m);
        let next = queue.borrow_mut().pop().ok_or("no reply".to_owned());
        async move { next }
    }))
    .expect("built");
    assert!(!built.repaired);
    assert_eq!(built.workflow.name, "Nettoyer");
    assert_eq!(built.workflow.nodes.len(), 3);
    assert!(built.workflow.nodes.iter().all(|n| n.pos.is_some()));
    // The request reaches the model as the person's words, after the instruction.
    let first = &seen.borrow()[0];
    assert!(matches!(&first[1], Message::User(t) if t.contains("nettoie un texte")));
}

#[test]
fn an_invented_tool_is_repaired_once_with_the_problem_fed_back() {
    let bad = r#"{"nodes":[{"id":"a","type":"tool","tool":"magic.wand"}],"edges":[]}"#;
    let (seen, queue) = scripted(&[bad, GOOD]);
    let built = block_on(build("x", "en", ports(), |m| {
        seen.borrow_mut().push(m);
        let next = queue.borrow_mut().pop().ok_or("no reply".to_owned());
        async move { next }
    }))
    .expect("repaired");
    assert!(built.repaired);
    let second = &seen.borrow()[1];
    assert!(
        matches!(second.last(), Some(Message::User(t)) if t.contains("magic.wand") || t.contains("unknown"))
    );
}

#[test]
fn two_bad_answers_are_refused_and_nothing_is_returned() {
    let bad = "I cannot do that";
    let (_, queue) = scripted(&[bad, bad, bad]);
    let failed = block_on(build("x", "en", ports(), |_| {
        let next = queue.borrow_mut().pop().ok_or("no reply".to_owned());
        async move { next }
    }))
    .expect_err("refused");
    assert_eq!(failed.code, "invalid");
    assert!(failed.detail.starts_with("not_json"));
}

#[test]
fn transport_errors_and_empty_requests_stop_early() {
    let failed =
        block_on(build("   ", "en", ports(), |_| async { Ok(String::new()) })).expect_err("empty");
    assert_eq!(failed.code, "empty_request");
    let failed = block_on(build("x", "en", ports(), |_| async {
        Err("ai_paused".to_owned())
    }))
    .expect_err("transport");
    assert_eq!(
        (failed.code, failed.detail.as_str()),
        ("transport", "ai_paused")
    );
}

#[test]
fn parsing_is_strict_about_shape_and_size() {
    assert_eq!(parse("nothing").expect_err("no json").code, "not_json");
    assert_eq!(parse(r#"{"nodes":[]}"#).expect_err("empty").code, "empty");
    assert_eq!(
        parse(r#"{"nodes":[{"id":"a","type":"alien"}]}"#)
            .expect_err("type")
            .code,
        "bad_node"
    );
    assert_eq!(
        parse(r#"{"nodes":[{"id":"a.b","type":"text"}]}"#)
            .expect_err("dotted id")
            .code,
        "bad_node"
    );
    let many: Vec<String> = (0..=MAX_NODES)
        .map(|i| format!(r#"{{"id":"n{i}","type":"text","value":"x"}}"#))
        .collect();
    assert_eq!(
        parse(&format!(r#"{{"nodes":[{}]}}"#, many.join(",")))
            .expect_err("big")
            .code,
        "too_big"
    );
    assert_eq!(
        parse(r#"{"nodes":[{"id":"a","type":"text"}],"edges":[{"from":"a","to":"b.x"}]}"#)
            .expect_err("edge")
            .code,
        "bad_edge"
    );
    // Numbers and booleans in params become text.
    let w =
        parse(r#"{"nodes":[{"id":"d","type":"logic","op":"delay","params":{"ms":500,"x":true}}]}"#)
            .expect("ok");
    assert!(
        matches!(&w.nodes[0].kind, kortex_workflow::NodeKind::Logic { params, .. } if params["ms"] == "500" && params["x"] == "true")
    );
}
