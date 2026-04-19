//! Image file storage helpers.
//!
//! Images are stored at:
//!   `${TERRAOPS_RUNTIME_DIR}/images/{uuid}`
//!
//! The runtime directory defaults to `/var/lib/terraops` when the env var
//! is absent (container default). In tests the temp dir is used.

use std::path::PathBuf;
use uuid::Uuid;

/// Returns the absolute filesystem path for a given image ID.
pub fn image_path(id: Uuid) -> String {
    let runtime_dir =
        std::env::var("TERRAOPS_RUNTIME_DIR").unwrap_or_else(|_| "/var/lib/terraops".to_string());
    let p: PathBuf = [&runtime_dir, "images", &id.to_string()].iter().collect();
    p.to_string_lossy().into_owned()
}

/// Ensure the images directory exists. Called once at startup.
pub fn ensure_images_dir() -> std::io::Result<()> {
    let runtime_dir =
        std::env::var("TERRAOPS_RUNTIME_DIR").unwrap_or_else(|_| "/var/lib/terraops".to_string());
    let dir: PathBuf = [&runtime_dir, "images"].iter().collect();
    std::fs::create_dir_all(dir)
}
