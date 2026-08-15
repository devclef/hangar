//! Static frontend serving. The SPA uses hash-based routing, so the only
//! dynamic requirement is: unknown `/api/*` paths get a JSON 404, everything
//! else is served from the build output (with `index.html` as the default).

use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use std::path::Path;

pub const MISSING_FRONTEND_HTML: &str = r#"<!doctype html>
<html lang="en">
<head><meta charset="utf-8"><title>Hangar</title>
<style>body{font-family:system-ui,sans-serif;background:#f5f5f4;color:#292524;display:flex;align-items:center;justify-content:center;height:100vh;margin:0}
.box{max-width:34rem;text-align:center;padding:2rem;border:1px solid #e7e5e4;border-radius:8px;background:#fff}</style>
</head><body><div class="box">
<h1>Hangar</h1>
<p>The backend is running, but no frontend build was found at <code>STATIC_DIR</code>.</p>
<p>Build it with <code>cd frontend &amp;&amp; npm install &amp;&amp; npm run build</code> and restart,
or set <code>STATIC_DIR</code> to the directory containing <code>index.html</code>
(used in Docker: <code>/app/static</code>).</p>
</div></body></html>"#;

/// Fallback for any request that did not match an API route.
pub async fn fallback(
    State(state): State<crate::routes::AppState>,
    req: Request<Body>,
) -> Response {
    let path = req.uri().path();
    if path.starts_with("/api/") {
        let body = Json(serde_json::json!({
            "error": "not_found",
            "message": format!("unknown API route {path}")
        }));
        return (StatusCode::NOT_FOUND, body).into_response();
    }
    serve_static(&state, path)
}

fn serve_static(state: &crate::routes::AppState, path: &str) -> Response {
    let dir = match &state.static_dir {
        Some(dir) if dir.is_dir() => dir.clone(),
        _ => return (StatusCode::NOT_FOUND, MISSING_FRONTEND_HTML).into_response(),
    };

    let rel = path.trim_start_matches('/');
    if rel.is_empty() {
        return file_response(&dir.join("index.html"), "text/html; charset=utf-8", false);
    }
    if rel == "index.html" {
        return file_response(&dir.join("index.html"), "text/html; charset=utf-8", false);
    }
    // Reject traversal; vite asset names are hash-suffixed and need no decoding.
    if rel.contains("..") {
        return StatusCode::FORBIDDEN.into_response();
    }
    let file = dir.join(rel);
    match std::fs::metadata(&file) {
        Ok(md) if md.is_file() => {
            // Vite emits content-hashed files under assets/ — cache them hard.
            let immutable = rel.starts_with("assets/");
            file_response(&file, mime_for(rel), immutable)
        }
        _ => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

fn file_response(path: &Path, content_type: &str, immutable: bool) -> Response {
    match std::fs::read(path) {
        Ok(bytes) => {
            let cache = if immutable {
                "public, max-age=31536000, immutable"
            } else {
                "no-cache"
            };
            (
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, content_type)],
                [(axum::http::header::CACHE_CONTROL, cache)],
                Body::from(bytes),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = ?e, path = %path.display(), "failed to read static file");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response()
        }
    }
}

fn mime_for(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}
