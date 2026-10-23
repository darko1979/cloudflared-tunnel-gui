use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use regex::Regex;
use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;
use tokio::net::TcpStream;

use crate::process::{self, TunnelState};
use crate::{cloudflared, config};

fn failure(error: impl Into<String>) -> Value {
    json!({ "success": false, "error": error.into() })
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

async fn list_tunnels() -> Result<Vec<Value>, String> {
    let out = cloudflared::run(&["tunnel", "list", "--output", "json"]).await;
    if out.error.is_some() {
        return Err(out.error_message());
    }
    let tunnels: Value = serde_json::from_str(&out.stdout).map_err(|_| "Failed to parse tunnel list".to_string())?;
    Ok(tunnels.as_array().cloned().unwrap_or_default())
}

async fn find_tunnel(tunnel_id: &str) -> Result<Option<Value>, String> {
    Ok(list_tunnels().await?.into_iter().find(|t| t["id"].as_str() == Some(tunnel_id)))
}

// ==================== cloudflared / auth ====================

#[tauri::command]
pub async fn cloudflared_check() -> Value {
    match cloudflared::find_executable() {
        Some(path) => json!({ "installed": true, "path": path_string(&path) }),
        None => json!({ "installed": false, "path": "" }),
    }
}

#[tauri::command]
pub async fn auth_login() -> Value {
    let out = cloudflared::run(&["tunnel", "login"]).await;
    if out.error.is_some() {
        return failure(out.error_message());
    }
    json!({ "success": true, "message": out.stdout })
}

#[tauri::command]
pub async fn auth_check() -> Value {
    json!({ "authenticated": config::cloudflared_dir().join("cert.pem").exists() })
}

#[tauri::command]
pub async fn auth_logout() -> Value {
    let cert = config::cloudflared_dir().join("cert.pem");
    if cert.exists() {
        if let Err(e) = fs::remove_file(&cert) {
            return failure(e.to_string());
        }
    }
    json!({ "success": true })
}

// ==================== config ====================

#[tauri::command]
pub async fn config_get() -> Value {
    let path = path_string(&config::config_path());
    if !config::config_path().exists() {
        return json!({ "success": true, "config": null, "raw": "", "path": path });
    }
    let result = fs::read_to_string(config::config_path())
        .map_err(|e| e.to_string())
        .and_then(|raw| config::parse_yaml(&raw).map(|parsed| (raw, parsed)));
    match result {
        Ok((raw, parsed)) => json!({ "success": true, "config": parsed, "raw": raw, "path": path }),
        Err(e) => json!({ "success": false, "error": e, "path": path }),
    }
}

#[tauri::command]
pub async fn config_save(config: Option<Value>, raw: Option<String>) -> Value {
    let content = match raw.filter(|r| !r.is_empty()) {
        Some(raw) => Ok(raw),
        None => config::dump_yaml(&config.unwrap_or(Value::Null)),
    };
    match content.and_then(|c| config::write_config(&c)) {
        Ok(()) => json!({ "success": true, "path": path_string(&config::config_path()) }),
        Err(e) => failure(e),
    }
}

// ==================== tunnel list / create / update / delete ====================

#[tauri::command]
pub async fn tunnel_list() -> Value {
    let tunnels = match list_tunnels().await {
        Ok(t) => t,
        Err(e) => return json!({ "success": false, "error": e, "tunnels": [] }),
    };
    let config = config::read_config().ok().flatten();
    let tunnels: Vec<Value> = tunnels
        .into_iter()
        .map(|mut tunnel| {
            let credentials = config::tunnel_credentials_path(config.as_ref(), &tunnel);
            tunnel["isActive"] = json!(config::is_configured_tunnel(config.as_ref(), &tunnel));
            tunnel["credentialsFile"] = json!(path_string(&credentials));
            tunnel["hasCredentials"] = json!(credentials.exists());
            tunnel
        })
        .collect();
    json!({ "success": true, "tunnels": tunnels })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelRoute {
    hostname: String,
    service: String,
    #[serde(rename = "noTLSVerify", default)]
    no_tls_verify: bool,
    #[serde(default)]
    overwrite_dns: bool,
}

#[tauri::command]
pub async fn tunnel_create(name: String, make_active: bool, route: Option<TunnelRoute>) -> Value {
    if !Regex::new(r"^[A-Za-z0-9][A-Za-z0-9._-]*$").unwrap().is_match(&name) {
        return failure(r#"Tunnel name may only contain letters, digits, ".", "_" and "-""#);
    }
    if let Some(route) = &route {
        // Ingress rules live in config.yml, so a route needs the tunnel to be the configured one
        if !make_active {
            return failure("A public hostname can only be added when the tunnel is used in config.yml");
        }
        let hostname_ok = route.hostname.len() <= 253
            && Regex::new(r"^([A-Za-z0-9-]{1,63}\.)+[A-Za-z]{2,63}$").unwrap().is_match(&route.hostname);
        if !hostname_ok {
            return failure("Invalid public hostname");
        }
        if !Regex::new(r"^https?://[^\s/:]+(:\d{1,5})?$").unwrap().is_match(&route.service) {
            return failure("Invalid local service URL");
        }
    }

    let out = cloudflared::run(&["tunnel", "create", "--output", "json", &name]).await;
    if out.error.is_some() {
        return failure(out.error_message());
    }

    let tunnel_id = serde_json::from_str::<Value>(&out.stdout)
        .ok()
        .and_then(|v| v["id"].as_str().map(String::from))
        .or_else(|| {
            let uuid = Regex::new(r"(?i)[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}").unwrap();
            uuid.find(&format!("{}\n{}", out.stdout, out.stderr)).map(|m| m.as_str().to_string())
        });
    let Some(tunnel_id) = tunnel_id else {
        return failure("Tunnel was created but its ID could not be determined");
    };

    let credentials_file = path_string(&config::cloudflared_dir().join(format!("{tunnel_id}.json")));
    let config_result = (|| {
        if make_active {
            config::set_configured_tunnel(&tunnel_id, &credentials_file)?;
        }
        if let Some(route) = &route {
            config::add_ingress_rule(&route.hostname, &route.service, route.no_tls_verify)?;
        }
        Ok::<(), String>(())
    })();
    if let Err(e) = config_result {
        return failure(format!("Tunnel created, but config.yml could not be updated: {e}"));
    }

    // Point the public hostname at the new tunnel
    let mut dns_error = None;
    if let Some(route) = &route {
        let mut args = vec!["tunnel", "route", "dns"];
        if route.overwrite_dns {
            args.push("-f");
        }
        args.push(&tunnel_id);
        args.push(&route.hostname);
        let dns = cloudflared::run(&args).await;
        if dns.error.is_some() {
            dns_error = Some(dns.error_message());
        }
    }

    json!({
        "success": true,
        "tunnel": { "id": tunnel_id, "name": name },
        "credentialsFile": credentials_file,
        "dnsError": dns_error,
    })
}

// Update tunnel: choose whether config.yml uses it, and restore its credentials file
#[tauri::command]
pub async fn tunnel_update(tunnel_id: String, make_active: bool, restore_credentials: bool) -> Value {
    let tunnel = match find_tunnel(&tunnel_id).await {
        Ok(Some(t)) => t,
        Ok(None) => return failure("Tunnel not found"),
        Err(e) => return failure(e),
    };

    let config = match config::read_config() {
        Ok(c) => c,
        Err(e) => return failure(e),
    };
    let was_active = config::is_configured_tunnel(config.as_ref(), &tunnel);
    let credentials_file: PathBuf = config::tunnel_credentials_path(config.as_ref(), &tunnel);
    let credentials = path_string(&credentials_file);
    let id = tunnel["id"].as_str().unwrap_or_default();

    if restore_credentials || (make_active && !credentials_file.exists()) {
        let out = cloudflared::run(&["tunnel", "token", "--cred-file", &credentials, id]).await;
        if out.error.is_some() {
            return failure(out.error_message());
        }
    }

    let result = if make_active {
        config::set_configured_tunnel(id, &credentials)
    } else if was_active {
        config::clear_configured_tunnel()
    } else {
        Ok(())
    };
    match result {
        Ok(()) => json!({ "success": true, "credentialsFile": credentials }),
        Err(e) => failure(e),
    }
}

// Delete tunnel (and its local credentials and config.yml reference)
#[tauri::command]
pub async fn tunnel_delete(app: AppHandle, tunnel_id: String, force: bool) -> Value {
    let tunnel = match find_tunnel(&tunnel_id).await {
        Ok(Some(t)) => t,
        Ok(None) => return failure("Tunnel not found"),
        Err(e) => return failure(e),
    };

    let config = match config::read_config() {
        Ok(c) => c,
        Err(e) => return failure(e),
    };
    let is_active = config::is_configured_tunnel(config.as_ref(), &tunnel);
    let credentials_file = config::tunnel_credentials_path(config.as_ref(), &tunnel);
    let id = tunnel["id"].as_str().unwrap_or_default();

    // Our own connector would keep the tunnel connected and block deletion
    let mut stopped_tunnel = false;
    if is_active && app.state::<TunnelState>().is_running() {
        process::stop(&app).await;
        stopped_tunnel = true;
    }

    let mut args = vec!["tunnel", "delete"];
    if force {
        args.push("-f");
    }
    args.push(id);
    let out = cloudflared::run(&args).await;
    if out.error.is_some() {
        return json!({ "success": false, "error": out.error_message(), "stoppedTunnel": stopped_tunnel });
    }

    let cleanup = (|| {
        if credentials_file.exists() {
            fs::remove_file(&credentials_file).map_err(|e| e.to_string())?;
        }
        if is_active {
            config::clear_configured_tunnel()?;
        }
        Ok::<(), String>(())
    })();
    match cleanup {
        Ok(()) => json!({ "success": true, "stoppedTunnel": stopped_tunnel }),
        Err(e) => failure(e),
    }
}

// ==================== tunnel process ====================

#[tauri::command]
pub async fn tunnel_status(app: AppHandle) -> Value {
    let (running, pid) = app.state::<TunnelState>().status();
    json!({ "running": running, "pid": pid })
}

#[tauri::command]
pub async fn tunnel_start(app: AppHandle, tunnel_name: Option<String>) -> Value {
    if app.state::<TunnelState>().is_running() {
        return failure("Tunnel is already running");
    }
    let (pid, exited) = match process::spawn(&app, tunnel_name.as_deref()) {
        Ok(spawned) => spawned,
        Err(e) => return failure(e),
    };
    match process::check_started(pid, &exited, Duration::from_secs(1)).await {
        Ok(pid) => json!({ "success": true, "pid": pid }),
        Err(e) => failure(e),
    }
}

#[tauri::command]
pub async fn tunnel_stop(app: AppHandle) -> Value {
    process::stop(&app).await;
    json!({ "success": true })
}

// Restart tunnel with retry logic
#[tauri::command]
pub async fn tunnel_restart(app: AppHandle, max_retries: Option<u32>, retry_delay: Option<u64>) -> Value {
    let max_retries = max_retries.unwrap_or(3).max(1);
    let retry_delay = Duration::from_millis(retry_delay.unwrap_or(2000));

    process::stop(&app).await;
    // Wait a bit for the old connections to close
    tokio::time::sleep(Duration::from_millis(1500)).await;

    let mut last_error = String::new();
    for attempt in 1..=max_retries {
        let result = match process::spawn(&app, None) {
            Ok((pid, exited)) => process::check_started(pid, &exited, Duration::from_secs(2)).await,
            Err(e) => Err(e),
        };
        match result {
            Ok(pid) => return json!({ "success": true, "pid": pid, "attempt": attempt }),
            Err(e) => {
                process::emit_log(
                    &app,
                    "stderr",
                    format!(
                        "Restart attempt {attempt}/{max_retries} failed: {e}. {}",
                        if attempt < max_retries { "Retrying..." } else { "Giving up." }
                    ),
                );
                last_error = e;
                if attempt < max_retries {
                    tokio::time::sleep(retry_delay).await;
                }
            }
        }
    }
    failure(format!("Failed after {max_retries} attempts: {last_error}"))
}

// ==================== DNS routes ====================

#[tauri::command]
pub async fn tunnel_route_dns(tunnel_id: String, hostname: String, overwrite: Option<bool>) -> Value {
    let mut args = vec!["tunnel", "route", "dns"];
    if overwrite.unwrap_or(false) {
        args.push("-f");
    }
    args.push(&tunnel_id);
    args.push(&hostname);

    let out = cloudflared::run(&args).await;
    // cloudflared outputs to stderr for info messages
    let output = if out.stderr.is_empty() { out.stdout.clone() } else { out.stderr.clone() };

    if output.contains("is already configured to route to your tunnel") {
        return json!({
            "success": true,
            "message": "DNS record already configured",
            "alreadyExists": true,
            "status": "configured",
        });
    }
    if output.contains("Added CNAME") && output.contains("which will route to this tunnel") {
        return json!({
            "success": true,
            "message": "DNS CNAME record added successfully",
            "alreadyExists": false,
            "status": "configured",
        });
    }
    if let Some(error) = &out.error {
        let error = if output.is_empty() { error.clone() } else { output };
        return json!({ "success": false, "error": error, "status": "error" });
    }
    json!({
        "success": true,
        "message": if output.is_empty() { "DNS route configured".to_string() } else { output },
        "status": "configured",
    })
}

#[tauri::command]
pub async fn tunnel_delete_dns(hostname: String) -> Value {
    // cloudflared has no command to delete a DNS route
    let _ = hostname;
    json!({
        "success": false,
        "error": "DNS record deletion requires manual action in Cloudflare Dashboard",
        "manual": true,
    })
}

// ==================== shell ====================

#[tauri::command]
pub async fn shell_open_external(app: AppHandle, url: String) -> Value {
    match app.opener().open_url(url, None::<&str>) {
        Ok(()) => json!({ "success": true }),
        Err(e) => failure(e.to_string()),
    }
}

// Open the folder containing a file
#[tauri::command]
pub async fn shell_open_path(app: AppHandle, file_path: String) -> Value {
    let dir = Path::new(&file_path).parent().map(Path::to_path_buf).unwrap_or_default();
    match app.opener().open_path(path_string(&dir), None::<&str>) {
        Ok(()) => json!({ "success": true }),
        Err(e) => failure(e.to_string()),
    }
}

// ==================== network ====================

async fn can_connect(host: &str, port: u16, timeout: Duration) -> bool {
    matches!(
        tokio::time::timeout(timeout, TcpStream::connect((host, port))).await,
        Ok(Ok(_))
    )
}

#[tauri::command]
pub async fn network_check_port(host: String, port: u16) -> Value {
    json!({ "reachable": can_connect(&host, port, Duration::from_secs(1)).await })
}

// Whether anything listens on a local port (replaces the lsof check, which only worked on macOS)
#[tauri::command]
pub async fn network_check_port_lsof(port: u16) -> Value {
    let listening = can_connect("127.0.0.1", port, Duration::from_millis(500)).await
        || can_connect("::1", port, Duration::from_millis(500)).await;
    json!({ "listening": listening, "process": null })
}

// Resolve a hostname with the system resolver (replaces the dig check)
#[tauri::command]
pub async fn dns_check(hostname: String) -> Value {
    let lookup = tokio::time::timeout(Duration::from_secs(3), tokio::net::lookup_host((hostname.as_str(), 443))).await;
    match lookup {
        Ok(Ok(addrs)) => {
            let mut addresses: Vec<String> = addrs.map(|a: SocketAddr| a.ip().to_string()).collect();
            addresses.dedup();
            if addresses.is_empty() {
                json!({ "resolved": false, "error": "No DNS records found" })
            } else {
                // Tunnel hostnames are proxied, so they resolve to Cloudflare addresses
                json!({ "resolved": true, "addresses": addresses, "isCloudflareTunnel": true })
            }
        }
        Ok(Err(e)) => json!({ "resolved": false, "error": e.to_string() }),
        Err(_) => json!({ "resolved": false, "error": "DNS lookup timed out" }),
    }
}
