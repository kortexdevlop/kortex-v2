use kortex_platform::{Storage, storage::WebStorage};
use wasm_bindgen::prelude::*;
fn error(error: kortex_platform::PlatformError) -> JsValue {
    JsValue::from_str(&error.0)
}
#[wasm_bindgen]
pub fn get(namespace: &str, key: &str) -> Result<Option<String>, JsValue> {
    WebStorage::open(namespace)
        .and_then(|s| s.get(key))
        .map_err(error)
}
#[wasm_bindgen]
pub fn set(namespace: &str, key: &str, value: &str) -> Result<(), JsValue> {
    WebStorage::open(namespace)
        .and_then(|mut s| s.set(key, value))
        .map_err(error)
}
#[wasm_bindgen]
pub fn remove(namespace: &str, key: &str) -> Result<(), JsValue> {
    WebStorage::open(namespace)
        .and_then(|mut s| s.remove(key))
        .map_err(error)
}
#[wasm_bindgen]
pub async fn clipboard_write(text: String) -> Result<(), JsValue> {
    kortex_platform::clipboard::write_text(&text)
        .await
        .map_err(error)
}
#[wasm_bindgen]
pub async fn clipboard_read() -> Result<String, JsValue> {
    kortex_platform::clipboard::read_text().await.map_err(error)
}
