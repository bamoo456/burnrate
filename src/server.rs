//! LAN web access: an optional, read-only HTTP server that serves the built
//! frontend plus the dashboard/insights JSON, so a phone on the same network
//! (or Tailscale) can watch quota without a native client.
//!
//! Read-only on purpose. Sign-in, account edits, and settings stay in the
//! desktop app; the served page is the tray view, whose write paths already
//! no-op outside Tauri.
//!
//! Assets come from Tauri's embedded asset resolver, which only holds them when
//! the `custom-protocol` feature is on — `npm run dev` disables it, so remote
//! access serves an explanatory page there instead of a blank one.

use std::sync::Mutex;

use axum::{
    Router,
    body::Body,
    extract::{Query, Request, State},
    http::{HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
};
use serde::Deserialize;
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Manager};

use crate::app_state::AppState;
use crate::models::AppSettings;

/// Fixed port. Not configurable: one less setting, and the share URL stays
/// stable across restarts so a home-screen bookmark keeps working.
pub(crate) const REMOTE_PORT: u16 = 17877;
const TOKEN_COOKIE: &str = "burnrate_token";
/// Marker the frontend keys "remote mode" off (`api.ts`): fetch the JSON API
/// instead of Tauri IPC, and hide desktop-only controls.
const REMOTE_MARKER: &str = "<head><script>window.__BURNRATE_REMOTE__=1</script>";

#[derive(Clone)]
struct ServerState {
    app: AppHandle,
    token: String,
}

/// Owns the running server task so a settings change can start/stop it without
/// an app restart.
#[derive(Default)]
pub(crate) struct RemoteServer {
    task: Mutex<Option<JoinHandle<()>>>,
}

impl RemoteServer {
    /// Bring the server in line with `settings`, starting or stopping it as
    /// needed. Restarts on a token change so a rotated token takes effect.
    pub(crate) fn apply(&self, app: &AppHandle, settings: &AppSettings) {
        let mut task = self.task.lock().expect("remote server lock");
        if let Some(handle) = task.take() {
            handle.abort();
        }
        if !settings.remote_access || settings.remote_token.is_empty() {
            return;
        }
        let state = ServerState {
            app: app.clone(),
            token: settings.remote_token.clone(),
        };
        *task = Some(tauri::async_runtime::spawn(async move {
            if let Err(error) = serve(state).await {
                eprintln!("Burnrate remote access stopped: {error}");
            }
        }));
    }
}

async fn serve(state: ServerState) -> anyhow::Result<()> {
    let router = Router::new()
        .route("/api/dashboard", get(dashboard))
        .route("/api/local-usage", get(local_usage))
        .fallback(asset)
        .layer(middleware::from_fn_with_state(
            state.token.clone(),
            require_token,
        ))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", REMOTE_PORT)).await?;
    axum::serve(listener, router).await?;
    Ok(())
}

#[derive(Deserialize)]
struct TokenQuery {
    t: Option<String>,
}

/// Gate every request on the shared token, taken from `?t=` (the share link) or
/// the cookie a previous `?t=` set (so a home-screen bookmark keeps working).
async fn require_token(
    State(token): State<String>,
    Query(query): Query<TokenQuery>,
    request: Request,
    next: Next,
) -> Response {
    let from_query = query.t.as_deref().is_some_and(|t| t == token);
    let authorized = from_query
        || cookie_token(request.headers().get(header::COOKIE)).is_some_and(|value| value == token);
    if !authorized {
        return (
            StatusCode::UNAUTHORIZED,
            "Burnrate: invalid or missing token",
        )
            .into_response();
    }

    let mut response = next.run(request).await;
    if from_query {
        // No `Secure`: this is plain HTTP on a LAN. `SameSite=Lax` still keeps
        // it off cross-site requests.
        if let Ok(cookie) = HeaderValue::from_str(&format!(
            "{TOKEN_COOKIE}={token}; Path=/; Max-Age=31536000; SameSite=Lax"
        )) {
            response.headers_mut().append(header::SET_COOKIE, cookie);
        }
    }
    response
}

pub(crate) fn cookie_token(header: Option<&HeaderValue>) -> Option<String> {
    let raw = header?.to_str().ok()?;
    raw.split(';')
        .filter_map(|pair| pair.split_once('='))
        .find(|(name, _)| name.trim() == TOKEN_COOKIE)
        .map(|(_, value)| value.trim().to_string())
}

