//! HMAC-signed image URLs.
//!
//! Design §Images: every protected image read goes through a signed URL
//! with `exp` ≤ 600s from issuance. The signature covers
//! `path | user_id | exp`, so:
//!
//!   * a leaked URL only grants access to the one path it was signed for,
//!   * the same URL cannot be replayed past its `exp`, and
//!   * the same URL cannot be reused by a *different* authenticated user
//!     — audit #11 issue #1. The verifier recomputes the HMAC using the
//!     authenticated caller's user id; a different caller yields a
//!     different HMAC and is rejected with `403`.
//!
//! Query shape remains `?exp=<unix_seconds>&sig=<hex(hmac-sha256)>`; the
//! user id is never placed in the URL (the authenticated session already
//! identifies the caller server-side), so nothing about the user is
//! leaked in logs or referers.

use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

pub const MAX_TTL_SECONDS: i64 = 600;

fn compute(path: &str, user_id: Uuid, exp: i64, key: &[u8; 32]) -> Vec<u8> {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(key).expect("hmac sha256 any key");
    mac.update(path.as_bytes());
    mac.update(b"|");
    // Bind the signature to the authenticated user so another signed-in
    // user cannot reuse a leaked URL within its lifetime. We hash the
    // canonical hyphenated string form of the UUID (36 ASCII bytes) so
    // the wire format is unambiguous and stable across targets.
    mac.update(user_id.hyphenated().to_string().as_bytes());
    mac.update(b"|");
    mac.update(exp.to_string().as_bytes());
    mac.finalize().into_bytes().to_vec()
}

/// Produce `?exp=...&sig=...` for the given path + user. Clamped to 600s.
pub fn sign(path: &str, user_id: Uuid, ttl_seconds: i64, key: &[u8; 32]) -> String {
    let ttl = ttl_seconds.clamp(1, MAX_TTL_SECONDS);
    let exp = Utc::now().timestamp() + ttl;
    let sig = compute(path, user_id, exp, key);
    format!("exp={exp}&sig={}", hex::encode(sig))
}

/// Verify a query string for `path` against the authenticated caller's
/// user id. Returns `Ok(())` on success; the HMAC mismatches if any of
/// `path`, `user_id`, or `exp` differ from what was signed.
pub fn verify(
    path: &str,
    user_id: Uuid,
    exp: i64,
    sig_hex: &str,
    key: &[u8; 32],
) -> anyhow::Result<()> {
    let now = Utc::now().timestamp();
    if exp < now {
        anyhow::bail!("signed url expired");
    }
    if exp - now > MAX_TTL_SECONDS + 5 {
        anyhow::bail!("signed url exp window too large");
    }
    let want = compute(path, user_id, exp, key);
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
        let uid = Uuid::new_v4();
        let qs = sign("/images/products/abc.png", uid, 300, &k);
        let (exp, sig) = parse_qs(&qs);
        verify("/images/products/abc.png", uid, exp, &sig, &k).unwrap();
    }

    #[test]
    fn wrong_path_rejected() {
        let k = [12u8; 32];
        let uid = Uuid::new_v4();
        let qs = sign("/images/a.png", uid, 300, &k);
        let (exp, sig) = parse_qs(&qs);
        assert!(verify("/images/b.png", uid, exp, &sig, &k).is_err());
    }

    #[test]
    fn wrong_key_rejected() {
        let k1 = [1u8; 32];
        let k2 = [2u8; 32];
        let uid = Uuid::new_v4();
        let qs = sign("/images/a.png", uid, 300, &k1);
        let (exp, sig) = parse_qs(&qs);
        assert!(verify("/images/a.png", uid, exp, &sig, &k2).is_err());
    }

    // Audit #11 issue #1: verifying a URL signed for user A under user
    // B's identity must fail, even with identical path + exp + key.
    #[test]
    fn wrong_user_rejected() {
        let k = [7u8; 32];
        let alice = Uuid::new_v4();
        let bob = Uuid::new_v4();
        let qs = sign("/images/a.png", alice, 300, &k);
        let (exp, sig) = parse_qs(&qs);
        assert!(verify("/images/a.png", alice, exp, &sig, &k).is_ok());
        assert!(verify("/images/a.png", bob, exp, &sig, &k).is_err());
    }

    #[test]
    fn expired_rejected() {
        let k = [3u8; 32];
        let uid = Uuid::new_v4();
        let want = compute("/x", uid, 1, &k);
        assert!(verify("/x", uid, 1, &hex::encode(want), &k).is_err());
    }

    #[test]
    fn ttl_window_too_large_rejected() {
        let k = [3u8; 32];
        let uid = Uuid::new_v4();
        let far = Utc::now().timestamp() + MAX_TTL_SECONDS + 3600;
        let want = compute("/x", uid, far, &k);
        assert!(verify("/x", uid, far, &hex::encode(want), &k).is_err());
    }

    #[test]
    fn sign_clamps_ttl() {
        let k = [4u8; 32];
        let uid = Uuid::new_v4();
        let qs = sign("/x", uid, 99_999, &k);
        let (exp, _) = parse_qs(&qs);
        let delta = exp - Utc::now().timestamp();
        assert!(delta <= MAX_TTL_SECONDS + 2);
    }
}
