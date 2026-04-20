//! Email PII at-rest encryption + deterministic lookup hash + display mask.
//!
//! Design §Email & PII storage:
//!   * `email_ct`   — AES-256-GCM ciphertext, nonce (12 bytes) prepended.
//!   * `email_hash` — HMAC-SHA256(normalized email) for uniqueness + lookup.
//!   * `email_mask` — user-safe display string, e.g. `j***@e***.com`.
//!
//! We never store plaintext email. Login is **username-first** (see
//! `handlers::auth` A1); the email hash is used for admin-side lookup
//! and uniqueness, not for sign-in. Display surfaces show the mask
//! unless the caller has `user.manage` (and even then the decrypted
//! email is fetched per request, never cached).

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use hmac::{Hmac, Mac};
use rand::{rngs::OsRng, RngCore};
use sha2::Sha256;
use subtle::ConstantTimeEq;

type HmacSha256 = Hmac<Sha256>;

pub fn normalize_email(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

pub fn encrypt_email(plaintext: &str, enc_key: &[u8; 32]) -> anyhow::Result<Vec<u8>> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(enc_key));
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ct = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| anyhow::anyhow!("aes-gcm encrypt: {e}"))?;
    let mut out = Vec::with_capacity(12 + ct.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ct);
    Ok(out)
}

pub fn decrypt_email(blob: &[u8], enc_key: &[u8; 32]) -> anyhow::Result<String> {
    if blob.len() < 12 + 16 {
        anyhow::bail!("email ciphertext too short");
    }
    let (nonce_bytes, ct) = blob.split_at(12);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(enc_key));
    let pt = cipher
        .decrypt(Nonce::from_slice(nonce_bytes), ct)
        .map_err(|e| anyhow::anyhow!("aes-gcm decrypt: {e}"))?;
    Ok(String::from_utf8(pt).map_err(|e| anyhow::anyhow!("email utf-8: {e}"))?)
}

pub fn email_hash(normalized_email: &str, hmac_key: &[u8; 32]) -> [u8; 32] {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(hmac_key).expect("hmac sha256 any key");
    mac.update(normalized_email.as_bytes());
    let out = mac.finalize().into_bytes();
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&out);
    arr
}

pub fn email_hashes_eq(a: &[u8], b: &[u8]) -> bool {
    a.ct_eq(b).into()
}

/// Produce a display-safe mask. Keeps the first character of the local part
/// and the first character of the domain label; hides the rest. Preserves
/// the TLD so users recognize their own address without revealing the rest.
pub fn email_mask(email: &str) -> String {
    let lower = email.trim().to_ascii_lowercase();
    let Some((local, domain)) = lower.split_once('@') else {
        return "***".to_string();
    };
    let local_first = local.chars().next().unwrap_or('*');
    let (dom_label, tld) = match domain.rsplit_once('.') {
        Some((lbl, tld)) => (lbl, tld),
        None => (domain, ""),
    };
    let dom_first = dom_label.chars().next().unwrap_or('*');
    if tld.is_empty() {
        format!("{local_first}***@{dom_first}***")
    } else {
        format!("{local_first}***@{dom_first}***.{tld}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_lowercases_and_trims() {
        assert_eq!(normalize_email("  JANE@Example.COM "), "jane@example.com");
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let k = [7u8; 32];
        let pt = "jane.doe@example.com";
        let ct = encrypt_email(pt, &k).unwrap();
        // nonce prepended + at least tag
        assert!(ct.len() > 12 + 16);
        let back = decrypt_email(&ct, &k).unwrap();
        assert_eq!(back, pt);
    }

    #[test]
    fn decrypt_wrong_key_fails() {
        let k1 = [1u8; 32];
        let k2 = [2u8; 32];
        let ct = encrypt_email("x@y.z", &k1).unwrap();
        assert!(decrypt_email(&ct, &k2).is_err());
    }

    #[test]
    fn encrypt_is_nondeterministic() {
        let k = [9u8; 32];
        let a = encrypt_email("jane@example.com", &k).unwrap();
        let b = encrypt_email("jane@example.com", &k).unwrap();
        assert_ne!(a, b, "fresh nonce per call");
    }

    #[test]
    fn hmac_stable_and_key_dependent() {
        let k1 = [3u8; 32];
        let k2 = [4u8; 32];
        let h1 = email_hash("jane@example.com", &k1);
        let h2 = email_hash("jane@example.com", &k1);
        let h3 = email_hash("jane@example.com", &k2);
        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
        assert!(email_hashes_eq(&h1, &h2));
    }

    #[test]
    fn mask_basic() {
        assert_eq!(email_mask("jane@example.com"), "j***@e***.com");
        assert_eq!(email_mask("a@b.co"), "a***@b***.co");
    }

    #[test]
    fn mask_handles_missing_at() {
        assert_eq!(email_mask("notanemail"), "***");
    }
}
