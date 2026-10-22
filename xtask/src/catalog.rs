//! Opt-in local consumer and release writer for the modular catalogue.
use kortex_catalog::{
    Floor,
    modular::{Catalog, MAX_FRAGMENT_BYTES, MAX_INDEX_BYTES},
    sign_bytes,
};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
};

pub fn publish(legacy: &str, seed: &[u8; 32], root: &Path, key: &str) -> Result<(), String> {
    let document: serde_json::Value = serde_json::from_str(legacy).map_err(|e| e.to_string())?;
    let contracts = document["tools"].as_array().ok_or("missing contracts")?;
    let channel = document["channel"].as_str().ok_or("missing channel")?;
    let sequence = document["sequence"].as_u64().ok_or("missing sequence")?;
    let publication =
        kortex_catalog::modular::build(contracts, channel, sequence).map_err(|e| e.to_string())?;
    let signature = sign_bytes(seed, &publication.index);
    let verified = Catalog::open(&publication.index, &signature, key, channel, None)
        .map_err(|e| e.to_string())?;
    fs::create_dir_all(root.join("contracts")).map_err(|e| e.to_string())?;
    for (id, descriptor) in verified.entries() {
        let bytes = &publication.fragments[&descriptor.path];
        verified.resolve(id, bytes).map_err(|e| e.to_string())?;
        fs::write(root.join(&descriptor.path), bytes).map_err(|e| e.to_string())?;
    }
    let archive =
        fs::File::create(root.join("catalog-contracts-v2.tar.gz")).map_err(|e| e.to_string())?;
    let mut archive = tar::Builder::new(flate2::write::GzEncoder::new(
        archive,
        flate2::Compression::default(),
    ));
    for (path, bytes) in &publication.fragments {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_mtime(0);
        header.set_cksum();
        archive
            .append_data(&mut header, path, bytes.as_slice())
            .map_err(|e| e.to_string())?;
    }
    archive
        .into_inner()
        .map_err(|e| e.to_string())?
        .finish()
        .map_err(|e| e.to_string())?;
    // Immutable fragments precede the signed root. Deployment must upload fragments first too.
    fs::write(root.join("catalog-index-v2.json.sig"), signature).map_err(|e| e.to_string())?;
    fs::write(root.join("catalog-index-v2.json"), publication.index).map_err(|e| e.to_string())?;
    Ok(())
}

fn bounded(path: &Path, max: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .take(max as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > max {
        return Err("catalogue resource exceeds byte limit".into());
    }
    Ok(bytes)
}

fn flag<'a>(args: &'a [String], name: &str) -> Result<&'a str, String> {
    args.windows(2)
        .find(|p| p[0] == name)
        .map(|p| p[1].as_str())
        .ok_or_else(|| format!("missing {name}"))
}

