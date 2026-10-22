//! Host-owned local media capabilities. Persist descriptive ResourceIds in projects, never Handles.
//! Native registration opens the host-selected path; it is not a filesystem sandbox or picker.
//! Browser Blobs and all capabilities must be reselected after reload. No file paths are exposed.
use std::collections::BTreeMap;
use std::sync::Arc;

pub const MAX_READ_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_HANDLES: usize = 4096;

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub max_handles: usize,
    pub max_read_bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_handles: 64,
            max_read_bytes: MAX_READ_BYTES,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    InvalidLimits,
    InvalidProject,
    InvalidMime,
    HandleQuota,
    InvalidHandle,
    WrongProject,
    ReadLimit,
    Bounds,
    Changed,
    Io,
    InvalidBlob,
    Allocation,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "local media: {self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

/// A capability created only by its broker. Cloning explicitly delegates the same access.
/// No serialization, public constructor, ResourceId lookup or path is available.
#[derive(Clone)]
pub struct Handle {
    owner: Arc<()>,
    slot: u64,
}
impl std::fmt::Debug for Handle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Handle(<opaque>)")
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Metadata {
    pub size_bytes: u64,
    pub mime: String,
}

struct Entry {
    project: String,
    metadata: Metadata,
    source: Source,
}
#[cfg(not(target_arch = "wasm32"))]
struct Source {
    file: std::fs::File,
    modified: Option<std::time::SystemTime>,
}
#[cfg(target_arch = "wasm32")]
struct Source {
    blob: wasm_bindgen::JsValue,
}

