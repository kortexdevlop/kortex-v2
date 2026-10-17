//! Build-only verifier for the existing xtask source/output receipt format.
use std::{fs, path::Path};

pub(crate) fn tree_fingerprint(path: &Path) -> Result<String, String> {
    use std::hash::{Hash, Hasher};
    fn visit(path: &Path, base: &Path, hash: &mut impl Hasher) -> Result<(), String> {
        path.strip_prefix(base)
            .map_err(|e| e.to_string())?
            .hash(hash);
        if path.is_dir() {
            let mut entries = fs::read_dir(path)
                .map_err(|e| e.to_string())?
                .map(|e| e.map(|e| e.path()).map_err(|e| e.to_string()))
                .collect::<Result<Vec<_>, _>>()?;
            entries.sort();
            for entry in entries {
                visit(&entry, base, hash)?;
            }
        } else {
            fs::read(path).map_err(|e| e.to_string())?.hash(hash);
        }
        Ok(())
    }
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    visit(path, path, &mut hash)?;
    Ok(format!("{:016x}", hash.finish()))
}

fn main() -> Result<(), String> {
    for path in std::env::args().skip(1) {
        println!("{}", tree_fingerprint(Path::new(&path))?);
    }
    Ok(())
}
