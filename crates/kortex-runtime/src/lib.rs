//! Engine runtime: execution context, shared data types, minimal executor.
pub mod engine_abi;
pub mod frame;
#[cfg(not(target_arch = "wasm32"))]
pub mod native;

use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll, Waker};

/// Dynamically typed value crossing the tool boundary (UI, bus, Flow, Mind, CLI).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Value {
    Text(String),
    Number(f64),
    Int(i64),
    Bool(bool),
    File(FileData),
    Files(Vec<FileData>),
    Table(Table),
}

/// Named values: inputs, params or outputs of a tool run.
pub type ValueMap = std::collections::BTreeMap<String, Value>;

/// Conversion from a dynamic [`Value`] (used by the generated `from_map`).
pub trait FromValue: Sized {
    /// # Errors
    /// `invalid_param` when the value has the wrong type.
    fn from_value(v: &Value) -> Result<Self, ToolError>;
}

/// Conversion into a dynamic [`Value`] (used by the generated `into_map`).
pub trait IntoValue {
    fn into_value(self) -> Value;
}

macro_rules! value_impls {
    ($($ty:ty => $variant:ident),* $(,)?) => {$(
        impl FromValue for $ty {
            fn from_value(v: &Value) -> Result<Self, ToolError> {
                match v {
                    Value::$variant(x) => Ok(x.clone()),
                    _ => Err(ToolError::new(ToolError::INVALID_PARAM)),
                }
            }
        }
        impl IntoValue for $ty {
            fn into_value(self) -> Value { Value::$variant(self) }
        }
    )*};
}

value_impls!(String => Text, i64 => Int, bool => Bool, FileData => File, Vec<FileData> => Files, Table => Table);

impl FromValue for f64 {
    #[allow(clippy::cast_precision_loss)]
    fn from_value(v: &Value) -> Result<Self, ToolError> {
        match v {
            Value::Number(x) => Ok(*x),
            Value::Int(i) => Ok(*i as f64),
            _ => Err(ToolError::new(ToolError::INVALID_PARAM)),
        }
    }
}

impl IntoValue for f64 {
    fn into_value(self) -> Value {
        Value::Number(self)
    }
}

/// Typed engine error. `code` is translated by the UI via `locales/` (never shown raw).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolError {
    pub code: &'static str,
    pub detail: Option<String>,
}

impl ToolError {
    #[must_use]
    pub const fn new(code: &'static str) -> Self {
        Self { code, detail: None }
    }

    #[must_use]
    pub fn with_detail(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: Some(detail.into()),
        }
    }

    pub const CANCELLED: &'static str = "cancelled";
    pub const INVALID_PARAM: &'static str = "invalid_param";
}

/// A file passed between tools.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FileData {
    pub name: String,
    pub mime: String,
    #[serde(with = "wire_bytes")]
    pub bytes: Vec<u8>,
}

/// Tabular data (`table` port type).
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Table {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

pub use kortex_platform::WorkerFailure;

impl From<ToolError> for WorkerFailure {
    fn from(error: ToolError) -> Self {
        Self {
            code: error.code.to_owned(),
            detail: error.detail,
        }
    }
}

// Binary serialization produces Uint8Array in the browser instead of JSON arrays of numbers.
mod wire_bytes {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(bytes)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
        struct Bytes;
        impl<'de> serde::de::Visitor<'de> for Bytes {
            type Value = Vec<u8>;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("binary bytes")
            }
            fn visit_byte_buf<E: serde::de::Error>(self, bytes: Vec<u8>) -> Result<Vec<u8>, E> {
                Ok(bytes)
            }
            fn visit_bytes<E: serde::de::Error>(self, bytes: &[u8]) -> Result<Vec<u8>, E> {
                Ok(bytes.to_vec())
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(self, seq: A) -> Result<Vec<u8>, A::Error> {
                Deserialize::deserialize(serde::de::value::SeqAccessDeserializer::new(seq))
            }
        }
        deserializer.deserialize_byte_buf(Bytes)
    }
}

/// Services of the engine host (module `kx_host` of the engine ABI, ADR 0035).
#[cfg(all(target_arch = "wasm32", feature = "engine-abi"))]
mod host {
    #[link(wasm_import_module = "kx_host")]
    unsafe extern "C" {
        pub fn now_ms() -> u64;
        pub fn fill_random(ptr: *mut u8, len: usize) -> u32;
        pub fn is_cancelled() -> u32;
        pub fn progress(milli: u32);
    }
}

fn platform_now_ms() -> u64 {
    #[cfg(all(target_arch = "wasm32", feature = "engine-abi"))]
    {
        // SAFETY: the engine host provides `kx_host.now_ms`; it takes and returns plain integers.
        unsafe { host::now_ms() }
    }
    #[cfg(not(all(target_arch = "wasm32", feature = "engine-abi")))]
    {
        kortex_platform::now_ms()
    }
}

fn platform_fill_random(buf: &mut [u8]) -> bool {
    #[cfg(all(target_arch = "wasm32", feature = "engine-abi"))]
    {
        // SAFETY: the host writes at most `buf.len()` bytes at `buf`, inside this module's memory.
        unsafe { host::fill_random(buf.as_mut_ptr(), buf.len()) == 0 }
    }
    #[cfg(not(all(target_arch = "wasm32", feature = "engine-abi")))]
    {
        kortex_platform::fill_random(buf).is_ok()
    }
}