/// `catalog-resolve --root DIR --id ID --state FILE [--channel stable] [--public-key FILE]`.
/// Uses the embedded release key by default. Alternate keys are an explicit operator trust choice.
/// A failed refresh does not overwrite state; no insecure automatic downgrade to legacy v1.
pub fn resolve(args: &[String]) -> Result<(), String> {
    let root = Path::new(flag(args, "--root")?)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let id = flag(args, "--id")?;
    let state_path = Path::new(flag(args, "--state")?);
    // OS lock survives process races and is released by closing the file on every return path.
    let state_lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(state_path.with_added_extension("lock"))
        .map_err(|e| e.to_string())?;
    state_lock.lock().map_err(|e| e.to_string())?;
    let floor: Option<Floor> = if state_path.exists() {
        Some(
            serde_json::from_slice(&bounded(state_path, 4096)?)
                .map_err(|e| format!("invalid floor: {e}"))?,
        )
    } else {
        None
    };
    let key = match flag(args, "--public-key") {
        Ok(path) => String::from_utf8(bounded(Path::new(path), 256)?).map_err(|e| e.to_string())?,
        Err(_) => kortex_catalog::RELEASE_PUBLIC_KEY.to_owned(),
    };
    let bytes = bounded(&root.join("catalog-index-v2.json"), MAX_INDEX_BYTES)?;
    let signature = String::from_utf8(bounded(&root.join("catalog-index-v2.json.sig"), 256)?)
        .map_err(|e| e.to_string())?;
    let catalog = Catalog::open(
        &bytes,
        signature.trim(),
        key.trim(),
        flag(args, "--channel").unwrap_or("stable"),
        floor.as_ref(),
    )
    .map_err(|e| e.to_string())?;
    let descriptor = catalog.descriptor(id).ok_or("unknown tool")?;
    let fragment = root
        .join(&descriptor.path)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if !fragment.starts_with(&root) {
        return Err("fragment escapes catalogue root".into());
    }
    let contract = catalog
        .resolve(
            id,
            &bounded(&fragment, descriptor.bytes.min(MAX_FRAGMENT_BYTES))?,
        )
        .map_err(|e| e.to_string())?;
    // Record rollback protection only after a successful resolution.
    let temporary = state_path.with_added_extension("tmp");
    let mut state = fs::File::create(&temporary).map_err(|e| e.to_string())?;
    state
        .write_all(&serde_json::to_vec(&catalog.accepted.floor).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    state.sync_all().map_err(|e| e.to_string())?;
    drop(state);
    fs::rename(&temporary, state_path).map_err(|e| e.to_string())?;
    println!(
        "{}",
        serde_json::to_string_pretty(&contract).map_err(|e| e.to_string())?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn concurrent_resolvers_never_regress_the_floor() {
        let root = std::env::temp_dir().join(format!("kortex-catalog-race-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let seed = [92; 32];
        let key = kortex_catalog::public_key_b64(&seed);
        fs::write(root.join("test.pub"), &key).unwrap();
        let contract = serde_json::json!({"schema":1,"tool":{"id":"test.one","version":"1.0.0","name":{"en":"Test","fr":"Test"},"universe":"test","platforms":["web","desktop"]}});
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let mut handles = Vec::new();
        for sequence in [1, 2] {
            let directory = root.join(sequence.to_string());
            let document =
                serde_json::json!({"channel":"stable","sequence":sequence,"tools":[contract]})
                    .to_string();
            publish(&document, &seed, &directory, &key).unwrap();
            let args = vec![
                "--root".into(),
                directory.to_string_lossy().into_owned(),
                "--id".into(),
                "test.one".into(),
                "--state".into(),
                root.join("floor.json").to_string_lossy().into_owned(),
                "--public-key".into(),
                root.join("test.pub").to_string_lossy().into_owned(),
            ];
            let barrier = barrier.clone();
            handles.push(std::thread::spawn(move || {
                barrier.wait();
                (sequence, resolve(&args))
            }));
        }
        for handle in handles {
            let (sequence, result) = handle.join().unwrap();
            if sequence == 2 {
                result.unwrap();
            }
        }
        let floor: Floor =
            serde_json::from_slice(&fs::read(root.join("floor.json")).unwrap()).unwrap();
        assert_eq!(floor.sequence, 2);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn release_writer_and_lazy_cli_consumer_roundtrip_real_registry() {
        let root = std::env::temp_dir().join(format!("kortex-catalog-test-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let seed = [91; 32];
        let key = kortex_catalog::public_key_b64(&seed);
        let registry: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(
                Path::new(env!("CARGO_MANIFEST_DIR")).join("../registry/contracts.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(!registry["tools"].as_array().unwrap().is_empty());
        let legacy = serde_json::json!({"schema":1,"channel":"stable","sequence":1,"tools":registry["tools"]}).to_string();
        publish(&legacy, &seed, &root, &key).unwrap();
        let archive = fs::File::open(root.join("catalog-contracts-v2.tar.gz")).unwrap();
        let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(archive));
        let members: Vec<_> = archive
            .entries()
            .unwrap()
            .map(|entry| {
                let mut entry = entry.unwrap();
                let path = entry.path().unwrap().into_owned();
                let mut bytes = Vec::new();
                entry.read_to_end(&mut bytes).unwrap();
                assert_eq!(fs::read(root.join(&path)).unwrap(), bytes);
                path
            })
            .collect();
        assert_eq!(members.len(), registry["tools"].as_array().unwrap().len());
        fs::write(root.join("test.pub"), key).unwrap();
        let args = vec![
            "--root".into(),
            root.to_string_lossy().into_owned(),
            "--id".into(),
            "calc.age".into(),
            "--state".into(),
            root.join("floor.json").to_string_lossy().into_owned(),
            "--public-key".into(),
            root.join("test.pub").to_string_lossy().into_owned(),
        ];
        resolve(&args).unwrap();
        let floor_before = fs::read(root.join("floor.json")).unwrap();
        let index_bytes = fs::read(root.join("catalog-index-v2.json")).unwrap();
        let index: serde_json::Value = serde_json::from_slice(&index_bytes).unwrap();
        // Other contracts are not opened: deleting one cannot prevent the selected resolution.
        let unused = index["tools"]["text.reverse"]["path"].as_str().unwrap();
        fs::remove_file(root.join(unused)).unwrap();
        resolve(&args).unwrap();
        let selected = index["tools"]["calc.age"]["path"].as_str().unwrap();
        fs::write(root.join(selected), b"corrupted").unwrap();
        assert!(resolve(&args).is_err());
        assert_eq!(floor_before, fs::read(root.join("floor.json")).unwrap());
        // Test-owned temporary directory only.
        fs::remove_dir_all(&root).unwrap();
    }
}
