//! HMAC-signed image URLs.
//!
//! Design §Images: every protected image read goes through a signed URL
//! with `exp` ≤ 600s from issuance. The signature covers the path + exp,
//! so the same key can safely sign many different objects without letting
//! a leaked URL grant access to a different path.
//!
//! Query shape: `?exp=<unix_seconds>&sig=<hex(hmac-sha256)>`.

use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;

type HmacSha256 = Hmac<Sha256>;

pub const MAX_TTL_SECONDS: i64 = 600;

fn compute(path: &str, exp: i64, key: &[u8; 32]) -> Vec<u8> {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(key).expect("hmac sha256 any key");
    mac.update(path.as_bytes());
    mac.update(b"|");
    mac.update(exp.to_string().as_bytes());
    mac.finalize().into_bytes().to_vec()
}

/// Produce `?exp=...&sig=...` for the given path. Clamped to 600s.
pub fn sign(path: &str, ttl_seconds: i64, key: &[u8; 32]) -> String {
    let ttl = ttl_seconds.clamp(1, MAX_TTL_SECONDS);
    let exp = Utc::now().timestamp() + ttl;
    let sig = compute(path, exp, key);
    format!("exp={exp}&sig={}", hex::encode(sig))
}

/// Verify a query string for `path`. Returns Ok on success.
pub fn verify(path: &str, exp: i64, sig_hex: &str, key: &[u8; 32]) -> anyhow::Result<()> {
    let now = Utc::now().timestamp();
    if exp < now {
        anyhow::bail!("signed url expired");
    }
    if exp - now > MAX_TTL_SECONDS + 5 {
        anyhow::bail!("signed url exp window too large");
    }
    let want = compute(path, exp, key);
    let got = hex::decode(sig_hex).map_err(|e| anyhow::anyhow!("sig hex: {e}"))?;
    if want.ct_eq(&got).into() {
        Ok(())
    } else {
        anyhow::bail!("signed url signature mismatch")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_qs(qs: &str) -> (i64, String) {
        let mut exp = 0i64;
        let mut sig = String::new();
        for kv in qs.split('&') {
            if let Some(v) = kv.strip_prefix("exp=") {
                exp = v.parse().unwrap();
            } else if let Some(v) = kv.strip_prefix("sig=") {
                sig = v.to_string();
            }
        }
        (exp, sig)
    }

    #[test]
    fn sign_verify_roundtrip() {
        let k = [11u8; 32];
        let qs = sign("/images/products/abc.png", 300, &k);
        let (exp, sig) = parse_qs(&qs);
        verify("/images/products/abc.png", exp, &sig, &k).unwrap();
    }

    #[test]
    fn wrong_path_rejected() {
        let k = [12u8; 32];
        let qs = sign("/images/a.png", 300, &k);
        let (exp, sig) = parse_qs(&qs);
        assert!(verify("/images/b.png", exp, &sig, &k).is_err());
    }

    #[test]
    fn wrong_key_rejected() {
        let k1 = [1u8; 32];
        let k2 = [2u8; 32];
        let qs = sign("/images/a.png", 300, &k1);
        let (exp, sig) = parse_qs(&qs);
        assert!(verify("/images/a.png", exp, &sig, &k2).is_err());
    }

    #[test]
    fn expired_rejected() {
        let k = [3u8; 32];
        let want = compute("/x", 1, &k);
        assert!(verify("/x", 1, &hex::encode(want), &k).is_err());
    }

    #[test]
    fn ttl_window_too_large_rejected() {
        let k = [3u8; 32];
        let far = Utc::now().timestamp() + MAX_TTL_SECONDS + 3600;
        let want = compute("/x", far, &k);
        assert!(verify("/x", far, &hex::encode(want), &k).is_err());
    }

    #[test]
    fn sign_clamps_ttl() {
        let k = [4u8; 32];
        let qs = sign("/x", 99_999, &k);
        let (exp, _) = parse_qs(&qs);
        let delta = exp - Utc::now().timestamp();
        assert!(delta <= MAX_TTL_SECONDS + 2);
    }
}
