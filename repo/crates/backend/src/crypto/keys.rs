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
    #[cfg(any(test, feature = "test-utils"))]
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
