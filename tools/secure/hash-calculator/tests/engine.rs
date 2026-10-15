use kortex::runtime::{Cx, FileData, block_on};
use tool_secure_hash_calculator::{Inputs, Outputs, Params, engine::run};

fn text(t: &str) -> Inputs {
    Inputs {
        file: None,
        text: Some(t.to_owned()),
    }
}

fn go(input: Inputs, params: Params) -> Result<Outputs, &'static str> {
    block_on(run(input, params, &mut Cx::new())).map_err(|e| e.code)
}

fn algo(a: &str) -> Params {
    Params {
        algorithm: a.to_owned(),
        ..Params::default()
    }
}

#[test]
fn nominal_known_vectors_for_abc() {
    let h = |a| go(text("abc"), algo(a)).map(|o| o.hash);
    assert_eq!(h("md5"), Ok("900150983cd24fb0d6963f7d28e17f72".to_owned()));
    assert_eq!(
        h("sha1"),
        Ok("a9993e364706816aba3e25717850c26c9cd0d89d".to_owned())
    );
    assert_eq!(
        h("sha256"),
        Ok("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".to_owned())
    );
    assert_eq!(
        h("sha512"),
        Ok("ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f".to_owned())
    );
}

#[test]
fn file_takes_precedence_over_text() {
    let file = FileData {
        name: "a.txt".into(),
        mime: "text/plain".into(),
        bytes: b"abc".to_vec(),
    };
    let out = go(
        Inputs {
            file: Some(file),
            text: Some("zzz".into()),
        },
        Params::default(),
    )
    .expect("hash");
    assert_eq!(
        out.hash,
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn empty_input_hash_and_large_file() {
    let empty = FileData {
        name: "e".into(),
        mime: String::new(),
        bytes: Vec::new(),
    };
    let out = go(
        Inputs {
            file: Some(empty),
            text: None,
        },
        algo("sha256"),
    )
    .expect("hash");
    assert_eq!(
        out.hash,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    let big = FileData {
        name: "big".into(),
        mime: String::new(),
        bytes: vec![0_u8; 3 * (1 << 20) + 17],
    };
    assert_eq!(
        go(
            Inputs {
                file: Some(big),
                text: None
            },
            algo("sha256")
        )
        .map(|o| o.hash.len()),
        Ok(64)
    );
}

#[test]
fn compare_and_uppercase() {
    let params = Params {
        uppercase: true,
        compare_with: " BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD ".into(),
        ..Params::default()
    };
    let out = go(text("abc"), params).expect("hash");
    assert_eq!(out.verdict, "match");
    assert!(out.hash.chars().all(|c| !c.is_ascii_lowercase()));
    let bad = Params {
        compare_with: "deadbeef".into(),
        ..Params::default()
    };
    assert_eq!(
        go(text("abc"), bad).map(|o| o.verdict),
        Ok("mismatch".to_owned())
    );
}

#[test]
fn error_without_input_or_with_unknown_algorithm() {
    assert_eq!(
        go(
            Inputs {
                file: None,
                text: None
            },
            Params::default()
        )
        .map(|_| ()),
        Err("missing_input")
    );
    assert_eq!(
        go(text(""), Params::default()).map(|_| ()),
        Err("missing_input")
    );
    assert_eq!(
        go(text("abc"), algo("rot13")).map(|_| ()),
        Err("invalid_param")
    );
}
