use kortex::runtime::{Cx, block_on};
use tool_secure_password_generator::{Inputs, Params, engine::run};

fn generate(params: Params) -> Result<String, &'static str> {
    block_on(run(Inputs {}, params, &mut Cx::new()))
        .map(|o| o.password)
        .map_err(|e| e.code)
}

#[test]
fn nominal_default_length_and_classes() {
    let pw = generate(Params::default()).expect("password");
    assert_eq!(pw.chars().count(), 16);
    assert!(pw.chars().any(|c| c.is_ascii_lowercase()));
    assert!(pw.chars().any(|c| c.is_ascii_uppercase()));
    assert!(pw.chars().any(|c| c.is_ascii_digit()));
    assert!(pw.chars().any(|c| !c.is_ascii_alphanumeric()));
}

#[test]
fn passwords_differ_between_runs() {
    assert_ne!(generate(Params::default()), generate(Params::default()));
}

#[test]
fn digits_only_pin() {
    let params = Params {
        length: 8,
        lowercase: false,
        uppercase: false,
        symbols: false,
        ..Params::default()
    };
    let pin = generate(params).expect("pin");
    assert!(pin.chars().all(|c| c.is_ascii_digit()));
}

#[test]
fn ambiguous_characters_are_excluded() {
    let params = Params {
        length: 128,
        exclude_ambiguous: true,
        ..Params::default()
    };
    let pw = generate(params).expect("password");
    assert!(!pw.chars().any(|c| "0O1lI|".contains(c)));
}

#[test]
fn error_without_any_class() {
    let params = Params {
        lowercase: false,
        uppercase: false,
        digits: false,
        symbols: false,
        ..Params::default()
    };
    assert_eq!(generate(params), Err("no_character_class"));
}

#[test]
fn error_when_length_out_of_range() {
    let params = Params {
        length: 4,
        ..Params::default()
    };
    assert_eq!(generate(params), Err("invalid_param"));
}

#[test]
fn deterministic_with_injected_entropy() {
    let mut a = Cx::new().with_entropy(|b| b.fill(3));
    let mut b = Cx::new().with_entropy(|b| b.fill(3));
    let x = block_on(run(Inputs {}, Params::default(), &mut a)).map(|o| o.password);
    let y = block_on(run(Inputs {}, Params::default(), &mut b)).map(|o| o.password);
    assert_eq!(x, y);
}
