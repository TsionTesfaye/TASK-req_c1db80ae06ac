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
    // Audit #7 Issue #3: the tiered-cache service worker must be served
    // from `/sw.js` with `Service-Worker-Allowed: /` so the worker can
    // control the whole SPA origin (app shell, images, API). The file
    // itself lives under `/app/dist/static/sw.js` on disk.
    let sw_dir = dir.clone();
    cfg.route(
        "/sw.js",
        web::get().to(move |req: HttpRequest| {
            let dir = sw_dir.clone();
            async move { serve_sw(req, dir).await }
        }),
    );
    cfg.service(files_service(&dir));
    let fallback_dir = dir.clone();
    cfg.default_service(web::route().to(move |req: HttpRequest| {
        let dir = fallback_dir.clone();
        async move { fallback(req, dir).await }
    }));
}

async fn serve_sw(req: HttpRequest, dir: PathBuf) -> HttpResponse {
    let path = dir.join("static").join("sw.js");
    match NamedFile::open_async(&path).await {
        Ok(f) => {
            let mut res = f.into_response(&req);
            res.headers_mut().insert(
                actix_web::http::header::HeaderName::from_static("service-worker-allowed"),
                actix_web::http::header::HeaderValue::from_static("/"),
            );
            res.headers_mut().insert(
                actix_web::http::header::CACHE_CONTROL,
                actix_web::http::header::HeaderValue::from_static("no-cache"),
            );
            res
        }
        Err(_) => HttpResponse::NotFound().body("sw.js missing"),
    }
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

#[cfg(test)]
mod tests {
    //! Focused edge-case tests for SPA static serving. These exercise
    //! the fallback discipline that keeps API 404s as JSON (never SPA
    //! shell) and the tiered-cache service-worker contract.

    use super::*;
    use actix_web::{body::to_bytes, http::StatusCode, test, App};
    use std::fs;

    fn tmpdir(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("terraops-spa-{name}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[actix_web::test]
    async fn fallback_on_api_path_returns_json_404_not_spa_shell() {
        let dir = tmpdir("api-404");
        fs::write(dir.join("index.html"), b"<html>SPA</html>").unwrap();

        let app = test::init_service(App::new().configure(|cfg| configure(cfg, &dir))).await;
        let req = test::TestRequest::get().uri("/api/v1/does-not-exist").to_request();
        let res = test::call_service(&app, req).await;

        assert_eq!(res.status(), StatusCode::NOT_FOUND);
        let ct = res
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        assert!(ct.contains("application/json"), "api 404 must be JSON, got: {ct}");
        let body = to_bytes(res.into_body()).await.unwrap();
        let text = std::str::from_utf8(&body).unwrap();
        assert!(text.contains("not_found"), "expected not_found envelope, got: {text}");
        assert!(!text.contains("<html"), "must NOT return SPA HTML for /api/*");
    }

    #[actix_web::test]
    async fn fallback_on_spa_path_serves_index_html_for_deep_link() {
        let dir = tmpdir("spa-deep");
        fs::write(dir.join("index.html"), b"<html>SPA-SHELL</html>").unwrap();

        let app = test::init_service(App::new().configure(|cfg| configure(cfg, &dir))).await;
        let req = test::TestRequest::get().uri("/admin/users/deep-link").to_request();
        let res = test::call_service(&app, req).await;

        assert_eq!(res.status(), StatusCode::OK);
        let body = to_bytes(res.into_body()).await.unwrap();
        let text = std::str::from_utf8(&body).unwrap();
        assert!(text.contains("SPA-SHELL"), "deep link must return SPA shell");
    }

    #[actix_web::test]
    async fn fallback_without_index_html_returns_404_not_panic() {
        let dir = tmpdir("no-index");
        // No index.html written.

        let app = test::init_service(App::new().configure(|cfg| configure(cfg, &dir))).await;
        let req = test::TestRequest::get().uri("/some/spa/path").to_request();
        let res = test::call_service(&app, req).await;

        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[actix_web::test]
    async fn sw_js_served_with_root_scope_header() {
        let dir = tmpdir("sw-ok");
        let static_dir = dir.join("static");
        fs::create_dir_all(&static_dir).unwrap();
        fs::write(static_dir.join("sw.js"), b"// worker").unwrap();

        let app = test::init_service(App::new().configure(|cfg| configure(cfg, &dir))).await;
        let req = test::TestRequest::get().uri("/sw.js").to_request();
        let res = test::call_service(&app, req).await;

        assert_eq!(res.status(), StatusCode::OK);
        let allowed = res
            .headers()
            .get("service-worker-allowed")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert_eq!(allowed, "/", "SW must be allowed to control whole origin");
        let cache = res
            .headers()
            .get("cache-control")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert_eq!(cache, "no-cache", "SW script must not be cached by browser");
    }

    #[actix_web::test]
    async fn sw_js_missing_returns_404_with_message() {
        let dir = tmpdir("sw-missing");
        fs::write(dir.join("index.html"), b"<html/>").unwrap();
        // No static/sw.js written.

        let app = test::init_service(App::new().configure(|cfg| configure(cfg, &dir))).await;
        let req = test::TestRequest::get().uri("/sw.js").to_request();
        let res = test::call_service(&app, req).await;

        assert_eq!(res.status(), StatusCode::NOT_FOUND);
        let body = to_bytes(res.into_body()).await.unwrap();
        let text = std::str::from_utf8(&body).unwrap();
        assert!(text.contains("sw.js missing"), "message body, got: {text}");
    }

    #[actix_web::test]
    async fn static_file_served_from_dir() {
        let dir = tmpdir("static-ok");
        fs::write(dir.join("app.css"), b"body{}").unwrap();
        fs::write(dir.join("index.html"), b"<html/>").unwrap();

        let app = test::init_service(App::new().configure(|cfg| configure(cfg, &dir))).await;
        let req = test::TestRequest::get().uri("/app.css").to_request();
        let res = test::call_service(&app, req).await;

        assert_eq!(res.status(), StatusCode::OK);
        let body = to_bytes(res.into_body()).await.unwrap();
        assert_eq!(&body[..], b"body{}");
    }
}
