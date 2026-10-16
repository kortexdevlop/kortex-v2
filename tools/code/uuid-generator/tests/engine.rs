use kortex::runtime::{Cx, block_on};
use tool_code_uuid_generator::{Inputs, Params, engine::run};

fn make(tweak: impl FnOnce(&mut Params), cx: &mut Cx) -> Vec<String> {
    let mut p = Params::default();
    tweak(&mut p);
    block_on(run(Inputs {}, p, cx))
        .expect("uuids")
        .uuids
        .lines()
        .map(str::to_owned)
        .collect()
}

fn is_uuid(s: &str) -> bool {
    let b: Vec<char> = s.chars().collect();
    b.len() == 36
        && [8, 13, 18, 23].iter().all(|i| b[*i] == '-')
        && b.iter()
            .enumerate()
            .all(|(i, c)| [8, 13, 18, 23].contains(&i) || c.is_ascii_hexdigit())
}

#[test]
fn v4_has_the_right_shape_version_and_variant() {
    let u = make(|_| {}, &mut Cx::new());
    assert_eq!(u.len(), 1);
    assert!(is_uuid(&u[0]), "{}", u[0]);
    let c: Vec<char> = u[0].chars().collect();
    assert_eq!(c[14], '4', "version nibble");
    assert!(
        matches!(c[19], '8' | '9' | 'a' | 'b'),
        "variant bits: {}",
        c[19]
    );
}

#[test]
fn v7_starts_with_the_injected_timestamp_and_is_sortable() {
    let ms: u64 = 1_700_000_000_123;
    let u = make(
        |p| {
            p.version = "v7".into();
            p.count = 3
        },
        &mut Cx::new().with_clock(move || ms),
    );
    assert_eq!(u.len(), 3);
    for id in &u {
        assert!(is_uuid(id), "{id}");
        let hex: String = id.chars().filter(|c| *c != '-').collect();
        assert_eq!(
            u64::from_str_radix(&hex[..12], 16),
            Ok(ms),
            "48-bit timestamp prefix"
        );
        assert_eq!(&hex[12..13], "7", "version nibble");
    }
    let later = make(
        |p| p.version = "v7".into(),
        &mut Cx::new().with_clock(move || ms + 5_000),
    );
    assert!(later[0] > u[0], "a later v7 sorts after an earlier one");
}

#[test]
fn count_formatting_and_uniqueness() {
    let u = make(
        |p| {
            p.count = 100;
            p.uppercase = true;
            p.hyphens = false
        },
        &mut Cx::new(),
    );
    assert_eq!(u.len(), 100);
    assert!(u.iter().all(|x| {
        x.len() == 32
            && x.chars()
                .all(|c| c.is_ascii_digit() || c.is_ascii_uppercase())
    }));
    let set: std::collections::HashSet<_> = u.iter().collect();
    assert_eq!(set.len(), 100, "no duplicates");
}

#[test]
fn deterministic_with_injected_entropy_and_validated_params() {
    let a = make(|_| {}, &mut Cx::new().with_entropy(|b| b.fill(0xAB)));
    let b = make(|_| {}, &mut Cx::new().with_entropy(|b| b.fill(0xAB)));
    assert_eq!(a, b);
    assert_eq!(a[0], "abababab-abab-4bab-abab-abababababab");
    let bad = block_on(run(
        Inputs {},
        Params {
            count: 0,
            ..Params::default()
        },
        &mut Cx::new(),
    ))
    .map_err(|e| e.code);
    assert_eq!(bad.map(|_| ()), Err("invalid_param"));
    let v = block_on(run(
        Inputs {},
        Params {
            version: "v9".into(),
            ..Params::default()
        },
        &mut Cx::new(),
    ))
    .map_err(|e| e.code);
    assert_eq!(v.map(|_| ()), Err("invalid_param"));
}
