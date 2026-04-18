//! Argon2id password hashing (design §Security — m=19456, t=2, p=1).
//!
//! `hash_password` is non-deterministic (salt is generated internally);
//! `verify_password` is constant-time.

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Algorithm, Argon2, Params, Version,
};

fn params() -> Params {
    // m_cost in KiB, t_cost, p_cost, output_len bytes (32 = default).
    Params::new(19_456, 2, 1, None).expect("argon2 params valid")
}

pub fn hash_password(plain: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let hasher = Argon2::new(Algorithm::Argon2id, Version::V0x13, params());
    let phc = hasher
        .hash_password(plain.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("argon2 hash: {e}"))?
        .to_string();
    Ok(phc)
}

pub fn verify_password(plain: &str, phc: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(phc) else {
        return false;
    };
    Argon2::default()
        .verify_password(plain.as_bytes(), &parsed)
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_ok() {
        let h = hash_password("TerraOps!2026").unwrap();
        assert!(verify_password("TerraOps!2026", &h));
    }

    #[test]
    fn wrong_password_rejected() {
        let h = hash_password("TerraOps!2026").unwrap();
        assert!(!verify_password("wrong", &h));
    }

    #[test]
    fn malformed_hash_rejected() {
        assert!(!verify_password("x", "not-a-phc"));
    }
}
