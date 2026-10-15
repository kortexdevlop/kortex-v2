//! Engine runtime: execution context, shared data types, minimal executor.
use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll, Waker};

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
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileData {
    pub name: String,
    pub mime: String,
    pub bytes: Vec<u8>,
}

/// Tabular data (`table` port type).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Table {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
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

/// Execution context handed to every engine: progress, cancellation, journal.
#[derive(Default)]
pub struct Cx {
    progress: f32,
    cancel: CancelToken,
    journal: Vec<String>,
    entropy: Option<Box<dyn FnMut(&mut [u8])>>,
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
    }

    #[must_use]
    pub const fn current_progress(&self) -> f32 {
        self.progress
    }

    /// # Errors
    /// Returns `ToolError::CANCELLED` once the user cancelled.
    pub fn check_cancelled(&self) -> Result<(), ToolError> {
        if self.cancel.is_cancelled() {
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

    /// Fill `buf` with random bytes (OS CSPRNG unless an entropy source was injected).
    ///
    /// # Errors
    /// `entropy_unavailable` when the platform cannot provide randomness.
    pub fn fill_random(&mut self, buf: &mut [u8]) -> Result<(), ToolError> {
        if let Some(src) = &mut self.entropy {
            src(buf);
            return Ok(());
        }
        getrandom::fill(buf).map_err(|_| ToolError::new("entropy_unavailable"))
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
        std::thread::yield_now();
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
    fn block_on_runs_async() {
        assert_eq!(block_on(async { 40 + 2 }), 42);
    }
}
