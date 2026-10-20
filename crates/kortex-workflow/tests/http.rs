//! The HTTP node: URL policy, headers, secrets and the job a host receives.
#![allow(clippy::result_large_err)]
use std::collections::BTreeMap;

use kortex_runtime::{CancelToken, Value, ValueMap};
use kortex_workflow::http::{
    HTTP_TOOL, build_request, check_url, outputs, parse_headers, substitute,
};
use kortex_workflow::{
    Edge, Endpoint, Execution, Node, NodeKind, Step, ToolPorts, Workflow, validate,
};

fn text(map: &[(&str, &str)]) -> ValueMap {
    map.iter()
        .map(|(k, v)| ((*k).to_owned(), Value::Text((*v).to_owned())))
        .collect()
}
fn secrets(name: &str) -> Option<String> {
    (name == "TOKEN").then(|| "s3cret".to_owned())
}

#[test]
fn only_public_https_hosts_pass_the_policy() {
    for ok in [
        "https://api.github.com/repos/o/r",
        "https://example.org:443/x?q=1#f",
        "https://sub.domain.example.co.uk/",
        "https://api.telegram.org/bot{secret}/sendMessage",
    ] {
        assert_eq!(check_url(ok), Ok(()), "{ok}");
    }
    for (bad, code) in [
        ("http://example.org/", "https_only"),
        ("ftp://example.org/", "https_only"),
        ("https://127.0.0.1/", "host_not_allowed"),
        ("https://[::1]/", "host_not_allowed"),
        ("https://169.254.169.254/latest", "host_not_allowed"),
        ("https://2130706433/", "host_not_allowed"),
        ("https://0x7f.1/", "host_not_allowed"),
        ("https://localhost/", "host_not_allowed"),
        ("https://intranet/", "host_not_allowed"),
        ("https://db.internal/", "host_not_allowed"),
        ("https://nas.local/", "host_not_allowed"),
        ("https://kortex.site/v1/me", "host_not_allowed"),
        ("https://api.kortex.site/", "host_not_allowed"),
        ("https://example.org:8443/", "port_not_allowed"),
        ("https://user:pw@example.org/", "credentials_in_url"),
        ("https://exa mple.org/", "invalid_url"),
        ("", "invalid_url"),
    ] {
        assert_eq!(check_url(bad), Err(code), "{bad}");
    }
}

#[test]
fn headers_are_limited_to_the_known_safe_names() {
    let ok =
        parse_headers("Authorization: Bearer x\n\nContent-Type: application/json").expect("ok");
    assert_eq!(ok[0], ("authorization".to_owned(), "Bearer x".to_owned()));
    for bad in ["Cookie: a=b", "Host: evil", "NoColon", "X-Anything: 1"] {
        assert!(parse_headers(bad).is_err(), "{bad}");
    }
    let many = (0..13).map(|_| "accept: a").collect::<Vec<_>>().join("\n");
    assert_eq!(
        parse_headers(&many).expect_err("too many").code,
        "too_many_headers"
    );
}

#[test]
fn secrets_are_placeholders_resolved_only_where_the_user_wrote_them() {
    assert_eq!(
        substitute("Bearer {secret:TOKEN}", &secrets).as_deref(),
        Ok("Bearer s3cret")
    );
    assert_eq!(
        substitute("a {b} {secret:} {secret:ok", &secrets).as_deref(),
        Ok("a {b} {secret:} {secret:ok")
    );
    assert_eq!(
        substitute("{secret:NOPE}", &secrets)
            .expect_err("missing")
            .code,
        "missing_secret"
    );
    let request = build_request(
        &ValueMap::new(),
        &text(&[
            ("method", "POST"),
            (
                "url",
                "https://api.telegram.org/bot{secret:TOKEN}/sendMessage",
            ),
            ("headers", "Authorization: Bearer {secret:TOKEN}"),
            ("body", "{\"chat\":1}"),
        ]),
        &secrets,
    )
    .expect("request");
    assert_eq!(
        request.url,
        "https://api.telegram.org/bots3cret/sendMessage"
    );
    assert_eq!(request.headers[0].1, "Bearer s3cret");
    // Text arriving from another node is data: a placeholder in it is never resolved.
    let from_node = build_request(
        &text(&[("body", "leak {secret:TOKEN}")]),
        &text(&[("method", "POST"), ("url", "https://example.org/")]),
        &secrets,
    )
    .expect("request");
    assert_eq!(from_node.body.as_deref(), Some("leak {secret:TOKEN}"));
}

