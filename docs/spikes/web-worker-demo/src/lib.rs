//! Disposable proof that an existing Rust tool engine can run inside a browser Web Worker.
use kortex_runtime::{Cx, FileData, block_on};
use tool_image_convert::{Inputs, Params, engine::run};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn convert_png_to_jpg(bytes: &[u8]) -> Result<Vec<u8>, JsValue> {
    let input = Inputs {
        images: vec![FileData {
            name: "sample.png".to_owned(),
            mime: "image/png".to_owned(),
            bytes: bytes.to_vec(),
        }],
    };
    let params = Params {
        format: "jpg".to_owned(),
        ..Params::default()
    };
    let output = block_on(run(input, params, &mut Cx::new()))
        .map_err(|e| JsValue::from_str(e.code))?;
    output
        .converted
        .into_iter()
        .next()
        .map(|file| file.bytes)
        .ok_or_else(|| JsValue::from_str("missing_output"))
}

#[wasm_bindgen]
pub fn burn_cpu(iterations: u32) -> u32 {
    let mut value = 1_u32;
    for i in 0..iterations {
        value = std::hint::black_box(value.wrapping_mul(1_664_525).wrapping_add(i));
    }
    value
}
