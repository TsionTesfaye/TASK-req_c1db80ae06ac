//! HS256 JWT access tokens.
//!
//! 15-minute lifetime. The refresh token is opaque and lives in the
//! `sessions` table (see `auth::sessions`). We only mint short access
//! tokens here.

use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const ACCESS_TOKEN_TTL_MINUTES: i64 = 15;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessClaims {
    /// Subject — user id.
    pub sub: Uuid,
    /// Session id — lets the backend invalidate a specific session by
    /// revoking its row.
    pub sid: Uuid,
    /// Issued-at (unix seconds).
    pub iat: i64,
    /// Expiry (unix seconds).
    pub exp: i64,
}

pub fn mint(sub: Uuid, sid: Uuid, key: &[u8; 32]) -> anyhow::Result<(String, DateTime<Utc>)> {
    let now = Utc::now();
    let exp = now + Duration::minutes(ACCESS_TOKEN_TTL_MINUTES);
    let claims = AccessClaims {
        sub,
        sid,
        iat: now.timestamp(),
        exp: exp.timestamp(),
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(key),
    )
    .map_err(|e| anyhow::anyhow!("jwt encode: {e}"))?;
    Ok((token, exp))
}

pub fn parse(token: &str, key: &[u8; 32]) -> anyhow::Result<AccessClaims> {
    let mut v = Validation::default();
    v.leeway = 5;
    let data = decode::<AccessClaims>(token, &DecodingKey::from_secret(key), &v)
        .map_err(|e| anyhow::anyhow!("jwt decode: {e}"))?;
    Ok(data.claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let k = [9u8; 32];
        let sub = Uuid::new_v4();
        let sid = Uuid::new_v4();
        let (t, exp) = mint(sub, sid, &k).unwrap();
        let claims = parse(&t, &k).unwrap();
        assert_eq!(claims.sub, sub);
        assert_eq!(claims.sid, sid);
        assert!((exp - Utc::now()).num_seconds() > 0);
    }

    #[test]
    fn wrong_key_rejected() {
        let k1 = [1u8; 32];
        let k2 = [2u8; 32];
        let (t, _) = mint(Uuid::new_v4(), Uuid::new_v4(), &k1).unwrap();
        assert!(parse(&t, &k2).is_err());
    }

    #[test]
    fn expired_rejected() {
        let k = [1u8; 32];
        let claims = AccessClaims {
            sub: Uuid::new_v4(),
            sid: Uuid::new_v4(),
            iat: 0,
            exp: 1,
        };
        let t = encode(&Header::default(), &claims, &EncodingKey::from_secret(&k)).unwrap();
        assert!(parse(&t, &k).is_err());
    }
}
