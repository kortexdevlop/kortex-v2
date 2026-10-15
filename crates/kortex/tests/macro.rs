use kortex::runtime::{Cx, ToolError, block_on};

mod demo {
    use kortex::runtime::{Cx, ToolError};

    kortex::contract!("tests/fixtures/demo.toml");

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
