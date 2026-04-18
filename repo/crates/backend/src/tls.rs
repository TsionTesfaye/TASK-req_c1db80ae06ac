//! Rustls ServerConfig builder. Scaffold-level one-way TLS (mTLS pinning
//! added in P1 under `backend::security`).

use std::{fs::File, io::BufReader, path::Path};

use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::ServerConfig;

pub fn load_server_config(
    cert_path: &Path,
    key_path: &Path,
) -> anyhow::Result<ServerConfig> {
    let certs = load_certs(cert_path)?;
    let key = load_key(key_path)?;
    let cfg = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)?;
    Ok(cfg)
}

fn load_certs(path: &Path) -> anyhow::Result<Vec<CertificateDer<'static>>> {
    let mut rd = BufReader::new(File::open(path)?);
    let certs: Vec<_> = rustls_pemfile::certs(&mut rd).collect::<Result<_, _>>()?;
    anyhow::ensure!(!certs.is_empty(), "no certs in {}", path.display());
    Ok(certs)
}

fn load_key(path: &Path) -> anyhow::Result<PrivateKeyDer<'static>> {
    let mut rd = BufReader::new(File::open(path)?);
    if let Some(key) = rustls_pemfile::private_key(&mut rd)? {
        Ok(key)
    } else {
        anyhow::bail!("no private key in {}", path.display());
    }
}
