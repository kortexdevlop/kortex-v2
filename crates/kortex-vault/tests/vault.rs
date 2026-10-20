use kortex_vault::{ROUNDS, Vault, VaultError, new_salt};

const SALT: &str = "00112233445566778899aabbccddeeff";
const FAST: u32 = 1_000;

fn vault(passphrase: &str) -> Vault {
    Vault::derive_with(passphrase, SALT, FAST).expect("vault")
}

#[test]
fn what_is_sealed_opens_with_the_same_passphrase_and_looks_like_noise() {
    let a = vault("correct horse battery staple");
    let plain = br#"{"name":"Mon workflow secret","nodes":[]}"#;
    let blob = a.seal(plain, [7; 12]);
    assert!(blob.starts_with("v1."));
    assert!(
        !blob.contains("workflow") && !blob.contains("secret"),
        "no plaintext in the blob"
    );
    assert_eq!(a.open(&blob).expect("open"), plain);
    // Another vault derived the same way reads it too (another device).
    assert_eq!(
        vault("correct horse battery staple")
            .open(&blob)
            .expect("open"),
        plain
    );
}

#[test]
fn a_wrong_passphrase_or_a_modified_blob_is_rejected_never_garbled() {
    let good = vault("right");
    let blob = good.seal(b"data", [1; 12]);
    assert_eq!(vault("wrong").open(&blob), Err(VaultError::Rejected));
    let mut parts: Vec<String> = blob.split('.').map(str::to_owned).collect();
    let last = parts[2].len() - 1;
    let flipped = if parts[2].ends_with('0') { '1' } else { '0' };
    parts[2].replace_range(last.., &flipped.to_string());
    assert_eq!(
        good.open(&parts.join(".")),
        Err(VaultError::Rejected),
        "the authentication tag catches a change"
    );
    // A different salt is a different key.
    assert_eq!(
        Vault::derive_with("right", "ffeeddccbbaa99887766554433221100", FAST)
            .expect("vault")
            .open(&blob),
        Err(VaultError::Rejected)
    );
}

#[test]
fn malformed_input_is_told_apart_from_a_wrong_key() {
    let v = vault("x");
    for bad in [
        "",
        "v2.00.00",
        "v1.zz.00",
        "v1.0011",
        "plain text",
        "v1.000000000000000000000000.zz",
        "v1.00.00.00",
    ] {
        assert_eq!(v.open(bad), Err(VaultError::Malformed), "{bad}");
    }
    assert_eq!(
        Vault::derive("p", "not-hex").err(),
        Some(VaultError::Malformed)
    );
    assert_eq!(
        Vault::derive_with("p", "00", FAST).err(),
        Some(VaultError::Malformed)
    );
}

#[test]
fn the_check_blob_proves_the_passphrase_and_nonces_make_blobs_differ() {
    let a = vault("passphrase");
    let check = a.check_blob([9; 12]);
    assert!(a.accepts(&check));
    assert!(!vault("other").accepts(&check));
    assert!(
        !a.accepts(&a.seal(b"something else", [9; 12])),
        "only the known text counts"
    );
    assert_ne!(a.seal(b"same", [1; 12]), a.seal(b"same", [2; 12]));
    // A salt comes out of the caller's randomness as 32 hex digits.
    let salt = new_salt(|bytes| bytes.fill(0xab));
    assert_eq!(salt, "ab".repeat(16));
    const { assert!(ROUNDS >= 300_000, "the real work factor stays high") };
}
