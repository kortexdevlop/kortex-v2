use kortex::runtime::{Cx, ToolError, block_on};

mod demo {
    kortex::contract!("tests/fixtures/demo.toml");

    pub mod engine {
        use super::{Inputs, Outputs, Params};
        use kortex::runtime::{Cx, ToolError};

        #[kortex::engine]
        pub async fn run(input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
            params.validate()?;
            cx.check_cancelled()?;
            Ok(Outputs {
                result: input
                    .content
                    .repeat(usize::try_from(params.size).unwrap_or(1)),
            })
        }
    }
    pub use engine::run;
}

#[test]
fn generated_types_and_defaults() {
    let p = demo::Params::default();
    assert_eq!(p.size, 5);
    assert_eq!(p.label, "hi");
    assert_eq!(demo::TOOL_ID, "text.demo");
    assert_eq!(demo::PARAM_LEVELS, &[("size", "simple"), ("label", "pro")]);
}

#[test]
fn engine_runs() {
    let out = block_on(demo::run(
        demo::Inputs {
            content: "a".into(),
        },
        demo::Params::default(),
        &mut Cx::new(),
    ));
    assert_eq!(out.map(|o| o.result), Ok("aaaaa".to_owned()));
}

#[test]
fn range_is_enforced() {
    let params = demo::Params {
        size: 99,
        ..demo::Params::default()
    };
    let out = block_on(demo::run(
        demo::Inputs {
            content: "a".into(),
        },
        params,
        &mut Cx::new(),
    ));
    assert_eq!(
        out.map(|o| o.result).map_err(|e| e.code),
        Err(ToolError::INVALID_PARAM)
    );
}

#[test]
fn dynamic_entry_point() {
    use kortex::runtime::{Value, ValueMap};
    let inputs = ValueMap::from([("content".to_owned(), Value::Text("ab".into()))]);
    let params = ValueMap::from([("size".to_owned(), Value::Int(2))]);
    let out = block_on(demo::run_map(&inputs, &params, &mut Cx::new()));
    assert_eq!(
        out.map(|m| m.get("result").cloned()),
        Ok(Some(Value::Text("abab".into())))
    );
    let missing = block_on(demo::run_map(&ValueMap::new(), &params, &mut Cx::new()));
    assert_eq!(
        missing.map(|_| ()).map_err(|e| e.code),
        Err("missing_input")
    );
    let bad = ValueMap::from([("size".to_owned(), Value::Text("x".into()))]);
    assert!(block_on(demo::run_map(&inputs, &bad, &mut Cx::new())).is_err());
}
