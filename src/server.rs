//! LAN web access: an optional, read-only HTTP server that serves the built
//! frontend plus the dashboard/insights JSON, so a phone on the same network
//! (or Tailscale) can watch quota without a native client.
//!
//! Read-only on purpose. Sign-in, account edits, and settings stay in the
//! desktop app; the served page is the remote dashboard, whose write paths
//! already no-op outside Tauri.
//!
//! Assets come from Tauri's embedded asset resolver, which only holds them when
//! the `custom-protocol` feature is on — `npm run dev` disables it, so remote
//! access serves an explanatory page there instead of a blank one.

use std::sync::Mutex;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;

use axum::{
    Router,
    body::Body,
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
};
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Manager};

use crate::app_state::AppState;
use crate::models::AppSettings;

/// Fixed port. Not configurable: one less setting, and the share URL stays
/// stable across restarts so a home-screen bookmark keeps working.
pub(crate) const REMOTE_PORT: u16 = 17877;
/// Marker the frontend keys "remote mode" off (`api.ts`): fetch the JSON API
/// instead of Tauri IPC, and hide desktop-only controls.
const REMOTE_MARKER: &str = "<head><script>window.__BURNRATE_REMOTE__=1</script>";

#[derive(Clone)]
struct ServerState {
    app: AppHandle,
    token: String,
}

/// Owns the running server task so a settings change can start/stop it without
/// an app restart. The token doubles as the identity of what is running.
#[derive(Default)]
pub(crate) struct RemoteServer {
    running: Mutex<Option<(String, JoinHandle<()>)>>,
}

impl RemoteServer {
    /// Bring the server in line with `settings`, starting, stopping, or leaving
    /// it alone. Every settings save lands here — the tray-scale slider alone
    /// fires one per step — and a needless restart would race the aborted task
    /// for the port and leave remote access silently dead, so an unchanged
    /// token is a no-op.
    pub(crate) fn apply(&self, app: &AppHandle, settings: &AppSettings) {
        let want = (settings.remote_access && !settings.remote_token.is_empty())
            .then(|| settings.remote_token.clone());
        let mut running = self.running.lock().expect("remote server lock");
        if !needs_restart(
            running.as_ref().map(|(token, _)| token.as_str()),
            want.as_deref(),
        ) {
            return;
        }
        if let Some((_, handle)) = running.take() {
            handle.abort();
        }
        let Some(token) = want else {
            return;
        };
        let state = ServerState {
            app: app.clone(),
            token: token.clone(),
        };
        let handle = tauri::async_runtime::spawn(async move {
            if let Err(error) = serve(state).await {
                eprintln!("Burnrate remote access stopped: {error}");
            }
        });
        *running = Some((token, handle));
    }
}

/// Whether the running server (identified by its token, `None` when stopped)
/// has to be torn down to reach the wanted state.
fn needs_restart(running: Option<&str>, want: Option<&str>) -> bool {
    running != want
}

async fn serve(state: ServerState) -> anyhow::Result<()> {
    let router = Router::new()
        .route("/api/dashboard", get(dashboard))
        .route("/api/local-usage", get(local_usage))
        .fallback(asset)
        .layer(middleware::from_fn_with_state(
            state.token.clone(),
            require_password,
        ))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", REMOTE_PORT)).await?;
    axum::serve(listener, router).await?;
    Ok(())
}

/// Gate every request on the token, supplied as the **password** of HTTP Basic
/// auth (the username is ignored). Basic keeps the share link free of secrets —
/// a URL leaks into history, screenshots and the address bar — and the browser
/// replays the credential on every request, so a home-screen bookmark keeps
/// working without a cookie of our own.
///
/// ponytail: plain `==` compare, no constant-time, and no attempt limit. A
/// minted token is 32 random hex chars on a LAN, with no realistic timing
/// oracle to close; the strength of a user-chosen one is on the user.
///
/// ponytail: iOS "Add to Home Screen" runs in a standalone context that does
/// not share Safari's Basic-auth cache, so it can re-prompt per launch. Live
/// with it; a cookie fallback would put the secret back on the wire in a URL.
async fn require_password(State(token): State<String>, request: Request, next: Next) -> Response {
    let supplied = basic_auth_password(
        request
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok()),
    );
    if supplied.as_deref() != Some(token.as_str()) {
        // Without `WWW-Authenticate` the browser never shows a prompt.
        return (
            StatusCode::UNAUTHORIZED,
            [(
                header::WWW_AUTHENTICATE,
                r#"Basic realm="Burnrate", charset="UTF-8""#,
            )],
            "Burnrate: paste the access token as the password",
        )
            .into_response();
    }
    next.run(request).await
}

/// The password half of an `Authorization: Basic <base64(user:password)>`
/// header. The username is whatever the user typed into the browser prompt.
pub(crate) fn basic_auth_password(header: Option<&str>) -> Option<String> {
    let encoded = header?.strip_prefix("Basic ")?.trim();
    let decoded = BASE64.decode(encoded).ok()?;
    let pair = String::from_utf8(decoded).ok()?;
    // A password may contain ':'; only the first separator counts.
    Some(pair.split_once(':')?.1.to_string())
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
             are not embedded in `npm run dev`). Run a bundled build \
             (`./scripts/build-app`) and try again.",
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

