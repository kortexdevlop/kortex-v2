//! Explicit clipboard operations; reads happen only when requested by a caller.
use crate::{PlatformError, Result};
use std::future::Future;

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use objc2_app_kit::NSPasteboard;
    use objc2_foundation::NSString;

    #[derive(Default)]
    pub struct NativeClipboard;
    fn write(board: &NSPasteboard, text: &str) -> Result<()> {
        let value = NSString::from_str(text);
        let kind = NSString::from_str("public.utf8-plain-text");
        board.clearContents();
        if board.setString_forType(&value, &kind) {
            Ok(())
        } else {
            Err(PlatformError("clipboard_failed".into()))
        }
    }
    fn read(board: &NSPasteboard) -> Result<String> {
        let kind = NSString::from_str("public.utf8-plain-text");
        board
            .stringForType(&kind)
            .map(|value| value.to_string())
            .ok_or_else(|| PlatformError("clipboard_unavailable".into()))
    }
    impl crate::Clipboard for NativeClipboard {
        fn write_text(&mut self, text: &str) -> Result<()> {
            write(&NSPasteboard::generalPasteboard(), text)
        }
        fn read_text(&self) -> Result<String> {
            read(&NSPasteboard::generalPasteboard())
        }
    }
    #[test]
    fn private_os_pasteboard_roundtrip_does_not_touch_general_clipboard() {
        let board = NSPasteboard::pasteboardWithUniqueName();
        assert!(read(&board).is_err());
        for text in ["quote\";\nUnicode é 🌙", ""] {
            write(&board, text).expect("write private board");
            assert_eq!(read(&board).expect("read private board"), text);
        }
        board.clearContents();
    }
}
#[cfg(target_os = "macos")]
pub use macos::NativeClipboard;

#[cfg(target_arch = "wasm32")]
mod web {
    use super::*;
    use wasm_bindgen::{JsCast, JsValue};
    use wasm_bindgen_futures::JsFuture;
    pub fn failure(error: JsValue) -> PlatformError {
        let name = js_sys::Reflect::get(&error, &JsValue::from_str("name"))
            .ok()
            .and_then(|v| v.as_string());
        PlatformError(
            match name.as_deref() {
                Some("NotAllowedError" | "SecurityError") => "clipboard_denied",
                _ => "clipboard_failed",
            }
            .into(),
        )
    }
    pub fn start(method: &str, arguments: &js_sys::Array) -> Result<JsFuture> {
        let unavailable = |_| PlatformError("clipboard_unavailable".into());
        let navigator = js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("navigator"))
            .map_err(unavailable)?;
        let clipboard = js_sys::Reflect::get(&navigator, &JsValue::from_str("clipboard"))
            .map_err(unavailable)?;
        let function = js_sys::Reflect::get(&clipboard, &JsValue::from_str(method))
            .map_err(unavailable)?
            .dyn_into::<js_sys::Function>()
            .map_err(unavailable)?;
        let promise = function.apply(&clipboard, arguments).map_err(failure)?;
        Ok(JsFuture::from(js_sys::Promise::resolve(&promise)))
    }
}

/// Start the browser operation immediately, retaining user activation before awaiting it.
#[cfg(target_arch = "wasm32")]
pub fn write_text(text: &str) -> impl Future<Output = Result<()>> + use<> {
    let operation = web::start(
        "writeText",
        &js_sys::Array::of1(&wasm_bindgen::JsValue::from_str(text)),
    );
    async move { operation?.await.map(|_| ()).map_err(web::failure) }
}
#[cfg(target_arch = "wasm32")]
pub async fn read_text() -> Result<String> {
    web::start("readText", &js_sys::Array::new())?
        .await
        .map_err(web::failure)?
        .as_string()
        .ok_or_else(|| PlatformError("clipboard_failed".into()))
}
#[cfg(target_os = "macos")]
pub fn write_text(text: &str) -> impl Future<Output = Result<()>> + use<> {
    std::future::ready(crate::Clipboard::write_text(&mut NativeClipboard, text))
}
#[cfg(target_os = "macos")]
pub async fn read_text() -> Result<String> {
    crate::Clipboard::read_text(&NativeClipboard)
}

// Other native WebViews use the renderer's explicit capability/result adapter.
#[cfg(all(
    not(target_arch = "wasm32"),
    not(target_os = "macos"),
    feature = "document"
))]
pub fn write_text(text: &str) -> impl Future<Output = Result<()>> + use<> {
    let operation = crate::document::clipboard_operation(Some(text));
    async move { operation.await.map(|_| ()) }
}
#[cfg(all(
    not(target_arch = "wasm32"),
    not(target_os = "macos"),
    feature = "document"
))]
pub async fn read_text() -> Result<String> {
    crate::document::clipboard_operation(None).await
}
#[cfg(all(
    not(target_arch = "wasm32"),
    not(target_os = "macos"),
    not(feature = "document")
))]
pub fn write_text(_text: &str) -> impl Future<Output = Result<()>> + use<> {
    std::future::ready(Err(PlatformError("clipboard_unavailable".into())))
}
#[cfg(all(
    not(target_arch = "wasm32"),
    not(target_os = "macos"),
    not(feature = "document")
))]
pub async fn read_text() -> Result<String> {
    Err(PlatformError("clipboard_unavailable".into()))
}
