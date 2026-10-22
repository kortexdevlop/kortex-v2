//! Native host of the engine ABI v1 (ADR 0035): runs an `engine.wasm` with `wasmtime`.
//!
//! One [`EngineModule`] is compiled once (Cranelift, a few hundred milliseconds); every
//! [`EngineModule::run`] gets a fresh instance (clean memory), a memory cap, a fuel budget and the
//! `kx_host` services. Imports the host does not know are satisfied with functions that trap, so
//! an engine that links code it never uses (a third-party crate's browser glue) still loads.
use std::time::Duration;

use kortex_runtime::frame;
use kortex_runtime::{CancelToken, ToolError, ValueMap};
use wasmtime::{
    Caller, Config, Engine as Wasmtime, Extern, Linker, Module, Store, StoreLimits,
    StoreLimitsBuilder,
};

/// Highest ABI version this host speaks.
pub const SUPPORTED_ABI: u32 = 1;
/// Memory a single run may use.
const MEMORY_LIMIT: usize = 1 << 30;
/// Fuel (roughly one unit per executed instruction) granted per millisecond of timeout: a safety
/// net above what compiled code executes in that time, the wall-clock deadline stays the primary
/// limit.
const FUEL_PER_MS: u64 = 2_000_000;
/// Fuel when the contract sets no timeout.
const DEFAULT_FUEL: u64 = 240_000_000_000;

fn failed(detail: impl std::fmt::Display) -> ToolError {
    ToolError::with_detail("worker_failed", detail.to_string())
}

/// A wasmtime error with its whole cause chain (`{:#}`): the root cause names the trap.
fn trap(error: impl Into<wasmtime::Error>) -> ToolError {
    failed(format!("{:#}", error.into()))
}

struct Host {
    cancel: CancelToken,
    limits: StoreLimits,
}

/// A compiled `engine.wasm`.
pub struct EngineModule {
    engine: Wasmtime,
    module: Module,
}

impl EngineModule {
    /// Compiles `wasm`.
    ///
    /// # Errors
    /// The module is invalid, or lacks one of the ABI exports.
    pub fn load(wasm: &[u8]) -> Result<Self, String> {
        let mut config = Config::new();
        config.consume_fuel(true);
        let engine = Wasmtime::new(&config).map_err(|e| e.to_string())?;
        let module = Module::new(&engine, wasm).map_err(|e| e.to_string())?;
        for name in ["memory", "kx_abi", "kx_alloc", "kx_free", "kx_run"] {
            if module.get_export(name).is_none() {
                return Err(format!("engine without the `{name}` export"));
            }
        }
        Ok(Self { engine, module })
    }

    /// Every `(module, name)` import of the engine.
    #[must_use]
    pub fn imports(&self) -> Vec<(String, String)> {
        self.module
            .imports()
            .map(|i| (i.module().to_owned(), i.name().to_owned()))
            .collect()
    }

    /// Imports outside `kx_host` (satisfied with trapping stubs).
    #[must_use]
    pub fn foreign_imports(&self) -> Vec<String> {
        self.imports()
            .into_iter()
            .filter(|(module, _)| module != "kx_host")
            .map(|(module, name)| format!("{module}.{name}"))
            .collect()
    }

