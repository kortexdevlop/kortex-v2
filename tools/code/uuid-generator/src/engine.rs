use kortex::runtime::{Cx, ToolError};

use crate::{Inputs, Outputs, Params};

fn format(b: &[u8; 16], upper: bool, hyphens: bool) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(36);
    for (i, byte) in b.iter().enumerate() {
        if hyphens && matches!(i, 4 | 6 | 8 | 10) {
            s.push('-');
        }
        let _ = if upper {
            write!(s, "{byte:02X}")
        } else {
            write!(s, "{byte:02x}")
        };
    }
    s
}

#[kortex::engine]
pub async fn run(_input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    let count =
        usize::try_from(params.count).map_err(|_| ToolError::new(ToolError::INVALID_PARAM))?;
    if !matches!(params.version.as_str(), "v4" | "v7") {
        return Err(ToolError::with_detail(ToolError::INVALID_PARAM, "version"));
    }
    let mut lines = Vec::with_capacity(count);
    for _ in 0..count {
        cx.check_cancelled()?;
        let mut b = [0_u8; 16];
        cx.fill_random(&mut b)?;
        if params.version == "v7" {
            // RFC 9562: 48-bit Unix time in milliseconds, then version 7 and random bits.
            let ms = cx.now_ms();
            b[..6].copy_from_slice(&ms.to_be_bytes()[2..]);
            b[6] = (b[6] & 0x0F) | 0x70;
        } else {
            b[6] = (b[6] & 0x0F) | 0x40;
        }
        b[8] = (b[8] & 0x3F) | 0x80; // RFC 4122 variant
        lines.push(format(&b, params.uppercase, params.hyphens));
    }
    Ok(Outputs {
        uuids: lines.join("\n"),
    })
}
