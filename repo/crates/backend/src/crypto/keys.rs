//! Runtime key-material loader.
//!
//! On boot we read four files from the runtime volume:
//!   * `secrets/jwt.key`        — raw 32 bytes, HS256 signing key.
//!   * `secrets/email_enc.key`  — raw 32 bytes, AES-256-GCM key.
//!   * `secrets/email_hmac.key` — raw 32 bytes, HMAC-SHA256 key.
//!   * `secrets/image_hmac.key` — raw 32 bytes, HMAC-SHA256 key.
//!
//! If a file is missing, we generate random material and write it there so
//! the backend self-bootstraps outside of Docker too (useful for the
//! integration tests, where `scripts/dev_bootstrap.sh` does not run).

use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use rand::{rngs::OsRng, RngCore};

pub struct RuntimeKeys {
    pub jwt: [u8; 32],
    pub email_enc: [u8; 32],
    pub email_hmac: [u8; 32],
    pub image_hmac: [u8; 32],
}

impl RuntimeKeys {
    pub fn load_or_init(runtime_dir: &Path) -> anyhow::Result<Self> {
        let secrets = runtime_dir.join("secrets");
        fs::create_dir_all(&secrets)?;
        Ok(Self {
            jwt: load_or_init_32(&secrets.join("jwt.key"))?,
            email_enc: load_or_init_32(&secrets.join("email_enc.key"))?,
            email_hmac: load_or_init_32(&secrets.join("email_hmac.key"))?,
            image_hmac: load_or_init_32(&secrets.join("image_hmac.key"))?,
        })
    }

    /// Pure in-memory constructor used by integration tests so each test
    /// process has a deterministic, known key set without touching the FS.
    ///
    /// Always compiled in: the bytes are fixed constants and the harness
    /// in `tests/common/mod.rs` needs this symbol in an integration-test
    /// build where `#[cfg(test)]` is not set on the lib crate.
    pub fn for_testing() -> Self {
        Self {
            jwt: [1; 32],
            email_enc: [2; 32],
            email_hmac: [3; 32],
            image_hmac: [4; 32],
        }
    }
}

fn load_or_init_32(path: &PathBuf) -> anyhow::Result<[u8; 32]> {
    if path.exists() {
        let mut buf = Vec::new();
        File::open(path)?.read_to_end(&mut buf)?;
        // Accept either raw 32-byte files (the dev_bootstrap.sh format) OR
        // hex-encoded 64-char files (operator-provided). Anything else is
        // a misconfiguration and we refuse to boot.
        let raw = if buf.len() == 32 {
            buf
        } else {
            let text = std::str::from_utf8(&buf)
                .map_err(|_| anyhow::anyhow!("{} is not utf-8 hex", path.display()))?
                .trim();
            hex::decode(text).map_err(|e| {
                anyhow::anyhow!(
                    "{} must be raw 32 bytes or 64 hex chars ({e})",
                    path.display()
                )
            })?
        };
        if raw.len() != 32 {
            anyhow::bail!(
                "{} resolved to {} bytes; expected 32",
                path.display(),
                raw.len()
            );
        }
        let mut out = [0u8; 32];
        out.copy_from_slice(&raw);
        Ok(out)
    } else {
        let mut out = [0u8; 32];
        OsRng.fill_bytes(&mut out);
        let mut f = File::create(path)?;
        f.write_all(&out)?;
        // Owner-read-only; the runtime volume is already only mounted
        // inside the container, but defense-in-depth costs nothing.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(path)?.permissions();
            perms.set_mode(0o600);
            fs::set_permissions(path, perms)?;
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn unique_tmp_dir(label: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "terraops-keys-test-{label}-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&p).expect("mkdir tmp");
        p
    }

    #[test]
    fn for_testing_returns_distinct_deterministic_bytes() {
        let k = RuntimeKeys::for_testing();
        assert_eq!(k.jwt, [1u8; 32]);
        assert_eq!(k.email_enc, [2u8; 32]);
        assert_eq!(k.email_hmac, [3u8; 32]);
        assert_eq!(k.image_hmac, [4u8; 32]);
    }

    #[test]
    fn load_or_init_creates_keys_when_missing_and_persists_them() {
        let dir = unique_tmp_dir("init");
        let first = RuntimeKeys::load_or_init(&dir).expect("first init");
        // Files now exist with 0o600 perms.
        for name in ["jwt.key", "email_enc.key", "email_hmac.key", "image_hmac.key"] {
            let p = dir.join("secrets").join(name);
            assert!(p.exists(), "{name} should exist");
            let meta = fs::metadata(&p).unwrap();
            assert_eq!(meta.len(), 32);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                assert_eq!(meta.permissions().mode() & 0o777, 0o600);
            }
        }
        // Re-loading reads the same bytes back.
        let again = RuntimeKeys::load_or_init(&dir).expect("reload");
        assert_eq!(first.jwt, again.jwt);
        assert_eq!(first.email_enc, again.email_enc);
        assert_eq!(first.email_hmac, again.email_hmac);
        assert_eq!(first.image_hmac, again.image_hmac);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_or_init_accepts_64_char_hex_format() {
        let dir = unique_tmp_dir("hex");
        let secrets = dir.join("secrets");
        fs::create_dir_all(&secrets).unwrap();
        let hex_bytes = "a".repeat(64); // 32 bytes worth of 0xAA
        for name in ["jwt.key", "email_enc.key", "email_hmac.key", "image_hmac.key"] {
            let mut f = File::create(secrets.join(name)).unwrap();
            f.write_all(hex_bytes.as_bytes()).unwrap();
        }
        let k = RuntimeKeys::load_or_init(&dir).expect("hex load");
        assert_eq!(k.jwt, [0xAAu8; 32]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_or_init_rejects_invalid_length_file() {
        let dir = unique_tmp_dir("bad");
        let secrets = dir.join("secrets");
        fs::create_dir_all(&secrets).unwrap();
        // Wrong length AND not valid hex either.
        let mut f = File::create(secrets.join("jwt.key")).unwrap();
        f.write_all(b"too-short-and-not-hex").unwrap();
        let res = RuntimeKeys::load_or_init(&dir);
        assert!(res.is_err(), "expected load error, got Ok");
        let _ = fs::remove_dir_all(&dir);
    }
}
