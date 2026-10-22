//! IndexedDB transactions serialize cross-tab snapshot revisions on this origin.
use crate::projects::{
    MAX_SNAPSHOT_BYTES, SnapshotStore, StoreError, StoredSnapshot, validate_id, validate_update,
};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/src/project-store.js")]
extern "C" {
    #[wasm_bindgen(catch, js_name = loadSnapshot)]
    async fn load_snapshot(namespace: &str, id: &str) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch, js_name = casSnapshot)]
    async fn cas_snapshot(
        namespace: &str,
        id: &str,
        expected: JsValue,
        revision: &str,
        bytes: &js_sys::Uint8Array,
    ) -> Result<JsValue, JsValue>;
}

pub struct WebStore {
    namespace: String,
}
impl WebStore {
    pub fn open(namespace: &str) -> Result<Self, StoreError> {
        validate_id(namespace)?;
        Ok(Self {
            namespace: namespace.into(),
        })
    }
}
fn error(value: JsValue) -> StoreError {
    match value.as_string().as_deref() {
        Some("Conflict") => StoreError::Conflict,
        Some("InvalidId") => StoreError::InvalidId,
        Some("TooLarge") => StoreError::TooLarge,
        Some("Corrupt") => StoreError::Corrupt,
        _ => StoreError::Unavailable,
    }
}
impl SnapshotStore for WebStore {
    async fn load(&self, id: &str) -> Result<Option<StoredSnapshot>, StoreError> {
        validate_id(id)?;
        let value = load_snapshot(&self.namespace, id).await.map_err(error)?;
        if value.is_null() {
            return Ok(None);
        }
        let revision = js_sys::Reflect::get(&value, &"revision".into())
            .map_err(|_| StoreError::Corrupt)?
            .as_string()
            .ok_or(StoreError::Corrupt)?
            .parse()
            .map_err(|_| StoreError::Corrupt)?;
        let bytes = js_sys::Reflect::get(&value, &"bytes".into())
            .map_err(|_| StoreError::Corrupt)?
            .dyn_into::<js_sys::Uint8Array>()
            .map_err(|_| StoreError::Corrupt)?;
        if bytes.length() as usize > MAX_SNAPSHOT_BYTES {
            return Err(StoreError::TooLarge);
        }
        Ok(Some(StoredSnapshot {
            revision,
            bytes: bytes.to_vec(),
        }))
    }
    async fn compare_and_swap(
        &self,
        id: &str,
        expected: Option<u64>,
        snapshot: &StoredSnapshot,
    ) -> Result<(), StoreError> {
        validate_id(id)?;
        validate_update(expected, snapshot)?;
        let expected = expected.map_or(JsValue::NULL, |v| JsValue::from_str(&v.to_string()));
        let bytes = js_sys::Uint8Array::from(snapshot.bytes.as_slice());
        cas_snapshot(
            &self.namespace,
            id,
            expected,
            &snapshot.revision.to_string(),
            &bytes,
        )
        .await
        .map_err(error)?;
        Ok(())
    }
}
