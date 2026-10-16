use std::cmp::Ordering;

/// `MAJOR.MINOR.PATCH` (no pre-release: every published version is a plain release).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version(pub u64, pub u64, pub u64);

impl Version {
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let mut it = s.trim().split('.');
        let v = Self(
            it.next()?.parse().ok()?,
            it.next()?.parse().ok()?,
            it.next()?.parse().ok()?,
        );
        it.next().is_none().then_some(v)
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.0, self.1, self.2).cmp(&(other.0, other.1, other.2))
    }
}

/// `candidate` is strictly newer than `current` (unparseable versions are never "newer").
#[must_use]
pub fn is_newer(candidate: &str, current: &str) -> bool {
    matches!((Version::parse(candidate), Version::parse(current)), (Some(a), Some(b)) if a > b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordering_is_numeric_not_lexical() {
        assert!(is_newer("1.10.0", "1.9.9"));
        assert!(is_newer("2.0.0", "1.99.99"));
        assert!(!is_newer("1.0.0", "1.0.0"));
        assert!(!is_newer("1.0.0", "1.0.1"));
    }

    #[test]
    fn rejects_garbage() {
        for bad in ["", "1", "1.2", "1.2.3.4", "a.b.c", "1.2.x", "-1.0.0"] {
            assert!(Version::parse(bad).is_none(), "{bad}");
        }
        assert!(!is_newer("garbage", "1.0.0"));
    }
}
