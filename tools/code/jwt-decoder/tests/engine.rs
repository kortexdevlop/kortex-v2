use kortex::runtime::{Cx, block_on};
use tool_code_jwt_decoder::{Inputs, Params, engine::run};

// The well-known jwt.io example token.
const TOKEN: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT32fCcMKJ-gLU5xT5S6TNe8WqVM";

fn decode(token: &str, pretty: bool) -> Result<tool_code_jwt_decoder::Outputs, &'static str> {
    block_on(run(
        Inputs {
            token: token.into(),
        },
        Params { pretty },
        &mut Cx::new(),
    ))
    .map_err(|e| e.code)
}

#[test]
fn nominal_decodes_header_payload_algorithm_and_issue_date() {
    let out = decode(TOKEN, false).expect("decoded");
    assert_eq!(out.header, r#"{"alg":"HS256","typ":"JWT"}"#);
    assert_eq!(
        out.payload,
        r#"{"sub":"1234567890","name":"John Doe","iat":1516239022}"#
    );
    assert_eq!(out.algorithm, "HS256");
    assert_eq!(out.issued_at, "2018-01-18T01:30:22.000Z");
    assert_eq!(out.expires_at, "", "no exp claim");
}

#[test]
fn pretty_printing_and_a_bearer_prefix() {
    let out = decode(&format!("  Bearer {TOKEN}\n"), true).expect("decoded");
    assert!(
        out.payload.contains("\n  \"name\": \"John Doe\""),
        "{}",
        out.payload
    );
    assert_eq!(out.algorithm, "HS256");
}

#[test]
fn the_expiry_claim_is_shown_as_a_utc_date() {
    // {"alg":"none"} . {"exp":1893456000} . (unsecured token, empty signature)
    let token = "eyJhbGciOiJub25lIn0.eyJleHAiOjE4OTM0NTYwMDB9.";
    let out = decode(token, false).expect("decoded");
    assert_eq!(out.algorithm, "none");
    assert_eq!(out.expires_at, "2030-01-01T00:00:00.000Z");
}

#[test]
fn errors_are_typed() {
    assert_eq!(decode("   ", true).map(|_| ()), Err("empty_input"));
    assert_eq!(
        decode("not-a-token", true).map(|_| ()),
        Err("invalid_token")
    );
    assert_eq!(
        decode("a.b.c.d.e", true).map(|_| ()),
        Err("encrypted_token")
    );
    assert_eq!(
        decode("!!!.e30.sig", true).map(|_| ()),
        Err("invalid_header")
    );
    // Valid base64url but the payload is a JSON array, not an object.
    assert_eq!(
        decode("e30.W10.sig", true).map(|_| ()),
        Err("invalid_payload")
    );
    // A truncated base64url part.
    assert_eq!(
        decode("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJz.sig", true).map(|_| ()),
        Err("invalid_payload")
    );
}
