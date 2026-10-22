//! Tool payload (`.kxp`) and the reserved region of the player (ADR 0035). Pure: no I/O.
//!
//! A payload is `KXPAYLD1`, the body length (u64 LE), the SHA-256 of the body and the body: a
//! file count (u32) then, per file, name length (u32), name, data length (u64), data.
//!
//! The player holds a [`region`](crate::region): a static of fixed capacity that starts with a
//! recognisable header and is otherwise zero. Assembling a tool binary writes the payload into
//! that area **in place** (nothing is appended, nothing moves), so the file stays a valid
//! executable that `codesign` can sign.
use sha2::{Digest, Sha256};

/// First bytes of a payload.
pub const PAYLOAD_MAGIC: &[u8; 8] = b"KXPAYLD1";
/// Bytes before the body: magic, length, hash.
pub const HEADER_LEN: usize = 8 + 8 + 32;
/// Most files one payload may list (a sanity bound for hostile input).
const MAX_FILES: usize = 64;

/// A payload that cannot be built or read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadError(pub String);

impl std::fmt::Display for PayloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for PayloadError {}

fn err<T>(message: impl Into<String>) -> Result<T, PayloadError> {
    Err(PayloadError(message.into()))
}

/// Serialises `files` (name, data) into a payload.
///
/// # Errors
/// More than 64 files, a name longer than 255 bytes, or an empty name.
pub fn encode(files: &[(String, Vec<u8>)]) -> Result<Vec<u8>, PayloadError> {
    if files.len() > MAX_FILES {
        return err("too many files in the payload");
    }
    let mut body = u32::try_from(files.len())
        .unwrap_or(u32::MAX)
        .to_le_bytes()
        .to_vec();
    for (name, data) in files {
        if name.is_empty() || name.len() > 255 {
            return err(format!("invalid payload file name `{name}`"));
        }
        body.extend_from_slice(&(name.len() as u32).to_le_bytes());
        body.extend_from_slice(name.as_bytes());
        body.extend_from_slice(&(data.len() as u64).to_le_bytes());
        body.extend_from_slice(data);
    }
    let mut out = Vec::with_capacity(HEADER_LEN + body.len());
    out.extend_from_slice(PAYLOAD_MAGIC);
    out.extend_from_slice(&(body.len() as u64).to_le_bytes());
    out.extend_from_slice(&Sha256::digest(&body));
    out.extend_from_slice(&body);
    Ok(out)
}

/// Total length of the payload that starts `bytes` (header included), without reading its body.
///
/// # Errors
/// `bytes` does not start with a payload header.
pub fn declared_len(bytes: &[u8]) -> Result<usize, PayloadError> {
    if bytes.len() < HEADER_LEN || &bytes[..8] != PAYLOAD_MAGIC {
        return err("no payload");
    }
    let mut length = [0_u8; 8];
    length.copy_from_slice(&bytes[8..16]);
    usize::try_from(u64::from_le_bytes(length))
        .ok()
        .and_then(|body| body.checked_add(HEADER_LEN))
        .map_or_else(|| err("payload length out of range"), Ok)
}

