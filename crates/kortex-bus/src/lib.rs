//! Typed host handshake. Value transfers and native IPC remain separate protocol work.
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ready {
    pub version: u8,
    pub frame: u64,
}
impl Ready {
    pub fn accepts(&self, expected: u64) -> bool {
        self.version == 1 && self.frame == expected
    }
}
#[derive(Clone, Serialize)]
pub struct Init {
    pub lang: String,
    pub theme: String,
    pub mode: String,
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn handshake_rejects_wrong_frame_and_version() {
        assert!(
            Ready {
                version: 1,
                frame: 4
            }
            .accepts(4)
        );
        assert!(
            !Ready {
                version: 2,
                frame: 4
            }
            .accepts(4)
        );
        assert!(
            !Ready {
                version: 1,
                frame: 5
            }
            .accepts(4)
        );
    }
}
