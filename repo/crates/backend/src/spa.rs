//! Single-Page-Application static serving.
//!
//! - Serves built Yew assets from `static_dir` (typically `/app/dist`).
//! - Falls back to `index.html` for any non-API path so the Yew router can
//!   resolve client-side routes on deep links and hard refresh.

use std::path::{Path, PathBuf};

use actix_files::{Files, NamedFile};
use actix_web::{dev::HttpServiceFactory, web, HttpRequest, HttpResponse};

pub fn configure(cfg: &mut web::ServiceConfig, static_dir: &Path) {
    let dir = static_dir.to_path_buf();
    cfg.service(files_service(&dir));
    let fallback_dir = dir.clone();
    cfg.default_service(web::route().to(move |req: HttpRequest| {
        let dir = fallback_dir.clone();
        async move { fallback(req, dir).await }
    }));
}

fn files_service(dir: &Path) -> impl HttpServiceFactory {
    Files::new("/", dir)
        .index_file("index.html")
        .prefer_utf8(true)
        .use_etag(true)
        .use_last_modified(true)
}

async fn fallback(req: HttpRequest, dir: PathBuf) -> HttpResponse {
    // Do not serve SPA shell for API paths; return 404 JSON instead.
    if req.path().starts_with("/api/") {
        return HttpResponse::NotFound()
            .content_type("application/json")
            .body(r#"{"error":{"code":"not_found","message":"no such route"}}"#);
    }
    match NamedFile::open_async(dir.join("index.html")).await {
        Ok(f) => f.into_response(&req),
        Err(_) => HttpResponse::NotFound().body("index.html missing"),
    }
}