/// Runtime authority. Only the trusted host registers files and retains ResourceId -> Handle maps.
/// Serialized model arguments cannot mint Handles. Drop/revoke closes its retained sources.
/// Native reads are synchronous: heavy callers should move their broker to a worker thread.
pub struct Broker {
    owner: Arc<()>,
    limits: Limits,
    next: u64,
    entries: BTreeMap<u64, Entry>,
}
impl Broker {
    /// # Errors
    /// Zero limits or limits above the hard ceilings are rejected.
    pub fn new(limits: Limits) -> Result<Self> {
        if limits.max_handles == 0
            || limits.max_handles > MAX_HANDLES
            || limits.max_read_bytes == 0
            || limits.max_read_bytes > MAX_READ_BYTES
        {
            return Err(Error::InvalidLimits);
        }
        Ok(Self {
            owner: Arc::new(()),
            limits,
            next: 0,
            entries: BTreeMap::new(),
        })
    }
    fn registration(&self, project: &str) -> Result<()> {
        if project.is_empty() || project.len() > 128 || project.chars().any(char::is_control) {
            return Err(Error::InvalidProject);
        }
        if self.entries.len() >= self.limits.max_handles || self.next == u64::MAX {
            return Err(Error::HandleQuota);
        }
        Ok(())
    }
    fn insert(&mut self, project: &str, metadata: Metadata, source: Source) -> Handle {
        let slot = self.next;
        self.next += 1;
        self.entries.insert(
            slot,
            Entry {
                project: project.into(),
                metadata,
                source,
            },
        );
        Handle {
            owner: self.owner.clone(),
            slot,
        }
    }
    fn entry(&self, project: &str, handle: &Handle) -> Result<&Entry> {
        if !Arc::ptr_eq(&self.owner, &handle.owner) {
            return Err(Error::InvalidHandle);
        }
        let entry = self.entries.get(&handle.slot).ok_or(Error::InvalidHandle)?;
        if entry.project != project {
            return Err(Error::WrongProject);
        }
        Ok(entry)
    }
    /// Descriptive metadata, with no local path or browser filename.
    pub fn metadata(&self, project: &str, handle: &Handle) -> Result<Metadata> {
        Ok(self.entry(project, handle)?.metadata.clone())
    }
    /// Revokes every clone of this capability. Another project's capability cannot be revoked.
    pub fn revoke(&mut self, project: &str, handle: &Handle) -> Result<()> {
        self.entry(project, handle)?;
        self.entries.remove(&handle.slot);
        Ok(())
    }
    /// Revokes all media when the host closes a project.
    pub fn revoke_project(&mut self, project: &str) {
        self.entries.retain(|_, e| e.project != project);
    }
    fn range(&self, project: &str, handle: &Handle, offset: u64, length: usize) -> Result<u64> {
        let entry = self.entry(project, handle)?;
        if length > self.limits.max_read_bytes {
            return Err(Error::ReadLimit);
        }
        let end = offset
            .checked_add(u64::try_from(length).map_err(|_| Error::Bounds)?)
            .ok_or(Error::Bounds)?;
        if end > entry.metadata.size_bytes {
            return Err(Error::Bounds);
        }
        Ok(end)
    }
    /// Opens and retains an ordinary local file selected by the host. Symlinks follow OS rules.
    /// MIME is host-supplied descriptive data, not content verification. No paths are retained.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn register_native(
        &mut self,
        project: &str,
        path: &std::path::Path,
        mime: &str,
    ) -> Result<Handle> {
        self.registration(project)?;
        let mime = checked_mime(mime)?;
        let file = std::fs::File::open(path).map_err(|_| Error::Io)?;
        let meta = file.metadata().map_err(|_| Error::Io)?;
        if !meta.is_file() {
            return Err(Error::Io);
        }
        Ok(self.insert(
            project,
            Metadata {
                size_bytes: meta.len(),
                mime,
            },
            Source {
                file,
                modified: meta.modified().ok(),
            },
        ))
    }
    /// Reads only the requested range. Bounds/quota checks run before allocation or I/O.
    /// Changed length/mtime invalidates the capability. This is detection, not an immutable snapshot:
    /// same-size edits with a restored/coarse timestamp may be undetectable; no content hash is made.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn read(
        &mut self,
        project: &str,
        handle: &Handle,
        offset: u64,
        length: usize,
    ) -> Result<Vec<u8>> {
        use std::io::{Read, Seek, SeekFrom};
        self.range(project, handle, offset, length)?;
        let result = (|| {
            let entry = self
                .entries
                .get_mut(&handle.slot)
                .ok_or(Error::InvalidHandle)?;
            let unchanged = |meta: &std::fs::Metadata| {
                meta.len() == entry.metadata.size_bytes
                    && meta.modified().ok() == entry.source.modified
            };
            if !unchanged(&entry.source.file.metadata().map_err(|_| Error::Io)?) {
                return Err(Error::Changed);
            }
            entry
                .source
                .file
                .seek(SeekFrom::Start(offset))
                .map_err(|_| Error::Io)?;
            let mut bytes = allocate(length)?;
            let read = entry.source.file.read_exact(&mut bytes);
            if !unchanged(&entry.source.file.metadata().map_err(|_| Error::Io)?) {
                return Err(Error::Changed);
            }
            read.map_err(|_| Error::Io)?;
            Ok(bytes)
        })();
        if result == Err(Error::Changed) {
            self.entries.remove(&handle.slot);
        }
        result
    }
    /// Accepts an actual host-selected File/Blob, without copying its contents or retaining its name.
    #[cfg(target_arch = "wasm32")]
    pub fn register_blob(&mut self, project: &str, blob: wasm_bindgen::JsValue) -> Result<Handle> {
        self.registration(project)?;
        let size = browser::size(&blob).map_err(|_| Error::InvalidBlob)?;
        if !size.is_finite() || size < 0.0 || size.fract() != 0.0 || size > 9_007_199_254_740_991.0
        {
            return Err(Error::InvalidBlob);
        }
        let mime = checked_mime(&browser::mime(&blob).map_err(|_| Error::InvalidBlob)?)?;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let size_bytes = size as u64;
        Ok(self.insert(project, Metadata { size_bytes, mime }, Source { blob }))
    }
    /// Blob.slice().arrayBuffer() reads at most one validated block; never the entire input Blob.
    /// File/Blob values are immutable browser snapshots; reload requires host reselection.
    #[cfg(target_arch = "wasm32")]
    pub async fn read(
        &mut self,
        project: &str,
        handle: &Handle,
        offset: u64,
        length: usize,
    ) -> Result<Vec<u8>> {
        let end = self.range(project, handle, offset, length)?;
        let entry = self.entry(project, handle)?;
        #[allow(clippy::cast_precision_loss)]
        let promise =
            browser::slice(&entry.source.blob, offset as f64, end as f64).map_err(|_| Error::Io)?;
        let buffer = wasm_bindgen_futures::JsFuture::from(promise)
            .await
            .map_err(|_| Error::Io)?;
        let view = js_sys::Uint8Array::new(&buffer);
        if view.length() as usize != length {
            return Err(Error::Changed);
        }
        let mut bytes = allocate(length)?;
        view.copy_to(&mut bytes);
        Ok(bytes)
    }
}
fn checked_mime(mime: &str) -> Result<String> {
    if mime.is_empty() {
        return Ok("application/octet-stream".into());
    }
    if mime.len() > 255
        || !mime.is_ascii()
        || mime.bytes().any(|c| c.is_ascii_control())
        || !mime.contains('/')
    {
        return Err(Error::InvalidMime);
    }
    Ok(mime.into())
}
fn allocate(length: usize) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| Error::Allocation)?;
    bytes.resize(length, 0);
    Ok(bytes)
}
#[cfg(target_arch = "wasm32")]
mod browser {
    use wasm_bindgen::prelude::*;
    // Call the built-in Blob methods/getters: do not trust overridden properties on host objects.
    #[wasm_bindgen(
        inline_js = "export function kx_media_size(b) { return Object.getOwnPropertyDescriptor(Blob.prototype,'size').get.call(b); } export function kx_media_mime(b) { return Object.getOwnPropertyDescriptor(Blob.prototype,'type').get.call(b); } export function kx_media_slice(b,start,end) { const part = Blob.prototype.slice.call(b,start,end); return Blob.prototype.arrayBuffer.call(part); }"
    )]
    extern "C" {
        #[wasm_bindgen(catch, js_name = kx_media_size)]
        pub fn size(blob: &JsValue) -> std::result::Result<f64, JsValue>;
        #[wasm_bindgen(catch, js_name = kx_media_mime)]
        pub fn mime(blob: &JsValue) -> std::result::Result<String, JsValue>;
        #[wasm_bindgen(catch, js_name = kx_media_slice)]
        pub fn slice(
            blob: &JsValue,
            start: f64,
            end: f64,
        ) -> std::result::Result<js_sys::Promise, JsValue>;
    }
}