/// Reads a payload: checks the header, the length and the SHA-256, then splits the files.
///
/// # Errors
/// Missing header, truncated or altered body, malformed file table.
pub fn decode(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, PayloadError> {
    let total = declared_len(bytes)?;
    if total > bytes.len() {
        return err("payload truncated");
    }
    let body = &bytes[HEADER_LEN..total];
    if Sha256::digest(body).as_slice() != &bytes[16..HEADER_LEN] {
        return err("payload hash mismatch (corrupted or altered)");
    }
    let mut at = 0_usize;
    let take = |at: &mut usize, n: usize| -> Result<&[u8], PayloadError> {
        let end = at.checked_add(n).filter(|e| *e <= body.len());
        let end = end.map_or_else(|| err("payload file table truncated"), Ok)?;
        let slice = &body[*at..end];
        *at = end;
        Ok(slice)
    };
    let mut count = [0_u8; 4];
    count.copy_from_slice(take(&mut at, 4)?);
    let count = u32::from_le_bytes(count) as usize;
    if count > MAX_FILES {
        return err("too many files in the payload");
    }
    let mut files = Vec::with_capacity(count);
    for _ in 0..count {
        let mut name_len = [0_u8; 4];
        name_len.copy_from_slice(take(&mut at, 4)?);
        let name =
            String::from_utf8(take(&mut at, u32::from_le_bytes(name_len) as usize)?.to_vec())
                .map_or_else(|_| err("payload file name is not UTF-8"), Ok)?;
        let mut data_len = [0_u8; 8];
        data_len.copy_from_slice(take(&mut at, 8)?);
        let data_len = usize::try_from(u64::from_le_bytes(data_len))
            .map_or_else(|_| err("payload file too large"), Ok)?;
        files.push((name, take(&mut at, data_len)?.to_vec()));
    }
    if at != body.len() {
        return err("bytes after the last payload file");
    }
    Ok(files)
}

/// The reserved region of the player.
pub mod region {
    use super::{PayloadError, declared_len, err};

    /// Marks the start of the region in the binary.
    pub const MAGIC: &[u8; 16] = b"KXPAYLOAD-REGN-1";
    /// Bytes that identify an *empty* region for the assembler: magic, capacity, 64 zero bytes.
    const PROBE_ZEROS: usize = 64;
    /// Header of the region: magic and capacity (u64 LE).
    pub const HEADER_LEN: usize = 16 + 8;

    /// Capacity in bytes (`KORTEX_REGION_KIB` at build time, 4096 KiB by default).
    pub const CAPACITY: usize = {
        let text = match option_env!("KORTEX_REGION_KIB") {
            Some(text) => text.as_bytes(),
            None => b"4096",
        };
        let mut kib = 0_usize;
        let mut i = 0;
        while i < text.len() {
            kib = kib * 10 + (text[i] - b'0') as usize;
            i += 1;
        }
        kib * 1024
    };

    /// Offset of the payload area and its capacity in `image` (the whole executable file).
    ///
    /// # Errors
    /// No empty region, or more than one (an ambiguous or already assembled binary).
    pub fn locate(image: &[u8]) -> Result<(usize, usize), PayloadError> {
        let mut found = None;
        let mut from = 0;
        while let Some(at) = image[from..]
            .windows(MAGIC.len())
            .position(|w| w == MAGIC)
            .map(|p| p + from)
        {
            from = at + 1;
            let Some(header) = image.get(at..at + HEADER_LEN + PROBE_ZEROS) else {
                continue;
            };
            let mut capacity = [0_u8; 8];
            capacity.copy_from_slice(&header[16..24]);
            let capacity = u64::from_le_bytes(capacity) as usize;
            let area = at + HEADER_LEN;
            if header[HEADER_LEN..].iter().all(|b| *b == 0)
                && capacity >= PROBE_ZEROS
                && area + capacity <= image.len()
            {
                if found.is_some() {
                    return err("several payload regions in the player");
                }
                found = Some((area, capacity));
            }
        }
        found.map_or_else(
            || err("no empty payload region in this binary (is it a pristine player?)"),
            Ok,
        )
    }

    /// Writes `payload` into the region of `image`, in place.
    ///
    /// # Errors
    /// No empty region, or the payload is larger than the region.
    pub fn fill(image: &mut [u8], payload: &[u8]) -> Result<(), PayloadError> {
        let (area, capacity) = locate(image)?;
        declared_len(payload)?;
        if payload.len() > capacity {
            return err(format!(
                "payload of {} bytes does not fit the {capacity}-byte region of the player; rebuild the player with a larger KORTEX_REGION_KIB",
                payload.len()
            ));
        }
        image[area..area + payload.len()].copy_from_slice(payload);
        Ok(())
    }
}

#[cfg(feature = "region")]
mod embedded {
    use super::region::{CAPACITY, MAGIC};
    use super::{PAYLOAD_MAGIC, declared_len};

    #[repr(C)]
    struct Region {
        magic: [u8; 16],
        capacity: [u8; 8],
        area: [u8; CAPACITY],
    }

    static REGION: Region = Region {
        magic: *MAGIC,
        capacity: (CAPACITY as u64).to_le_bytes(),
        area: [0; CAPACITY],
    };

    /// The payload written into this binary, if any (not yet verified: use [`super::decode`]).
    #[must_use]
    pub fn embedded() -> Option<&'static [u8]> {
        // The compiler must not assume the area is still the zeros it was initialised with:
        // the assembler rewrites it in the file. `black_box` hides the static's value and the
        // reads are volatile.
        let region: &'static Region = std::hint::black_box(&REGION);
        let area = std::ptr::from_ref(&region.area).cast::<u8>();
        let read = |at: usize| -> u8 {
            // SAFETY: `at` is below `CAPACITY`, inside the static.
            unsafe { std::ptr::read_volatile(area.add(at)) }
        };
        if (0..PAYLOAD_MAGIC.len()).any(|i| read(i) != PAYLOAD_MAGIC[i]) {
            return None;
        }
        let header: Vec<u8> = (0..super::HEADER_LEN).map(read).collect();
        let total = declared_len(&header).ok().filter(|t| *t <= CAPACITY)?;
        // SAFETY: `total` bytes starting at `area` lie inside the static.
        Some(unsafe { std::slice::from_raw_parts(area, total) })
    }
}