/// Share link for the phone. A bare `hostname` is a trap on a managed Mac: a
/// corporate DNS search domain resolves it to whatever address it last
/// registered (a VPN or hotspot lease), not the machine on this LAN. The
/// Bonjour `.local` name is answered by the host itself over the interface the
/// query arrived on, so it always names the right address without guessing
/// which of the utun/Wi-Fi interfaces is the LAN one.
///
/// ponytail: mDNS is not resolvable from most Android browsers; those users
/// need the LAN IP, which the UI does not offer yet.
pub(crate) fn share_url() -> String {
    share_url_for_host(&share_host())
}

fn share_url_for_host(host: &str) -> String {
    format!("http://{host}:{REMOTE_PORT}/")
}

/// The Bonjour name of this machine, e.g. `george-c-m3p.local`.
fn share_host() -> String {
    // `scutil --get LocalHostName` is the actual Bonjour name, which differs
    // from `hostname` whenever the Mac was renamed in Sharing preferences.
    #[cfg(target_os = "macos")]
    let command = {
        let mut c = std::process::Command::new("scutil");
        c.args(["--get", "LocalHostName"]);
        c
    };
    #[cfg(not(target_os = "macos"))]
    let command = std::process::Command::new("hostname");

    let mut command = command;
    let host = command
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "localhost".to_string());
    mdns_host(&host)
}

/// Qualify a bare host label with `.local`; an already-qualified name (or
/// `localhost`) is left alone.
fn mdns_host(host: &str) -> String {
    if host == "localhost" || host.contains('.') {
        host.to_string()
    } else {
        format!("{host}.local")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restarts_only_when_the_wanted_token_changes() {
        assert!(!needs_restart(None, None), "still off");
        assert!(
            !needs_restart(Some("a"), Some("a")),
            "unrelated settings save"
        );
        assert!(needs_restart(None, Some("a")), "turned on");
        assert!(needs_restart(Some("a"), None), "turned off");
        assert!(needs_restart(Some("a"), Some("b")), "token rotated");
    }

    #[test]
    fn reads_the_password_half_of_a_basic_header() {
        let header = format!("Basic {}", BASE64.encode("anyone:abc123"));
        assert_eq!(
            basic_auth_password(Some(&header)).as_deref(),
            Some("abc123")
        );
        // A ':' inside the password belongs to the password.
        let odd = format!("Basic {}", BASE64.encode("u:a:b"));
        assert_eq!(basic_auth_password(Some(&odd)).as_deref(), Some("a:b"));
    }

    #[test]
    fn ignores_a_missing_or_malformed_authorization_header() {
        assert_eq!(basic_auth_password(None), None);
        assert_eq!(basic_auth_password(Some("Bearer abc")), None);
        assert_eq!(basic_auth_password(Some("Basic !!not-base64")), None);
        // No ':' separator at all.
        let bare = format!("Basic {}", BASE64.encode("nocolon"));
        assert_eq!(basic_auth_password(Some(&bare)), None);
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

    /// End-to-end through the real middleware on a throwaway port: no
    /// credential is rejected with a prompt-triggering challenge, a wrong
    /// password is rejected, and the token as the password gets in whatever
    /// username the browser sent.
    #[tokio::test]
    async fn password_gate_challenges_then_accepts_the_token() {
        let router =
            Router::new()
                .fallback(|| async { "ok" })
                .layer(middleware::from_fn_with_state(
                    "secret".to_string(),
                    require_password,
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
        assert!(
            denied
                .headers()
                .get(header::WWW_AUTHENTICATE)
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default()
                .starts_with("Basic realm=")
        );

        let wrong = client
            .get(&base)
            .basic_auth("burnrate", Some("nope"))
            .send()
            .await
            .unwrap();
        assert_eq!(wrong.status(), reqwest::StatusCode::UNAUTHORIZED);

        let allowed = client
            .get(&base)
            .basic_auth("", Some("secret"))
            .send()
            .await
            .unwrap();
        assert!(allowed.status().is_success());
    }

    #[test]
    fn share_url_keeps_the_token_out_of_the_link() {
        let url = share_url();
        assert!(url.starts_with("http://"));
        assert!(url.ends_with(&format!(":{REMOTE_PORT}/")));
    }

    #[test]
    fn share_url_uses_the_bonjour_name_so_corporate_dns_cannot_hijack_it() {
        assert_eq!(mdns_host("george-c-m3p"), "george-c-m3p.local");
        assert_eq!(
            mdns_host("box.office.example.com"),
            "box.office.example.com"
        );
        assert_eq!(mdns_host("localhost"), "localhost");
        assert_eq!(
            share_url_for_host("mac.local"),
            format!("http://mac.local:{REMOTE_PORT}/")
        );
    }
}
