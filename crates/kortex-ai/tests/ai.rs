use std::collections::BTreeMap;

use kortex_ai::chat::{ApiError, Message, Request, ToolCall};
use kortex_ai::consent::{Attachment, Kind, describe_all};
use kortex_ai::credits::{Credits, Status};
use kortex_ai::plan::{
    Context, Executed, Reference, parse_reference, plan_call, summarize, to_workflow,
};
use kortex_ai::prompt;
use kortex_ai::stream::{Assembler, Event, SseParser, turn_from_response};
use kortex_ai::tools::{definition, function_name, tool_id};
use kortex_contract::ToolContract;
use kortex_runtime::{Table, Value, ValueMap};
use kortex_workflow::{catalogue_of, contracts_from_registry, validate};
use serde_json::{Value as Json, json};

fn contracts() -> BTreeMap<String, ToolContract> {
    contracts_from_registry(include_str!("../../../registry/contracts.json"))
        .unwrap_or_default()
        .into_iter()
        .map(|c| (c.tool.id.clone(), c))
        .collect()
}

fn call(id: &str, tool: &str, arguments: &str) -> ToolCall {
    ToolCall {
        id: id.into(),
        name: function_name(tool),
        arguments: arguments.into(),
    }
}

fn file(n: usize, name: &str, mime: &str, bytes: &[u8]) -> Attachment {
    Attachment {
        n,
        kind: Kind::File,
        name: name.into(),
        mime: mime.into(),
        bytes: bytes.to_vec(),
        share_content: false,
    }
}

fn plan(
    call: &ToolCall,
    attachments: &[Attachment],
    results: &BTreeMap<String, ValueMap>,
) -> Result<kortex_ai::plan::PlannedCall, kortex_ai::plan::PlanError> {
    let all = contracts();
    plan_call(
        call,
        &Context {
            attachments,
            results,
            contracts: &all,
        },
    )
}

#[test]
fn messages_and_requests_use_the_gateway_wire_format() {
    let request = Request {
        messages: vec![
            Message::System("s".into()),
            Message::User("u".into()),
            Message::Assistant {
                content: None,
                tool_calls: vec![call("c1", "text.cleaner", "{\"text\":\"@texte1\"}")],
            },
            Message::Tool {
                call_id: "c1".into(),
                content: "{}".into(),
            },
        ],
        tools: vec![json!({"type": "function"})],
        stream: true,
        max_tokens: 400,
    };
    let body: Json = serde_json::from_str(&request.body()).unwrap_or_default();
    assert_eq!(body["consent"], true);
    assert_eq!(body["stream"], true);
    assert_eq!(body["max_tokens"], 400);
    assert_eq!(body["tool_choice"], "auto");
    let assistant = &body["messages"][2];
    assert_eq!(assistant["content"], Json::Null);
    assert_eq!(
        assistant["tool_calls"][0]["function"]["name"],
        "text__cleaner"
    );
    assert_eq!(assistant["tool_calls"][0]["type"], "function");
    assert_eq!(body["messages"][3]["tool_call_id"], "c1");
    let plain = Request {
        messages: vec![Message::User("u".into())],
        tools: vec![],
        stream: false,
        max_tokens: 10,
    };
    let plain: Json = serde_json::from_str(&plain.body()).unwrap_or_default();
    assert!(plain.get("tools").is_none() && plain.get("tool_choice").is_none());
}