#[test]
fn a_request_is_refused_before_it_is_built_when_it_breaks_the_rules() {
    let p = |extra: &[(&str, &str)]| {
        let mut base = vec![("method", "GET"), ("url", "https://example.org/")];
        base.extend_from_slice(extra);
        text(&base)
    };
    assert!(build_request(&ValueMap::new(), &p(&[]), &secrets).is_ok());
    assert_eq!(
        build_request(&ValueMap::new(), &p(&[("method", "TRACE")]), &secrets)
            .expect_err("m")
            .code,
        "bad_method"
    );
    assert_eq!(
        build_request(
            &ValueMap::new(),
            &p(&[("url", "https://10.0.0.1/")]),
            &secrets
        )
        .expect_err("u")
        .code,
        "host_not_allowed"
    );
    assert_eq!(
        build_request(&ValueMap::new(), &p(&[("body", "x")]), &secrets)
            .expect_err("b")
            .code,
        "body_not_allowed"
    );
    // The URL from another node is checked like the typed one.
    let from_node = build_request(&text(&[("url", "http://example.org/")]), &p(&[]), &secrets);
    assert_eq!(from_node.expect_err("u").code, "https_only");
}

#[test]
fn a_response_becomes_text_and_status_and_errors_can_fail_the_node() {
    let ok = outputs(201, b"created", true).expect("ok");
    assert_eq!(ok["text"], Value::Text("created".into()));
    assert_eq!(ok["status"], Value::Text("201".into()));
    let failure = outputs(404, b"nope", true).expect_err("404");
    assert_eq!(
        (failure.code.as_str(), failure.detail.as_deref()),
        ("http_status", Some("404"))
    );
    assert!(
        outputs(404, b"nope", false).is_ok(),
        "fail_on_error can be turned off"
    );
}

#[test]
fn the_http_node_is_a_host_job_validated_like_any_node_and_skipped_with_a_missing_feeder() {
    let node = |id: &str, kind: NodeKind| Node {
        id: id.into(),
        kind,
        retries: 0,
        pos: None,
    };
    let http = |params: &[(&str, &str)]| {
        node(
            "h",
            NodeKind::Logic {
                op: "http".into(),
                params: params
                    .iter()
                    .map(|(k, v)| ((*k).into(), (*v).into()))
                    .collect(),
            },
        )
    };
    let flow = |nodes: Vec<Node>, edges: Vec<Edge>| Workflow {
        nodes,
        edges,
        ..Workflow::default()
    };
    let cat: BTreeMap<String, ToolPorts> = BTreeMap::new();
    // Invalid URL and header are reported by validation, before anything runs.
    let bad = flow(
        vec![http(&[("url", "http://x.org/"), ("headers", "Cookie: a")])],
        vec![],
    );
    let codes: Vec<_> = validate(&bad, &cat)
        .iter()
        .map(|i| (i.code, i.detail.clone()))
        .collect();
    assert_eq!(
        codes,
        [
            ("bad_param", "url".to_owned()),
            ("bad_param", "headers".to_owned())
        ]
    );
    // A placeholder inside the path is fine at validation time.
    let ok = flow(
        vec![http(&[("url", "https://api.telegram.org/bot{secret:T}/x")])],
        vec![],
    );
    assert!(validate(&ok, &cat).is_empty());
    let mut exec = Execution::new(&ok, &cat).expect("valid");
    match exec.next(&CancelToken::default()) {
        Step::Run(job) => {
            assert_eq!(job.tool, HTTP_TOOL);
            assert_eq!(job.params["method"], Value::Text("GET".into()));
            assert_eq!(job.params["fail_on_error"], Value::Bool(true));
        }
        other => panic!("expected the http job, got {other:?}"),
    }
    // Behind a condition branch that was not taken, the request is skipped, not sent.
    let skipped = flow(
        vec![
            node("t", NodeKind::Text { value: "x".into() }),
            node(
                "c",
                NodeKind::Logic {
                    op: "condition".into(),
                    params: [
                        ("test".to_owned(), "equals".to_owned()),
                        ("value".to_owned(), "never".to_owned()),
                    ]
                    .into(),
                },
            ),
            http(&[("method", "POST"), ("url", "https://example.org/")]),
        ],
        vec![
            Edge {
                from: Endpoint {
                    node: "t".into(),
                    port: "text".into(),
                },
                to: Endpoint {
                    node: "c".into(),
                    port: "text".into(),
                },
            },
            Edge {
                from: Endpoint {
                    node: "c".into(),
                    port: "then".into(),
                },
                to: Endpoint {
                    node: "h".into(),
                    port: "body".into(),
                },
            },
        ],
    );
    let mut exec = Execution::new(&skipped, &cat).expect("valid");
    match exec.next(&CancelToken::default()) {
        Step::Done(report) => assert_eq!(report.skipped, vec!["h".to_owned()]),
        other => panic!("the request must be skipped, got {other:?}"),
    }
}

