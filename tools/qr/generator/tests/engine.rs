use kortex::runtime::{Cx, FileData, block_on};
use tool_qr_generator::{Inputs, Params, engine::run};

fn go(content: &str, params: Params) -> Result<FileData, &'static str> {
    block_on(run(
        Inputs {
            content: content.to_owned(),
        },
        params,
        &mut Cx::new(),
    ))
    .map(|o| o.image)
    .map_err(|e| e.code)
}

#[test]
fn png_decodes_with_expected_palette_and_size() {
    let f = go("https://kortex.site", Params::default()).expect("png");
    assert_eq!(
        (f.name.as_str(), f.mime.as_str()),
        ("qrcode.png", "image/png")
    );
    let mut reader = png::Decoder::new(std::io::Cursor::new(&f.bytes))
        .read_info()
        .expect("valid png");
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).expect("decode");
    assert_eq!(info.width, info.height);
    assert!(
        info.width <= 1024 && info.width >= 500,
        "width {}",
        info.width
    );
    // First pixel is in the quiet zone: background (palette index 0 => white).
    assert_eq!(buf[0] & 0x80, 0);
    // The image contains dark modules.
    assert!(buf.iter().any(|b| *b != 0));
}

#[test]
fn svg_is_well_formed_and_colored() {
    let params = Params {
        format: "svg".into(),
        foreground: "#FF0000".into(),
        background: "#00ff00".into(),
        ..Params::default()
    };
    let f = go("hello", params).expect("svg");
    let text = String::from_utf8(f.bytes).expect("utf8");
    assert!(text.starts_with("<svg") && text.ends_with("</svg>"));
    assert!(text.contains("#ff0000") && text.contains("#00ff00"));
    assert!(text.contains("M"));
}

#[test]
fn error_correction_changes_density_and_content_is_deterministic() {
    let svg = |ec: &str| {
        go(
            "same content",
            Params {
                format: "svg".into(),
                error_correction: ec.into(),
                ..Params::default()
            },
        )
        .map(|f| f.bytes)
    };
    assert_eq!(svg("L"), svg("L"));
    assert_ne!(svg("L"), svg("H"));
}

#[test]
fn errors_on_empty_content_bad_color_and_too_long() {
    assert_eq!(go("", Params::default()).map(|_| ()), Err("empty_content"));
    assert_eq!(
        go(
            "x",
            Params {
                foreground: "red".into(),
                ..Params::default()
            }
        )
        .map(|_| ()),
        Err("invalid_color")
    );
    assert_eq!(
        go(&"a".repeat(5000), Params::default()).map(|_| ()),
        Err("content_too_long")
    );
    assert_eq!(
        go(
            "x",
            Params {
                size: 10,
                ..Params::default()
            }
        )
        .map(|_| ()),
        Err("invalid_param")
    );
}