#[test]
fn gateway_errors_are_read_with_a_safe_default() {
    assert_eq!(
        ApiError::parse(429, r#"{"error":"ai_paused","credits":{}}"#),
        ApiError {
            status: 429,
            code: "ai_paused".into()
        }
    );
    assert_eq!(ApiError::parse(502, "<html>").code, "ai_unavailable");
}

#[test]
fn the_parser_handles_events_split_across_network_chunks() {
    let wire = "data: {\"delta\":{\"content\":\"Les\"},\"finish_reason\":null}\n\ndata: {\"delta\":{\"content\":\" chats\"},\"finish_reason\":null}\n\ndata: {\"delta\":{},\"finish_reason\":\"stop\"}\n\nevent: done\ndata: {\"credits\":{\"percent\":97,\"status\":\"ok\",\"resets_in_seconds\":null}}\n\n";
    let mut parser = SseParser::default();
    let mut events = Vec::new();
    // Worst case: one byte at a time, with CRLF line ends.
    for ch in wire.replace('\n', "\r\n").chars() {
        events.extend(parser.feed(&ch.to_string()));
    }
    assert_eq!(events.len(), 4, "{events:?}");
    let mut assembler = Assembler::default();
    for e in &events {
        assembler.push(e);
    }
    assert_eq!(assembler.text(), "Les chats");
    assert_eq!(
        events[3],
        Event::Done(Some(Credits {
            percent: 97,
            status: Status::Ok,
            resets_in_seconds: None
        }))
    );
    let turn = assembler.finish();
    assert_eq!(
        (
            turn.content.as_deref(),
            turn.finish_reason.as_str(),
            turn.tool_calls.len()
        ),
        (Some("Les chats"), "stop", 0)
    );
}

#[test]
fn streamed_tool_calls_are_reassembled_from_fragments() {
    let wire = concat!(
        "data: {\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_a\",\"type\":\"function\",\"function\":{\"name\":\"text__cleaner\",\"arguments\":\"\"}}]},\"finish_reason\":null}\n\n",
        "data: {\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":null,\"function\":{\"name\":null,\"arguments\":\"{\\\"text\\\": \"}}]},\"finish_reason\":null}\n\n",
        "data: {\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"\\\"@texte1\\\"}\"}},{\"index\":1,\"id\":\"call_b\",\"function\":{\"name\":\"text__case-converter\",\"arguments\":\"{}\"}}]},\"finish_reason\":null}\n\n",
        "data: {\"delta\":{},\"finish_reason\":\"tool_calls\"}\n\n",
    );
    let mut assembler = Assembler::default();
    for event in SseParser::default().feed(wire) {
        assembler.push(&event);
    }
    let turn = assembler.finish();
    assert_eq!(turn.finish_reason, "tool_calls");
    assert_eq!(turn.content, None);
    assert_eq!(turn.tool_calls.len(), 2);
    assert_eq!(
        turn.tool_calls[0],
        ToolCall {
            id: "call_a".into(),
            name: "text__cleaner".into(),
            arguments: "{\"text\": \"@texte1\"}".into()
        }
    );
    assert_eq!(turn.tool_calls[1].name, "text__case-converter");
}

#[test]
fn error_events_and_garbage_are_survivable() {
    let mut parser = SseParser::default();
    assert_eq!(
        parser.feed("event: error\ndata: {\"error\":\"ai_unavailable\"}\n\n"),
        vec![Event::Error("ai_unavailable".into())]
    );
    assert!(parser.feed("data: not json\n\n").is_empty());
    assert!(parser.feed(": keep-alive\n\n").is_empty());
}

#[test]
fn a_non_streaming_answer_gives_the_same_turn() {
    let body = r#"{"message":{"role":"assistant","content":null,"tool_calls":[{"id":"call_1","type":"function","function":{"name":"text__cleaner","arguments":"{\"text\":\"@texte1\"}"}}]},"finish_reason":"tool_calls","credits":{"percent":88,"status":"ok","resets_in_seconds":null}}"#;
    let (turn, credits) = turn_from_response(body).unwrap_or_default();
    assert_eq!(turn.tool_calls[0].name, "text__cleaner");
    assert_eq!(turn.finish_reason, "tool_calls");
    assert_eq!(credits.map(|c| c.percent), Some(88));
    assert!(turn_from_response("nope").is_none());
}

#[test]
fn credits_say_when_to_wait_and_when_the_ai_is_back() {
    let paused =
        Credits::from_json(&json!({"percent": 40, "status": "paused", "resets_in_seconds": 3_600}))
            .unwrap_or_else(|| unreachable!());
    assert!(!paused.can_ask());
    assert_eq!(paused.back_at_utc(10 * 3600), Some("11:00".into()));
    assert!(
        Credits::from_json(&json!({"percent": 5, "status": "low"})).is_some_and(|c| c.can_ask())
    );
    assert!(Credits::from_json(&json!({"percent": 5, "status": "weird"})).is_none());
}

#[test]
fn tool_names_roundtrip_and_definitions_come_from_contracts() {
    assert_eq!(function_name("text.case-converter"), "text__case-converter");
    assert_eq!(tool_id("text__case-converter"), "text.case-converter");
    let all = contracts();
    let def = definition(&all["text.case-converter"], "fr");
    assert_eq!(def["type"], "function");
    assert_eq!(def["function"]["name"], "text__case-converter");
    let parameters = &def["function"]["parameters"];
    assert!(parameters.get("$schema").is_none());
    assert_eq!(parameters["properties"]["case"]["enum"][0], "upper");
    let text = parameters["properties"]["text"]["description"]
        .as_str()
        .unwrap_or_default();
    assert!(text.contains("@texteN") && text.contains("@cN"), "{text}");
    assert!(
        !def.to_string().contains("x-kortex"),
        "contract annotations are stripped"
    );
    assert!(!definition(&all["pdf.merge"], "en")["function"]["parameters"]["properties"]["files"]["description"].as_str().unwrap_or_default().is_empty());
}

#[test]
fn references_parse_strictly() {
    assert_eq!(parse_reference("@texte3"), Some(Reference::Text(3)));
    assert_eq!(parse_reference(" @fichier12 "), Some(Reference::File(12)));
    assert_eq!(
        parse_reference("@c2.text"),
        Some(Reference::Result {
            call: "c2".into(),
            port: "text".into()
        })
    );
    for bad in [
        "texte1",
        "@texte0",
        "@texte",
        "@c0.text",
        "@call_0.text",
        "@c1.",
        "@c1.a b",
        "@x1.text",
        "bonjour",
    ] {
        assert_eq!(parse_reference(bad), None, "{bad}");
    }
}

#[test]
fn a_call_by_reference_resolves_to_the_real_text() {
    let attachments = [Attachment::text(1, "  a   b ")];
    let planned = plan(
        &call("c1", "text.cleaner", r#"{"text":"@texte1"}"#),
        &attachments,
        &BTreeMap::new(),
    )
    .unwrap_or_else(|e| unreachable!("{e:?}"));
    assert_eq!(planned.tool, "text.cleaner");
    assert_eq!(
        planned.inputs["text"],
        Value::Text("  a   b ".into()),
        "the exact text, trailing spaces included"
    );
    assert_eq!(planned.refs, [("text".to_owned(), Reference::Text(1))]);
    assert!(planned.params.is_empty() && planned.confirmations.is_empty());
    // A literal is accepted for a text input (the user sees the plan before it runs).
    let literal = plan(
        &call("c1", "text.cleaner", r#"{"text":"salut"}"#),
        &[],
        &BTreeMap::new(),
    )
    .unwrap_or_else(|e| unreachable!("{e:?}"));
    assert_eq!(literal.inputs["text"], Value::Text("salut".into()));
    assert!(literal.refs.is_empty());
}

#[test]
fn a_second_call_reads_the_first_result_by_its_short_id() {
    let results = BTreeMap::from([(
        "c1".to_owned(),
        ValueMap::from([("text".to_owned(), Value::Text("bonjour le monde".into()))]),
    )]);
    let planned = plan(
        &call(
            "c2",
            "text.case-converter",
            r#"{"text":"@c1.text","case":"upper"}"#,
        ),
        &[],
        &results,
    )
    .unwrap_or_else(|e| unreachable!("{e:?}"));
    assert_eq!(
        planned.inputs["text"],
        Value::Text("bonjour le monde".into())
    );
    assert_eq!(planned.params["case"], Value::Text("upper".into()));
    assert_eq!(
        planned.refs,
        [(
            "text".to_owned(),
            Reference::Result {
                call: "c1".into(),
                port: "text".into()
            }
        )]
    );
}

#[test]
fn every_kind_of_bad_call_is_refused_with_a_code_the_model_can_act_on() {
    let attachments = [Attachment::text(1, "x")];
    let results = BTreeMap::from([(
        "c1".to_owned(),
        ValueMap::from([("text".to_owned(), Value::Text("t".into()))]),
    )]);
    let code = |c: ToolCall| {
        plan(&c, &attachments, &results)
            .err()
            .map(|e| (e.code, e.detail))
    };
    assert_eq!(
        code(call("c2", "text.nope", "{}")),
        Some(("unknown_tool", "text.nope".into()))
    );
    assert_eq!(
        code(call("c2", "text.cleaner", "not json")).map(|e| e.0),
        Some("invalid_arguments")
    );
    assert_eq!(
        code(call("c2", "text.cleaner", "[1]")).map(|e| e.0),
        Some("invalid_arguments")
    );
    assert_eq!(
        code(call(
            "c2",
            "text.cleaner",
            r#"{"text":"@texte1","bogus":1}"#
        )),
        Some(("unknown_argument", "bogus".into()))
    );
    assert_eq!(
        code(call("c2", "text.cleaner", "{}")),
        Some(("missing_input", "text".into()))
    );
    assert_eq!(
        code(call("c2", "text.cleaner", r#"{"text":"@texte9"}"#)),
        Some(("unknown_reference", "@texte9".into()))
    );
    assert_eq!(
        code(call("c2", "text.cleaner", r#"{"text":"@call_0.text"}"#)),
        Some(("unknown_reference", "@call_0.text".into())),
        "an invented reference is not literal text"
    );
    assert!(
        code(call(
            "c2",
            "text.cleaner",
            r#"{"text":"écris à @alice demain"}"#
        ))
        .is_none(),
        "a sentence containing an @ is plain text"
    );
    assert_eq!(
        code(call("c2", "text.cleaner", r#"{"text":"@c1.missing"}"#)),
        Some(("unknown_reference", "@c1.missing".into()))
    );
    assert_eq!(
        code(call(
            "c2",
            "text.case-converter",
            r#"{"text":"@texte1","case":"shout"}"#
        ))
        .map(|e| e.0),
        Some("invalid_param")
    );
    assert_eq!(
        code(call(
            "c2",
            "text.case-converter",
            r#"{"text":"@texte1","case":3}"#
        ))
        .map(|e| e.0),
        Some("invalid_param")
    );
    assert_eq!(
        code(call("c2", "text.cleaner", r#"{"text":5}"#)).map(|e| e.0),
        Some("invalid_argument")
    );
}

#[test]
fn numeric_params_are_checked_against_the_contract_range() {
    let c = contracts();
    let qr = &c["qr.generator"];
    let size = qr
        .params
        .iter()
        .find(|p| p.id == "size")
        .unwrap_or_else(|| unreachable!("size param"));
    let range = size.range.clone().unwrap_or_default();
    let ok = plan(
        &call(
            "c1",
            "qr.generator",
            &format!(r#"{{"content":"https://kortex.site","size":{}}}"#, range[0]),
        ),
        &[],
        &BTreeMap::new(),
    );
    assert!(ok.is_ok(), "{ok:?}");
    let too_big = plan(
        &call(
            "c1",
            "qr.generator",
            &format!(r#"{{"content":"x","size":{}}}"#, range[1] + 1.0),
        ),
        &[],
        &BTreeMap::new(),
    );
    assert_eq!(too_big.err().map(|e| e.code), Some("invalid_param"));
    let float = plan(
        &call("c1", "qr.generator", r#"{"content":"x","size":128.5}"#),
        &[],
        &BTreeMap::new(),
    );
    assert_eq!(
        float.err().map(|e| e.code),
        Some("invalid_param"),
        "an integer param refuses a fraction"
    );
}

#[test]
fn file_inputs_need_references_and_batches_take_several() {
    let attachments = [
        file(1, "a.pdf", "application/pdf", b"%PDF-1.4 a"),
        file(2, "b.pdf", "application/pdf", b"%PDF-1.4 b"),
    ];
    let merged = plan(
        &call("c1", "pdf.merge", r#"{"files":["@fichier1","@fichier2"]}"#),
        &attachments,
        &BTreeMap::new(),
    )
    .unwrap_or_else(|e| unreachable!("{e:?}"));
    let Value::Files(files) = &merged.inputs["files"] else {
        unreachable!("a batch")
    };
    assert_eq!(
        files.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(),
        ["a.pdf", "b.pdf"]
    );
    assert_eq!(merged.refs.len(), 2);
    let literal = plan(
        &call("c1", "pdf.merge", r#"{"files":["a.pdf"]}"#),
        &attachments,
        &BTreeMap::new(),
    );
    assert_eq!(
        literal.err().map(|e| e.code),
        Some("reference_required"),
        "the model cannot invent file content"
    );
    let single = plan(
        &call("c1", "pdf.merge", r#"{"files":"@fichier1"}"#),
        &attachments,
        &BTreeMap::new(),
    )
    .unwrap_or_else(|e| unreachable!("{e:?}"));
    assert!(matches!(&single.inputs["files"], Value::Files(f) if f.len() == 1));
}

#[test]
fn sensitive_network_and_write_tools_require_extra_confirmation() {
    let mut all = contracts();
    let tool = all
        .get_mut("text.cleaner")
        .unwrap_or_else(|| unreachable!("tool"));
    tool.permissions.sensitive = true;
    tool.permissions.network = true;
    tool.permissions.filesystem = "write".into();
    let attachments = [Attachment::text(1, "x")];
    let planned = plan_call(
        &call("c1", "text.cleaner", r#"{"text":"@texte1"}"#),
        &Context {
            attachments: &attachments,
            results: &BTreeMap::new(),
            contracts: &all,
        },
    )
    .unwrap_or_else(|e| unreachable!("{e:?}"));
    assert_eq!(planned.confirmations, ["sensitive", "network", "write"]);
}

#[test]
fn feedback_lists_the_references_the_model_may_use() {
    let attachments = [
        Attachment::text(1, "x"),
        file(1, "a.pdf", "application/pdf", b"%PDF"),
    ];
    let results = BTreeMap::from([(
        "c1".to_owned(),
        ValueMap::from([("text".to_owned(), Value::Text("t".into()))]),
    )]);
    let all = contracts();
    let ctx = Context {
        attachments: &attachments,
        results: &results,
        contracts: &all,
    };
    let valid = ctx.valid_references();
    assert_eq!(valid, ["@texte1", "@fichier1", "@c1.text"]);
    let error = plan_call(
        &call("c2", "text.cleaner", r#"{"text":"@call_0.text"}"#),
        &ctx,
    )
    .err()
    .unwrap_or_else(|| unreachable!("refused"));
    let feedback = error.feedback(&valid);
    assert!(
        feedback.contains("@c1.text") && feedback.contains("unknown_reference"),
        "{feedback}"
    );
}

#[test]
fn tool_results_go_back_to_the_model_as_references_never_as_data() {
    let outputs = ValueMap::from([
        ("text".to_owned(), Value::Text("très secret".into())),
        ("count".to_owned(), Value::Int(3)),
        ("flag".to_owned(), Value::Bool(true)),
        (
            "file".to_owned(),
            Value::File(kortex_runtime::FileData {
                name: "o.png".into(),
                mime: "image/png".into(),
                bytes: vec![1, 2, 3],
            }),
        ),
        (
            "table".to_owned(),
            Value::Table(Table {
                columns: vec!["a".into()],
                rows: vec![vec!["secret".into()]],
            }),
        ),
    ]);
    let summary = summarize("c1", &outputs);
    assert!(!summary.contains("secret"), "{summary}");
    let v: Json = serde_json::from_str(&summary).unwrap_or_default();
    assert!(
        v["text"]
            .as_str()
            .unwrap_or_default()
            .starts_with("@c1.text (texte de 11 caractères)")
    );
    assert_eq!(v["count"], "3");
    assert!(v["file"].as_str().unwrap_or_default().contains("3 octets"));
}

#[test]
fn the_system_prompt_teaches_references_and_exposes_metadata_only() {
    let mut text = Attachment::text(1, "mot de passe du wifi");
    let prompt_fr = prompt::system("fr", &[text.clone()]);
    assert!(prompt_fr.contains("@texte1") && prompt_fr.contains("UNIQUEMENT une référence"));
    assert!(
        !prompt_fr.contains("wifi"),
        "content is not shared by default"
    );
    text.share_content = true;
    assert!(
        prompt::system("en", &[text.clone()]).contains("mot de passe du wifi"),
        "shared on purpose"
    );
    assert!(prompt::system("en", &[]).contains("No attachments."));
    assert!(describe_all(&[], "fr").is_empty());
}

#[test]
fn the_executed_calls_export_as_a_valid_flow_workflow() {
    let attachments = [Attachment::text(1, "  Bonjour   le monde ")];
    let executed = [
        Executed {
            call_id: "c1".into(),
            tool: "text.cleaner".into(),
            params: ValueMap::new(),
            refs: vec![("text".into(), Reference::Text(1))],
        },
        Executed {
            call_id: "c2".into(),
            tool: "text.case-converter".into(),
            params: ValueMap::from([("case".to_owned(), Value::Text("upper".into()))]),
            refs: vec![(
                "text".into(),
                Reference::Result {
                    call: "c1".into(),
                    port: "text".into(),
                },
            )],
        },
    ];
    let workflow = to_workflow("Mind", &executed, &attachments);
    assert_eq!(
        workflow
            .nodes
            .iter()
            .map(|n| n.id.as_str())
            .collect::<Vec<_>>(),
        ["t1", "c1", "c2"]
    );
    assert_eq!(workflow.edges.len(), 2);
    let all: Vec<ToolContract> = contracts().into_values().collect();
    assert_eq!(
        validate(&workflow, &catalogue_of(&all)).len(),
        0,
        "Flow accepts the exported plan"
    );
    let json = kortex_workflow::to_json(&workflow);
    assert_eq!(kortex_workflow::from_json(&json), Ok(workflow));
}

#[test]
fn file_attachments_export_as_file_nodes_without_their_bytes() {
    use kortex_ai::consent::Kind;
    let pdf = |n: usize, name: &str| Attachment {
        n,
        kind: Kind::File,
        name: name.into(),
        mime: "application/pdf".into(),
        bytes: b"%PDF-secret".to_vec(),
        share_content: false,
    };
    let attachments = [pdf(1, "a.pdf"), pdf(2, "b.pdf")];
    let executed = [Executed {
        call_id: "c1".into(),
        tool: "pdf.merge".into(),
        params: ValueMap::new(),
        refs: vec![
            ("files".into(), Reference::File(1)),
            ("files".into(), Reference::File(2)),
        ],
    }];
    let workflow = to_workflow("Mind", &executed, &attachments);
    let ids: Vec<&str> = workflow.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(
        ids,
        ["b1", "c1"],
        "two files on one input form a batch node"
    );
    assert_eq!(workflow.edges.len(), 1, "one link per input");
    let all: Vec<ToolContract> = contracts().into_values().collect();
    assert_eq!(
        validate(&workflow, &catalogue_of(&all)).len(),
        0,
        "Flow accepts it"
    );
    let json = kortex_workflow::to_json(&workflow);
    assert!(
        !json.contains("secret") && !json.contains("a.pdf"),
        "no file content or name"
    );
    // A lone file on a batch input becomes a single-file node, which Flow also accepts.
    let single = [Executed {
        call_id: "c1".into(),
        tool: "pdf.merge".into(),
        params: ValueMap::new(),
        refs: vec![("files".into(), Reference::File(1))],
    }];
    let workflow = to_workflow("Mind", &single, &attachments);
    assert_eq!(workflow.nodes[0].id, "f1");
    assert_eq!(validate(&workflow, &catalogue_of(&all)).len(), 0);
    // Mixed families fall back to any file; the node is still valid for a `*/*` input only.
    let image = Attachment {
        n: 3,
        mime: "image/png".into(),
        ..pdf(3, "c.png")
    };
    let mixed = [pdf(1, "a.pdf"), image];
    let shared = to_workflow(
        "Mind",
        &[Executed {
            call_id: "c1".into(),
            tool: "pdf.merge".into(),
            params: ValueMap::new(),
            refs: vec![
                ("files".into(), Reference::File(1)),
                ("files".into(), Reference::File(3)),
            ],
        }],
        &mixed,
    );
    match &shared.nodes[0].kind {
        kortex_workflow::NodeKind::File { accept, multiple } => {
            assert_eq!((accept.as_str(), *multiple), ("*/*", true))
        }
        other => unreachable!("{other:?}"),
    }
}
