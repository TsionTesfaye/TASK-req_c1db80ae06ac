//! HMAC-signed image URLs — self-authorizing so plain `<img src="…">`
//! browser requests work without an `Authorization: Bearer` header.
//!
//! Audit #13 Issue #1: the previous contract required both the signed
//! URL *and* a bearer session on the image fetch. That is
//! incompatible with how browsers load images from `<img src=…>` (they
//! cannot attach a `Authorization` header), so the shipped UI was
//! statically broken. The contract is now: the signed URL alone is the
//! capability, and it is minted only by authenticated handlers.
//!
//! Design §Images: every protected image read goes through a signed URL
//! with `exp` ≤ 600s from issuance. The signature covers
//! `path | user_id | exp`, so:
//!
//!   * a leaked URL only grants access to the one path it was signed for,
//!   * the same URL cannot be replayed past its `exp`,
//!   * the `u=<uuid>` parameter is in the URL but is bound by the HMAC
//!     — tampering with it produces a signature mismatch (403),
//!   * only handlers that already enforce bearer auth mint signed URLs,
//!     so the only way to obtain a valid URL is to have been
//!     authenticated at mint time (anti-hotlink + auth-gated provenance).
//!
//! Query shape is `?u=<uuid>&exp=<unix_seconds>&sig=<hex(hmac-sha256)>`.

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

/// Produce `?u=<uuid>&exp=<unix>&sig=<hex>` for the given path + user.
/// TTL is clamped to `MAX_TTL_SECONDS` (600). The `u=` parameter is
/// part of the URL because browsers cannot attach a bearer header for
/// `<img src=…>` loads; the HMAC over `path|user_id|exp` prevents any
/// tampering with `u` (Audit #13 Issue #1).
pub fn sign(path: &str, user_id: Uuid, ttl_seconds: i64, key: &[u8; 32]) -> String {
    let ttl = ttl_seconds.clamp(1, MAX_TTL_SECONDS);
    let exp = Utc::now().timestamp() + ttl;
    let sig = compute(path, user_id, exp, key);
    format!(
        "u={}&exp={exp}&sig={}",
        user_id.hyphenated(),
        hex::encode(sig)
    )
}

/// Parse the three query-string parameters (`u`, `exp`, `sig`) out of
/// the provided raw query. Returns `None` if any are missing or
/// malformed. Accepts params in any order.
pub fn parse_query(qs: &str) -> Option<(Uuid, i64, String)> {
    let mut u_val: Option<Uuid> = None;
    let mut exp_val: Option<i64> = None;
    let mut sig_val: Option<String> = None;
    for kv in qs.split('&') {
        if let Some(v) = kv.strip_prefix("u=") {
            u_val = Uuid::parse_str(v).ok();
        } else if let Some(v) = kv.strip_prefix("exp=") {
            exp_val = v.parse::<i64>().ok();
        } else if let Some(v) = kv.strip_prefix("sig=") {
            sig_val = Some(v.to_string());
        }
    }
    match (u_val, exp_val, sig_val) {
        (Some(u), Some(e), Some(s)) => Some((u, e, s)),
        _ => None,
    }
}

/// Verify the tuple `(path, user_id, exp, sig_hex)` against the
/// provided HMAC key. `user_id` comes straight from the URL's `u=`
/// parameter; any tampering yields a signature mismatch. Returns
/// `Ok(())` on success.
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

    #[test]
    fn sign_verify_roundtrip() {
        let k = [11u8; 32];
        let uid = Uuid::new_v4();
        let qs = sign("/images/products/abc.png", uid, 300, &k);
        let (u, exp, sig) = parse_query(&qs).unwrap();
        assert_eq!(u, uid);
        verify("/images/products/abc.png", u, exp, &sig, &k).unwrap();
    }

    #[test]
    fn query_contains_u_exp_sig() {
        let k = [11u8; 32];
        let uid = Uuid::new_v4();
        let qs = sign("/p", uid, 60, &k);
        assert!(qs.contains(&format!("u={}", uid.hyphenated())));
        assert!(qs.contains("exp="));
        assert!(qs.contains("sig="));
    }

    #[test]
    fn wrong_path_rejected() {
        let k = [12u8; 32];
        let uid = Uuid::new_v4();
        let qs = sign("/images/a.png", uid, 300, &k);
        let (u, exp, sig) = parse_query(&qs).unwrap();
        assert!(verify("/images/b.png", u, exp, &sig, &k).is_err());
    }

    #[test]
    fn wrong_key_rejected() {
        let k1 = [1u8; 32];
        let k2 = [2u8; 32];
        let uid = Uuid::new_v4();
        let qs = sign("/images/a.png", uid, 300, &k1);
        let (u, exp, sig) = parse_query(&qs).unwrap();
        assert!(verify("/images/a.png", u, exp, &sig, &k2).is_err());
    }

    // Audit #13 Issue #1: tampering with the `u=` parameter to swap in
    // another user's id must fail — the HMAC binds path|user_id|exp so
    // swapping the URL's user id yields a signature mismatch.
    #[test]
    fn tampered_u_rejected() {
        let k = [7u8; 32];
        let alice = Uuid::new_v4();
        let bob = Uuid::new_v4();
        let qs = sign("/images/a.png", alice, 300, &k);
        let (_u, exp, sig) = parse_query(&qs).unwrap();
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
        let (_u, exp, _sig) = parse_query(&qs).unwrap();
        let delta = exp - Utc::now().timestamp();
        assert!(delta <= MAX_TTL_SECONDS + 2);
    }
}