async fn dashboard(State(state): State<ServerState>) -> Response {
    match state.app.state::<AppState>().dashboard().await {
        Ok(dashboard) => json_response(serde_json::to_vec(&dashboard)),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

async fn local_usage(State(state): State<ServerState>) -> Response {
    let report = state.app.state::<AppState>().local_usage().await;
    json_response(serde_json::to_vec(&report))
}

fn json_response(body: serde_json::Result<Vec<u8>>) -> Response {
    match body {
        Ok(bytes) => (
            [
                (header::CONTENT_TYPE, "application/json"),
                (header::CACHE_CONTROL, "no-store"),
            ],
            bytes,
        )
            .into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

/// Serve the embedded frontend. Everything that isn't a known asset falls back
/// to `index.html` so the SPA's query-string routing works.
async fn asset(State(state): State<ServerState>, request: Request) -> Response {
    let path = request.uri().path().trim_start_matches('/').to_string();
    let resolver = state.app.asset_resolver();
    let asset = resolver
        .get(path.clone())
        .or_else(|| resolver.get("index.html".to_string()));
    let Some(asset) = asset else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "Burnrate remote access needs a bundled build (the frontend assets \
             are not embedded in `npm run dev`). Run a release build, or point \
             this browser at the Vite dev server instead.",
        )
            .into_response();
    };

    let mime = asset.mime_type;
    let bytes = if mime.starts_with("text/html") {
        inject_remote_marker(&String::from_utf8_lossy(&asset.bytes)).into_bytes()
    } else {
        asset.bytes
    };
    Response::builder()
        .header(header::CONTENT_TYPE, mime)
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from(bytes))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

/// Flag the page as remotely served, and drop any injected CSP `<meta>`: the
/// desktop policy has no `connect-src 'self'` for plain HTTP, which would block
/// the page's own `/api` calls.
pub(crate) fn inject_remote_marker(html: &str) -> String {
    let stripped = strip_csp_meta(html);
    stripped.replacen("<head>", REMOTE_MARKER, 1)
}

fn strip_csp_meta(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find("<meta http-equiv=\"Content-Security-Policy\"") {
        let Some(end) = rest[start..].find('>') else {
            break;
        };
        out.push_str(&rest[..start]);
        rest = &rest[start + end + 1..];
    }
    out.push_str(rest);
    out
}

/// Share link for the phone. `hostname` resolves over Bonjour on a LAN and over
/// MagicDNS on Tailscale, so it beats a hardcoded IP that changes with DHCP.
pub(crate) fn share_url(token: &str) -> String {
    let host = std::process::Command::new("hostname")
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "localhost".to_string());
    format!("http://{host}:{REMOTE_PORT}/?view=tray&t={token}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_token_cookie_among_others() {
        let header = HeaderValue::from_static("theme=dark; burnrate_token=abc123; other=1");
        assert_eq!(cookie_token(Some(&header)).as_deref(), Some("abc123"));
    }

    #[test]
    fn ignores_a_missing_or_unrelated_cookie() {
        let header = HeaderValue::from_static("theme=dark");
        assert_eq!(cookie_token(Some(&header)), None);
        assert_eq!(cookie_token(None), None);
    }

    #[test]
    fn injects_the_marker_and_drops_the_desktop_csp() {
        let html = concat!(
            "<html><head>",
            "<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'self'\">",
            "<title>Burnrate</title></head></html>"
        );
        let injected = inject_remote_marker(html);
        assert!(injected.contains("window.__BURNRATE_REMOTE__=1"));
        assert!(!injected.contains("Content-Security-Policy"));
        assert!(injected.contains("<title>Burnrate</title>"));
    }

    /// End-to-end through the real middleware on a throwaway port: no token is
    /// rejected, `?t=` is accepted and hands back the cookie, and that cookie
    /// alone authorizes the next request.
    #[tokio::test]
    async fn token_gate_rejects_then_accepts_and_sets_a_cookie() {
        let router =
            Router::new()
                .fallback(|| async { "ok" })
                .layer(middleware::from_fn_with_state(
                    "secret".to_string(),
                    require_token,
                ));
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });

        let client = reqwest::Client::new();
        let denied = client.get(&base).send().await.unwrap();
        assert_eq!(denied.status(), reqwest::StatusCode::UNAUTHORIZED);

        let wrong = client.get(format!("{base}/?t=nope")).send().await.unwrap();
        assert_eq!(wrong.status(), reqwest::StatusCode::UNAUTHORIZED);

        let allowed = client
            .get(format!("{base}/?t=secret"))
            .send()
            .await
            .unwrap();
        assert!(allowed.status().is_success());
        let cookie = allowed
            .headers()
            .get(header::SET_COOKIE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string();
        assert!(cookie.starts_with("burnrate_token=secret"));

        let with_cookie = client
            .get(&base)
            .header(header::COOKIE, "burnrate_token=secret")
            .send()
            .await
            .unwrap();
        assert!(with_cookie.status().is_success());
    }

    #[test]
    fn share_url_carries_the_tray_view_and_token() {
        let url = share_url("tok");
        assert!(url.starts_with("http://"));
        assert!(url.ends_with(&format!(":{REMOTE_PORT}/?view=tray&t=tok")));
    }
}