fn platform_yield() {
    #[cfg(not(all(target_arch = "wasm32", feature = "engine-abi")))]
    kortex_platform::yield_now();
}

fn host_cancelled() -> bool {
    #[cfg(all(target_arch = "wasm32", feature = "engine-abi"))]
    {
        // SAFETY: plain integer result.
        unsafe { host::is_cancelled() != 0 }
    }
    #[cfg(not(all(target_arch = "wasm32", feature = "engine-abi")))]
    {
        false
    }
}

/// Cloneable handle to cancel a running engine from another place (UI, Flow, CLI).
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

type EntropySource = Box<dyn FnMut(&mut [u8])>;
type Clock = Box<dyn Fn() -> u64>;

/// Execution context handed to every engine: progress, cancellation, journal.
#[derive(Default)]
pub struct Cx {
    progress: f32,
    cancel: CancelToken,
    journal: Vec<String>,
    entropy: Option<EntropySource>,
    clock: Option<Clock>,
}

impl Cx {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_cancel(cancel: CancelToken) -> Self {
        Self {
            cancel,
            ..Self::default()
        }
    }

    /// Report progress in `0.0..=1.0`.
    pub fn progress(&mut self, value: f32) {
        self.progress = value.clamp(0.0, 1.0);
        #[cfg(all(target_arch = "wasm32", feature = "engine-abi"))]
        // SAFETY: plain integer argument; the host may ignore it.
        unsafe {
            host::progress((self.progress * 1000.0) as u32);
        }
    }

    #[must_use]
    pub const fn current_progress(&self) -> f32 {
        self.progress
    }

    /// # Errors
    /// Returns `ToolError::CANCELLED` once the user cancelled.
    pub fn check_cancelled(&self) -> Result<(), ToolError> {
        if self.cancel.is_cancelled() || host_cancelled() {
            Err(ToolError::new(ToolError::CANCELLED))
        } else {
            Ok(())
        }
    }

    /// Inject a deterministic entropy source (tests, reproducible runs).
    #[must_use]
    pub fn with_entropy(mut self, source: impl FnMut(&mut [u8]) + 'static) -> Self {
        self.entropy = Some(Box::new(source));
        self
    }

    /// Inject a fixed clock (tests, reproducible runs): milliseconds since the Unix epoch.
    #[must_use]
    pub fn with_clock(mut self, now_ms: impl Fn() -> u64 + 'static) -> Self {
        self.clock = Some(Box::new(now_ms));
        self
    }

    /// Current time in milliseconds since the Unix epoch (UTC). Engines get time only through
    /// here: `SystemTime::now()` panics on `wasm32-unknown-unknown`.
    #[must_use]
    pub fn now_ms(&self) -> u64 {
        if let Some(c) = &self.clock {
            return c();
        }
        platform_now_ms()
    }

    /// Fill `buf` with random bytes (OS CSPRNG unless an entropy source was injected).
    ///
    /// # Errors
    /// `entropy_unavailable` when the platform cannot provide randomness.
    pub fn fill_random(&mut self, buf: &mut [u8]) -> Result<(), ToolError> {
        if let Some(src) = &mut self.entropy {
            src(buf);
            return Ok(());
        }
        if platform_fill_random(buf) {
            Ok(())
        } else {
            Err(ToolError::new("entropy_unavailable"))
        }
    }

    pub fn log(&mut self, line: impl Into<String>) {
        self.journal.push(line.into());
    }

    #[must_use]
    pub fn journal(&self) -> &[String] {
        &self.journal
    }
}

/// Run a future to completion on the current thread (engines are pure and never block on I/O
/// themselves; platform backends provide real executors).
pub fn block_on<F: Future>(fut: F) -> F::Output {
    let mut fut = pin!(fut);
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
            return v;
        }
        platform_yield();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_is_clamped() {
        let mut cx = Cx::new();
        cx.progress(3.0);
        assert!((cx.current_progress() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn cancellation_is_observed() {
        let token = CancelToken::default();
        let cx = Cx::with_cancel(token.clone());
        assert!(cx.check_cancelled().is_ok());
        token.cancel();
        assert_eq!(
            cx.check_cancelled(),
            Err(ToolError::new(ToolError::CANCELLED))
        );
    }

    #[test]
    fn entropy_is_injectable_and_os_works() {
        let mut cx = Cx::new().with_entropy(|b| b.fill(7));
        let mut buf = [0_u8; 4];
        assert!(cx.fill_random(&mut buf).is_ok());
        assert_eq!(buf, [7; 4]);
        let mut os = [0_u8; 32];
        assert!(Cx::new().fill_random(&mut os).is_ok());
        assert_ne!(os, [0_u8; 32]);
    }

    #[test]
    fn value_conversions() {
        assert_eq!(i64::from_value(&Value::Int(3)), Ok(3));
        assert_eq!(f64::from_value(&Value::Int(3)), Ok(3.0));
        assert!(bool::from_value(&Value::Text("x".into())).is_err());
        assert_eq!("a".to_owned().into_value(), Value::Text("a".into()));
    }

    #[test]
    fn clock_is_injectable_and_real_clock_is_sane() {
        assert_eq!(
            Cx::new().with_clock(|| 1_700_000_000_123).now_ms(),
            1_700_000_000_123
        );
        assert!(
            Cx::new().now_ms() > 1_700_000_000_000,
            "the real clock reports a recent date"
        );
    }

    #[test]
    fn block_on_runs_async() {
        assert_eq!(block_on(async { 40 + 2 }), 42);
    }
}
