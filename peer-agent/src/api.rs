//! Local read-only HTTP API + static dashboard hosting.
//! Defence for an unauthenticated local service: loopback-only by default, Host-header
//! validation (blocks DNS rebinding), strict security headers, GET-only, no secrets in output.
use std::{net::SocketAddr, path::PathBuf};

use anyhow::{bail, Result};
use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderValue, Method, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use tower_http::services::{ServeDir, ServeFile};
use tracing::info;

use crate::status::Status;

pub type StatusRx = tokio::sync::watch::Receiver<Status>;

fn host_ok(host: &str) -> bool {
    let h = host.rsplit_once(':').filter(|(_, p)| p.chars().all(|c| c.is_ascii_digit())).map_or(host, |(h, _)| h);
    matches!(h, "localhost" | "127.0.0.1" | "[::1]")
}

async fn guard(req: Request<Body>, next: Next, allow_remote: bool) -> Response {
    if req.method() != Method::GET && req.method() != Method::HEAD {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    if !allow_remote {
        let ok = req.headers().get(header::HOST).and_then(|h| h.to_str().ok()).is_some_and(host_ok);
        if !ok { return (StatusCode::FORBIDDEN, "invalid host").into_response(); }
    }
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    for (k, v) in [
        ("x-content-type-options", "nosniff"),
        ("x-frame-options", "DENY"),
        ("referrer-policy", "no-referrer"),
        ("cross-origin-resource-policy", "same-origin"),
        ("content-security-policy", "default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; font-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'"),
        ("cache-control", "no-store"),
    ] {
        h.insert(k, HeaderValue::from_static(v));
    }
    res
}

async fn status(State(rx): State<StatusRx>) -> Json<Status> {
    Json(rx.borrow().clone())
}

pub fn check_bind(addr: SocketAddr, allow_remote: bool) -> Result<()> {
    if !addr.ip().is_loopback() && !allow_remote {
        bail!("--api-listen {addr} is not loopback; the API has no authentication (use --api-allow-remote to override)");
    }
    Ok(())
}

pub async fn serve(addr: SocketAddr, rx: StatusRx, dashboard: Option<PathBuf>, allow_remote: bool) -> Result<()> {
    check_bind(addr, allow_remote)?;
    let mut app = Router::new().route("/api/status", get(status)).route("/api/healthz", get(|| async { "ok" })).with_state(rx);
    if let Some(dir) = dashboard {
        let index = dir.join("index.html");
        app = app.fallback_service(ServeDir::new(dir).fallback(ServeFile::new(index)));
    }
    let app = app.layer(middleware::from_fn(move |req, next| guard(req, next, allow_remote)));
    let l = tokio::net::TcpListener::bind(addr).await?;
    info!(%addr, "API/dashboard listening");
    axum::serve(l, app).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::host_ok;
    #[test]
    fn bind_policy() {
        use super::check_bind;
        assert!(check_bind("127.0.0.1:1".parse().unwrap(), false).is_ok());
        assert!(check_bind("[::1]:1".parse().unwrap(), false).is_ok());
        assert!(check_bind("0.0.0.0:1".parse().unwrap(), false).is_err());
        assert!(check_bind("192.168.1.5:1".parse().unwrap(), false).is_err());
        assert!(check_bind("0.0.0.0:1".parse().unwrap(), true).is_ok());
    }

    #[test]
    fn host_validation() {
        assert!(host_ok("localhost:7777") && host_ok("127.0.0.1:7777") && host_ok("[::1]:7777") && host_ok("localhost"));
        assert!(!host_ok("evil.com") && !host_ok("evil.com:7777") && !host_ok("127.0.0.1.evil.com:80") && !host_ok(""));
    }
}
