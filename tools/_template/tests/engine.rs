use kortex::runtime::{Cx, block_on};
use {{crate}}::{Inputs, Params, engine::run};

#[test]
fn nominal() {
    let out = block_on(run(Inputs { text: "hello".into() }, Params::default(), &mut Cx::new()));
    assert_eq!(out.map(|o| o.text), Ok("hello".to_owned()));
}

#[test]
fn error_on_empty_input() {
    let out = block_on(run(Inputs { text: String::new() }, Params::default(), &mut Cx::new()));
    assert_eq!(out.map_err(|e| e.code), Err("empty_input"));
}
