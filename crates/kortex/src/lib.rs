//! Facade used by tools: `kortex::contract!`, `#[kortex::engine]`, `kortex::runtime::*`.
pub use kortex_macros::{contract, engine};
pub use kortex_runtime as runtime;

/// File typing by signature, with the runtime error mapping used by generated entry points.
pub mod io {
    pub use kortex_io::*;
    use kortex_runtime::{FileData, ToolError};

    /// Rejects a file whose bytes do not match the port pattern (`file_type_mismatch`).
    ///
    /// # Errors
    /// `file_type_mismatch` with the file name as detail.
    pub fn check_file(file: &FileData, pattern: &str) -> Result<(), ToolError> {
        check(&file.bytes, pattern)
            .map_err(|_| ToolError::with_detail("file_type_mismatch", file.name.clone()))
    }
}
