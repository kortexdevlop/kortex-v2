//! Passphrase-based encryption for synced data (AES-256-GCM, key from PBKDF2-HMAC-SHA256).
//!
//! A [`Vault`] is derived once from the passphrase and a random salt that is stored next to the
//! data (the salt is not secret). Every sealed blob carries its own random 96-bit nonce and an
//! authentication tag: a wrong passphrase or a modified blob fails to open, it never yields
//! garbage. The blob is plain text (`v1.<nonce hex>.<ciphertext hex>`) so it can travel in JSON.
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use sha2::Sha256;

/// PBKDF2 rounds: about a second on a phone, slow enough to make guessing a passphrase costly.
pub const ROUNDS: u32 = 300_000;
const NONCE_LEN: usize = 12;
const SALT_LEN: usize = 16;
const KEY_LEN: usize = 32;
/// What the passphrase check seals; opening it proves the passphrase is the right one.
pub const CHECK_TEXT: &[u8] = b"kortex-vault-check-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultError {
    /// Not a blob of this format.
    Malformed,
    /// Wrong passphrase, or the blob was changed.
    Rejected,
}

pub struct Vault {
    cipher: Aes256Gcm,
}

/// A fresh random salt from caller-provided randomness (`fill` writes random bytes).
pub fn new_salt(fill: impl FnOnce(&mut [u8])) -> String {
    let mut salt = [0_u8; SALT_LEN];
    fill(&mut salt);
    hex(&salt)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) || !text.is_ascii() {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).ok())
        .collect()
}

impl Vault {
    /// Derives the key. Costs [`ROUNDS`] hash iterations: do it once and keep the vault.
    ///
    /// # Errors
    /// [`VaultError::Malformed`] when the salt is not 32 hex digits.
    pub fn derive(passphrase: &str, salt_hex: &str) -> Result<Self, VaultError> {
        Self::derive_with(passphrase, salt_hex, ROUNDS)
    }

    /// Like [`Self::derive`] with a chosen number of rounds (tests use few).
    ///
    /// # Errors
    /// [`VaultError::Malformed`] when the salt is not 32 hex digits.
    pub fn derive_with(passphrase: &str, salt_hex: &str, rounds: u32) -> Result<Self, VaultError> {
        let salt = unhex(salt_hex)
            .filter(|s| s.len() == SALT_LEN)
            .ok_or(VaultError::Malformed)?;
        let mut key = [0_u8; KEY_LEN];
        pbkdf2::pbkdf2_hmac::<Sha256>(passphrase.as_bytes(), &salt, rounds, &mut key);
        let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| VaultError::Malformed)?;
        Ok(Self { cipher })
    }

    /// Encrypts with a nonce the caller drew at random; never reuse a nonce with one key.
    #[must_use]
    pub fn seal(&self, plaintext: &[u8], nonce: [u8; NONCE_LEN]) -> String {
        // Encrypting an in-memory buffer cannot fail; an empty ciphertext would not open anyway.
        let sealed = self
            .cipher
            .encrypt(Nonce::from_slice(&nonce), plaintext)
            .unwrap_or_default();
        format!("v1.{}.{}", hex(&nonce), hex(&sealed))
    }

    /// # Errors
    /// [`VaultError::Malformed`] for a blob of another format, [`VaultError::Rejected`] for a
    /// wrong passphrase or a modified blob.
    pub fn open(&self, blob: &str) -> Result<Vec<u8>, VaultError> {
        let mut parts = blob.split('.');
        let (Some("v1"), Some(nonce), Some(sealed), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(VaultError::Malformed);
        };
        let nonce = unhex(nonce)
            .filter(|n| n.len() == NONCE_LEN)
            .ok_or(VaultError::Malformed)?;
        let sealed = unhex(sealed).ok_or(VaultError::Malformed)?;
        self.cipher
            .decrypt(Nonce::from_slice(&nonce), sealed.as_slice())
            .map_err(|_| VaultError::Rejected)
    }

    /// The blob that proves a passphrase, to store next to the salt.
    #[must_use]
    pub fn check_blob(&self, nonce: [u8; NONCE_LEN]) -> String {
        self.seal(CHECK_TEXT, nonce)
    }

    /// Whether `blob` (from [`Self::check_blob`]) opens with this vault's passphrase.
    #[must_use]
    pub fn accepts(&self, blob: &str) -> bool {
        self.open(blob).is_ok_and(|text| text == CHECK_TEXT)
    }
}