    fn linker(&self) -> Result<Linker<Host>, wasmtime::Error> {
        let mut linker = Linker::<Host>::new(&self.engine);
        linker.func_wrap("kx_host", "now_ms", kortex_platform::now_ms)?;
        linker.func_wrap(
            "kx_host",
            "fill_random",
            |mut caller: Caller<'_, Host>, ptr: u32, len: u32| -> u32 {
                let Some(memory) = caller.get_export("memory").and_then(Extern::into_memory) else {
                    return 1;
                };
                let mut bytes = vec![0_u8; len as usize];
                if kortex_platform::fill_random(&mut bytes).is_err() {
                    return 1;
                }
                u32::from(memory.write(&mut caller, ptr as usize, &bytes).is_err())
            },
        )?;
        linker.func_wrap(
            "kx_host",
            "is_cancelled",
            |caller: Caller<'_, Host>| -> u32 { u32::from(caller.data().cancel.is_cancelled()) },
        )?;
        linker.func_wrap("kx_host", "progress", |_milli: u32| {})?;
        // Anything else the module imports fails only if the engine actually calls it.
        linker.define_unknown_imports_as_traps(&self.module)?;
        Ok(linker)
    }

    /// Runs the engine once.
    ///
    /// # Errors
    /// The engine's own error, `cancelled`, or `worker_failed` when the module traps, runs out of
    /// fuel or memory, or breaks the ABI.
    pub fn run(
        &self,
        inputs: &ValueMap,
        params: &ValueMap,
        cancel: &CancelToken,
        timeout: Option<Duration>,
    ) -> Result<ValueMap, ToolError> {
        let request = frame::encode_request(inputs, params);
        let response = self.call(&request, cancel, timeout)?;
        frame::decode_response(&response).map_err(frame::Failure::into_tool_error)
    }

    fn call(
        &self,
        request: &[u8],
        cancel: &CancelToken,
        timeout: Option<Duration>,
    ) -> Result<Vec<u8>, ToolError> {
        let mut store = Store::new(
            &self.engine,
            Host {
                cancel: cancel.clone(),
                limits: StoreLimitsBuilder::new().memory_size(MEMORY_LIMIT).build(),
            },
        );
        store.limiter(|host| &mut host.limits);
        let fuel = timeout.map_or(DEFAULT_FUEL, |t| {
            u64::try_from(t.as_millis())
                .unwrap_or(u64::MAX)
                .saturating_mul(FUEL_PER_MS)
        });
        store.set_fuel(fuel).map_err(trap)?;
        let linker = self.linker().map_err(trap)?;
        let instance = linker.instantiate(&mut store, &self.module).map_err(trap)?;
        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| failed("no memory export"))?;
        let abi = instance
            .get_typed_func::<(), u32>(&mut store, "kx_abi")
            .map_err(trap)?
            .call(&mut store, ())
            .map_err(trap)?;
        if abi == 0 || abi > SUPPORTED_ABI {
            return Err(failed(format!("unsupported engine ABI {abi}")));
        }
        let alloc = instance
            .get_typed_func::<u32, u32>(&mut store, "kx_alloc")
            .map_err(trap)?;
        let free = instance
            .get_typed_func::<(u32, u32), ()>(&mut store, "kx_free")
            .map_err(trap)?;
        let run = instance
            .get_typed_func::<(u32, u32), u64>(&mut store, "kx_run")
            .map_err(trap)?;
        let length = u32::try_from(request.len()).map_err(|_| failed("request too large"))?;
        let pointer = alloc.call(&mut store, length).map_err(trap)?;
        memory
            .write(&mut store, pointer as usize, request)
            .map_err(trap)?;
        let packed = run.call(&mut store, (pointer, length)).map_err(|e| {
            if cancel.is_cancelled() {
                ToolError::new(ToolError::CANCELLED)
            } else {
                trap(e)
            }
        })?;
        let (out_ptr, out_len) = ((packed >> 32) as u32, (packed & 0xffff_ffff) as u32);
        let mut response = vec![0_u8; out_len as usize];
        memory
            .read(&store, out_ptr as usize, &mut response)
            .map_err(trap)?;
        // The response and the request buffers belong to the guest; releasing them is polite but
        // the whole instance is dropped right after, so a failure here changes nothing.
        let _ = free.call(&mut store, (out_ptr, out_len));
        let _ = free.call(&mut store, (pointer, length));
        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kortex_runtime::Value;

    /// A guest that ignores its input and answers a fixed `{"Ok":{"t":{"Text":"hi"}}}` frame.
    fn fixed_guest(extra: &str, run_body: &str) -> Vec<u8> {
        let json = br#"{"Ok":{"t":{"Text":"hi"}},"blobs":[]}"#;
        let mut frame = (json.len() as u32).to_le_bytes().to_vec();
        frame.extend_from_slice(json);
        let escaped: String = frame.iter().map(|b| format!("\\{b:02x}")).collect();
        let len = frame.len();
        wat::parse_str(format!(
            r#"(module
              {extra}
              (memory (export "memory") 2)
              (data (i32.const 1024) "{escaped}")
              (global $bump (mut i32) (i32.const 4096))
              (func (export "kx_abi") (result i32) (i32.const 1))
              (func (export "kx_alloc") (param $n i32) (result i32)
                (local $p i32) (local.set $p (global.get $bump))
                (global.set $bump (i32.add (global.get $bump) (local.get $n)))
                (local.get $p))
              (func (export "kx_free") (param i32 i32))
              (func (export "kx_run") (param i32 i32) (result i64)
                {run_body}
                (i64.or (i64.shl (i64.const 1024) (i64.const 32)) (i64.const {len}))))"#
        ))
        .unwrap_or_default()
    }

    fn run(wasm: &[u8]) -> Result<ValueMap, ToolError> {
        EngineModule::load(wasm).map_err(failed)?.run(
            &ValueMap::new(),
            &ValueMap::new(),
            &CancelToken::default(),
            None,
        )
    }

    #[test]
    fn runs_a_guest_through_the_abi() {
        assert_eq!(
            run(&fixed_guest("", "")),
            Ok(ValueMap::from([(
                "t".to_owned(),
                Value::Text("hi".to_owned())
            )]))
        );
    }

    #[test]
    fn a_module_without_the_exports_is_refused() {
        let bare = wat::parse_str("(module (memory (export \"memory\") 1))").unwrap_or_default();
        assert!(EngineModule::load(&bare).is_err());
    }

    #[test]
    fn unknown_imports_load_and_only_fail_when_called() {
        let import = r#"(import "env" "boom" (func $boom))"#;
        let unused = fixed_guest(import, "");
        let module = EngineModule::load(&unused).map_err(|e| e.to_string());
        assert_eq!(
            module.as_ref().map(EngineModule::foreign_imports),
            Ok(vec!["env.boom".to_owned()])
        );
        assert!(run(&unused).is_ok());
        let called = fixed_guest(import, "(call $boom)");
        let error = run(&called).err().map(|e| (e.code, e.detail));
        assert_eq!(error.as_ref().map(|e| e.0), Some("worker_failed"));
        assert!(error.and_then(|e| e.1).is_some_and(|d| d.contains("boom")));
    }

    #[test]
    fn an_endless_guest_runs_out_of_fuel_instead_of_hanging() {
        let spin = fixed_guest("", "(loop $l (br $l))");
        let started = std::time::Instant::now();
        let outcome = EngineModule::load(&spin).map_err(failed).and_then(|m| {
            m.run(
                &ValueMap::new(),
                &ValueMap::new(),
                &CancelToken::default(),
                Some(Duration::from_millis(2)),
            )
        });
        assert_eq!(outcome.map_err(|e| e.code), Err("worker_failed"));
        assert!(started.elapsed() < Duration::from_secs(60));
    }

    #[test]
    fn a_newer_abi_is_refused() {
        let newer = wat::parse_str(
            r#"(module (memory (export "memory") 1)
                (func (export "kx_abi") (result i32) (i32.const 2))
                (func (export "kx_alloc") (param i32) (result i32) (i32.const 0))
                (func (export "kx_free") (param i32 i32))
                (func (export "kx_run") (param i32 i32) (result i64) (i64.const 0)))"#,
        )
        .unwrap_or_default();
        let error = run(&newer).err();
        assert_eq!(error.as_ref().map(|e| e.code), Some("worker_failed"));
        assert!(
            error
                .and_then(|e| e.detail)
                .is_some_and(|d| d.contains("unsupported engine ABI 2"))
        );
    }
}
