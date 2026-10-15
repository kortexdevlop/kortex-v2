//! Hash of a file (preferred) or a text. Algorithms: sha256, sha1, md5, sha512.
use kortex::runtime::{Cx, ToolError};
use md5::Md5;
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha512};

use crate::{Inputs, Outputs, Params};

fn hex(bytes: &[u8], upper: bool) -> String {
    use std::fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
            let _ = if upper {
                write!(s, "{b:02X}")
            } else {
                write!(s, "{b:02x}")
            };
            s
        })
}

/// Hash `data` in 1 MiB chunks so progress and cancellation stay responsive.
fn digest<D: Digest>(data: &[u8], cx: &mut Cx) -> Result<Vec<u8>, ToolError> {
    let mut h = D::new();
    let total = data.len().max(1);
    for (i, chunk) in data.chunks(1 << 20).enumerate() {
        cx.check_cancelled()?;
        h.update(chunk);
        let done = ((i + 1) << 20).min(total);
        #[allow(clippy::cast_precision_loss)]
        cx.progress(done as f32 / total as f32);
    }
    Ok(h.finalize().to_vec())
}

#[kortex::engine]
pub async fn run(input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    let data: &[u8] = match (&input.file, &input.text) {
        (Some(f), _) => &f.bytes,
        (None, Some(t)) if !t.is_empty() => t.as_bytes(),
        _ => return Err(ToolError::new("missing_input")),
    };
    let raw = match params.algorithm.as_str() {
        "sha256" => digest::<Sha256>(data, cx)?,
        "sha1" => digest::<Sha1>(data, cx)?,
        "md5" => digest::<Md5>(data, cx)?,
        "sha512" => digest::<Sha512>(data, cx)?,
        _ => {
            return Err(ToolError::with_detail(
                ToolError::INVALID_PARAM,
                "algorithm",
            ));
        }
    };
    let hash = hex(&raw, params.uppercase);
    let expected = params.compare_with.trim();
    let verdict = if expected.is_empty() {
        "none"
    } else if expected.eq_ignore_ascii_case(&hash) {
        "match"
    } else {
        "mismatch"
    };
    Ok(Outputs {
        hash,
        verdict: verdict.to_owned(),
    })
}
