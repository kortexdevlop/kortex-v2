//! The shipped connectors: contracts, ports and the requests they build.
use std::collections::BTreeMap;

use kortex_flow::catalogue::{connector_tools, ports};
use kortex_flow::connectors::{all, find};
use kortex_workflow::connector::expand;

fn secrets(name: &str) -> Option<String> {
    match name {
        "TELEGRAM_TOKEN" => Some("123:ABC".to_owned()),
        "GITHUB_TOKEN" => Some("ghp_test".to_owned()),
        "DISCORD_WEBHOOK" => Some("https://discord.com/api/webhooks/1/abc".to_owned()),
        _ => None,
    }
}
fn values(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

#[test]
fn the_four_connectors_are_valid_and_every_action_is_a_catalogue_tool() {
    let ids: Vec<&str> = all().iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["webhook", "telegram", "discord", "github"]);
    let tools = connector_tools("fr");
    assert_eq!(tools.len(), 6);
    for (id, name) in &tools {
        assert!(ports().contains_key(id), "{id} is in the catalogue");
        assert!(name.contains(" — "), "{name}");
    }
    let (_, send) = find("connector.telegram.send_message").expect("action");
    let p = send.ports();
    assert_eq!(
        p.inputs.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
        ["chat_id", "text"]
    );
    assert!(
        p.inputs.iter().all(|i| !i.required),
        "inputs are optional: a value can be typed instead"
    );
    assert_eq!(
        p.outputs.iter().map(|o| o.id.as_str()).collect::<Vec<_>>(),
        ["text", "status"]
    );
}

#[test]
fn telegram_builds_a_json_body_with_the_token_in_the_url_only() {
    let (_, action) = find("connector.telegram.send_message").expect("action");
    let request = expand(
        action,
        &values(&[
            ("chat_id", "42"),
            ("text", "say \"hi\"\nline2 \\ {secret:TELEGRAM_TOKEN}"),
        ]),
        &secrets,
    )
    .expect("request");
    assert_eq!(request.method, "POST");
    assert_eq!(
        request.url,
        "https://api.telegram.org/bot123:ABC/sendMessage"
    );
    let body: serde_json::Value =
        serde_json::from_str(request.body.as_deref().expect("body")).expect("valid JSON");
    assert_eq!(body["chat_id"], "42");
    assert_eq!(
        body["text"], "say \"hi\"\nline2 \\ {secret:TELEGRAM_TOKEN}",
        "the value is data: a placeholder in it stays text"
    );
    assert!(
        !request.body.as_deref().unwrap_or("").contains("123:ABC"),
        "the token never enters the body"
    );
}

#[test]
fn github_encodes_path_values_and_sends_the_token_as_a_header() {
    let (_, action) = find("connector.github.get_repo").expect("action");
    let request = expand(
        action,
        &values(&[("owner", "a b"), ("repo", "r/../x")]),
        &secrets,
    )
    .expect("request");
    assert_eq!(request.url, "https://api.github.com/repos/a%20b/r%2F..%2Fx");
    assert!(
        request
            .headers
            .contains(&("authorization".to_owned(), "Bearer ghp_test".to_owned()))
    );
    assert!(request.body.is_none());
    let (_, create) = find("connector.github.create_issue").expect("action");
    let issue = expand(
        create,
        &values(&[("owner", "o"), ("repo", "r"), ("title", "Bug: é\u{1}")]),
        &secrets,
    )
    .expect("request");
    let body: serde_json::Value =
        serde_json::from_str(issue.body.as_deref().expect("body")).expect("JSON");
    assert_eq!(
        (body["title"].as_str(), body["body"].as_str()),
        (Some("Bug: é\u{1}"), Some(""))
    );
}

#[test]
fn discord_and_webhook_obey_the_same_url_policy_as_the_http_node() {
    let (_, discord) = find("connector.discord.send_message").expect("action");
    let ok = expand(discord, &values(&[("content", "hello")]), &secrets).expect("request");
    assert_eq!(ok.url, "https://discord.com/api/webhooks/1/abc");
    let private =
        |name: &str| (name == "DISCORD_WEBHOOK").then(|| "https://192.168.1.1/hook".to_owned());
    assert_eq!(
        expand(discord, &values(&[("content", "x")]), &private)
            .expect_err("private")
            .code,
        "host_not_allowed"
    );
    let (_, post) = find("connector.webhook.post_json").expect("action");
    let sent = expand(
        post,
        &values(&[
            ("url", "https://hooks.example.org/x"),
            ("json", "{\"a\":1}"),
        ]),
        &secrets,
    )
    .expect("request");
    assert_eq!(sent.body.as_deref(), Some("{\"a\":1}"));
    assert_eq!(
        expand(
            post,
            &values(&[("url", "http://hooks.example.org/x"), ("json", "{}")]),
            &secrets
        )
        .expect_err("http")
        .code,
        "https_only"
    );
    assert_eq!(
        expand(
            post,
            &values(&[("url", "https://localhost/x"), ("json", "{}")]),
            &secrets
        )
        .expect_err("localhost")
        .code,
        "host_not_allowed"
    );
}

#[test]
fn missing_values_and_secrets_are_reported_by_name() {
    let (_, action) = find("connector.telegram.send_message").expect("action");
    let e = expand(action, &values(&[("chat_id", "42")]), &secrets).expect_err("no text");
    assert_eq!(
        (e.code.as_str(), e.detail.as_deref()),
        ("missing_param", Some("text"))
    );
    let none = |_: &str| None;
    let e =
        expand(action, &values(&[("chat_id", "42"), ("text", "x")]), &none).expect_err("no token");
    assert_eq!(
        (e.code.as_str(), e.detail.as_deref()),
        ("missing_secret", Some("TELEGRAM_TOKEN"))
    );
}