#[cfg(feature = "region")]
pub use embedded::embedded;

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<(String, Vec<u8>)> {
        vec![
            ("tool.toml".to_owned(), b"[tool]".to_vec()),
            ("locales/fr.toml".to_owned(), vec![]),
            ("engine.wasm".to_owned(), (0..=255).collect()),
        ]
    }

    #[test]
    fn payload_round_trips() {
        let bytes = encode(&sample()).unwrap_or_default();
        assert_eq!(decode(&bytes), Ok(sample()));
        assert_eq!(declared_len(&bytes), Ok(bytes.len()));
    }

    #[test]
    fn any_altered_byte_is_refused() {
        let bytes = encode(&sample()).unwrap_or_default();
        for at in [HEADER_LEN, HEADER_LEN + 7, bytes.len() - 1] {
            let mut altered = bytes.clone();
            altered[at] ^= 1;
            assert!(decode(&altered).is_err(), "byte {at}");
        }
        assert!(decode(&bytes[..bytes.len() - 1]).is_err());
        assert!(decode(b"nope").is_err());
        assert!(decode(&[]).is_err());
    }

    #[test]
    fn invalid_inputs_are_rejected() {
        assert!(encode(&[(String::new(), vec![])]).is_err());
        assert!(encode(&[("x".repeat(256), vec![])]).is_err());
        let many: Vec<_> = (0..65).map(|i| (format!("f{i}"), vec![])).collect();
        assert!(encode(&many).is_err());
    }

    fn player(capacity: usize) -> Vec<u8> {
        let mut image = b"\x7fELF-pretend-code....".to_vec();
        image.extend_from_slice(region::MAGIC);
        image.extend_from_slice(&(capacity as u64).to_le_bytes());
        image.extend(std::iter::repeat_n(0, capacity));
        image.extend_from_slice(b"....trailing code and a copy of the magic: ");
        image.extend_from_slice(region::MAGIC);
        image
    }

    #[test]
    fn fill_writes_in_place_without_changing_the_file_size() {
        let mut image = player(4096);
        let before = image.len();
        let payload = encode(&sample()).unwrap_or_default();
        assert_eq!(region::fill(&mut image, &payload), Ok(()));
        assert_eq!(image.len(), before);
        let (area, _) = region::locate(&player(4096)).unwrap_or_default();
        assert_eq!(decode(&image[area..]), Ok(sample()));
        // A second fill refuses: the region is no longer empty.
        assert!(region::fill(&mut image, &payload).is_err());
    }

    #[test]
    fn a_payload_larger_than_the_region_is_refused_with_advice() {
        let mut image = player(128);
        let big = encode(&[("big".to_owned(), vec![1; 1000])]).unwrap_or_default();
        let message = region::fill(&mut image, &big).err().map(|e| e.0);
        assert!(message.is_some_and(|m| m.contains("KORTEX_REGION_KIB")));
    }

    #[test]
    fn a_binary_without_a_region_is_refused() {
        assert!(region::locate(b"just code").is_err());
    }
}