#[test]
fn base64_bodies_from_the_proxy_decode_exactly() {
    use kortex_workflow::http::decode_base64;
    assert_eq!(decode_base64("aGVsbG8="), Some(b"hello".to_vec()));
    assert_eq!(decode_base64("aGk="), Some(b"hi".to_vec()));
    assert_eq!(decode_base64(""), Some(Vec::new()));
    assert_eq!(decode_base64("a*b="), None);
    let all: Vec<u8> = (0..=255).collect();
    let encoded = {
        const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut s = String::new();
        for chunk in all.chunks(3) {
            let n = chunk
                .iter()
                .enumerate()
                .fold(0_u32, |acc, (i, b)| acc | (u32::from(*b) << (16 - 8 * i)));
            for i in 0..=chunk.len() {
                s.push(T[((n >> (18 - 6 * i)) & 63) as usize] as char);
            }
            for _ in chunk.len()..3 {
                s.push('=');
            }
        }
        s
    };
    assert_eq!(decode_base64(&encoded), Some(all));
}

#[test]
fn connector_contracts_are_validated_when_they_are_read() {
    use kortex_workflow::connector::{parse, split_tool};
    let base = |action: &str| {
        format!(
            "schema = 1\n[connector]\nid = \"demo\"\n[connector.name]\nfr = \"D\"\nen = \"D\"\n[[actions]]\nid = \"go\"\nmethod = \"POST\"\n{action}\n[actions.name]\nfr = \"A\"\nen = \"A\"\n[[actions.params]]\nid = \"x\"\n[actions.params.label]\nfr = \"X\"\nen = \"X\"\n"
        )
    };
    assert!(parse(&base("url = \"https://example.org/{x|url}\"")).is_ok());
    for (bad, why) in [
        ("url = \"http://example.org/\"", "https"),
        (
            "url = \"https://example.org/{y}\"",
            "undeclared placeholder",
        ),
        ("url = \"https://example.org/{x|shout}\"", "unknown filter"),
    ] {
        assert!(parse(&base(bad)).is_err(), "{why}");
    }
    assert!(
        parse(&base("url = \"https://e.org/\"").replace("POST", "TRACE")).is_err(),
        "method"
    );
    assert!(
        parse(&base("url = \"https://e.org/\"").replace("fr = \"A\"\n", "")).is_err(),
        "names need fr and en"
    );
    assert_eq!(
        split_tool("connector.telegram.send_message"),
        Some(("telegram", "send_message"))
    );
    assert_eq!(split_tool("text.cleaner"), None);
}

#[test]
fn the_ai_node_is_a_host_job_and_its_tools_must_be_text_tools() {
    use kortex_contract::PortType;
    use kortex_workflow::{AI_TOOL, Port};
    let port = |id: &str, ty: PortType, required: bool| Port {
        id: id.into(),
        ty,
        required,
    };
    let cat: BTreeMap<String, ToolPorts> = BTreeMap::from([
        (
            "text.cleaner".to_owned(),
            ToolPorts {
                inputs: vec![port("text", PortType::Text, true)],
                params: vec![],
                outputs: vec![port("text", PortType::Text, false)],
            },
        ),
        (
            "pdf.merge".to_owned(),
            ToolPorts {
                inputs: vec![port(
                    "files",
                    PortType::Files("application/pdf".into()),
                    true,
                )],
                params: vec![],
                outputs: vec![port(
                    "file",
                    PortType::File("application/pdf".into()),
                    false,
                )],
            },
        ),
    ]);
    let ai = |params: &[(&str, &str)]| Node {
        id: "a".into(),
        kind: NodeKind::Logic {
            op: "ai".into(),
            params: params
                .iter()
                .map(|(k, v)| ((*k).into(), (*v).into()))
                .collect(),
        },
        retries: 0,
        pos: None,
    };
    let flow = |n: Node| Workflow {
        nodes: vec![n],
        ..Workflow::default()
    };
    assert!(
        validate(
            &flow(ai(&[("task", "summarize"), ("tools", "text.cleaner")])),
            &cat
        )
        .is_empty()
    );
    let codes = |w: &Workflow| {
        validate(w, &cat)
            .iter()
            .map(|i| (i.code, i.detail.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        codes(&flow(ai(&[("tools", "pdf.merge")]))),
        [("bad_param", "tools".to_owned())]
    );
    assert_eq!(
        codes(&flow(ai(&[("tools", "no.such")]))),
        [("bad_param", "tools".to_owned())]
    );
    assert_eq!(
        codes(&flow(ai(&[("task", "dance")]))),
        [("bad_param", "task".to_owned())]
    );
    let workflow = flow(ai(&[("prompt", "hi"), ("format", "json")]));
    let mut exec = Execution::new(&workflow, &cat).expect("valid");
    match exec.next(&CancelToken::default()) {
        Step::Run(job) => {
            assert_eq!(job.tool, AI_TOOL);
            assert_eq!(job.params["format"], Value::Text("json".into()));
            assert_eq!(job.params["max_steps"], Value::Int(4));
        }
        other => panic!("expected the AI job, got {other:?}"),
    }
}
