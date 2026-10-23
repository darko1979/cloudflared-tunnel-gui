use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::sync::{oneshot, watch};

use crate::{cloudflared, config};

// Exit code once the process has exited: None while running, Some(code) after
type ExitWatch = watch::Receiver<Option<Option<i32>>>;

struct Running {
    id: u64,
    pid: Option<u32>,
    kill: Option<oneshot::Sender<()>>,
    exited: ExitWatch,
}

#[derive(Default)]
pub struct TunnelState {
    running: Mutex<Option<Running>>,
    next_id: AtomicU64,
}

impl TunnelState {
    pub fn status(&self) -> (bool, Option<u32>) {
        match self.running.lock().unwrap().as_ref() {
            Some(r) => (true, r.pid),
            None => (false, None),
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.lock().unwrap().is_some()
    }
}

fn forward_output<R>(app: AppHandle, stream: R, kind: &'static str)
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tauri::async_runtime::spawn(async move {
        let mut lines = BufReader::new(stream).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = app.emit("tunnel-log", json!({ "type": kind, "message": line }));
        }
    });
}

pub fn emit_log(app: &AppHandle, kind: &str, message: String) {
    let _ = app.emit("tunnel-log", json!({ "type": kind, "message": message }));
}

// Spawn `cloudflared tunnel run` and track it as the active tunnel process
pub fn spawn(app: &AppHandle, tunnel_name: Option<&str>) -> Result<(Option<u32>, ExitWatch), String> {
    let state = app.state::<TunnelState>();

    let mut cmd = cloudflared::command();
    cmd.arg("tunnel");
    let config_path = config::config_path();
    if config_path.exists() {
        cmd.arg("--config").arg(&config_path);
    }
    cmd.arg("run");
    if let Some(name) = tunnel_name {
        cmd.arg(name);
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let pid = child.id();
    if let Some(stdout) = child.stdout.take() {
        forward_output(app.clone(), stdout, "stdout");
    }
    if let Some(stderr) = child.stderr.take() {
        forward_output(app.clone(), stderr, "stderr");
    }

    let id = state.next_id.fetch_add(1, Ordering::SeqCst);
    let (kill_tx, kill_rx) = oneshot::channel::<()>();
    let (exit_tx, exit_rx) = watch::channel(None);
    *state.running.lock().unwrap() = Some(Running {
        id,
        pid,
        kill: Some(kill_tx),
        exited: exit_rx.clone(),
    });
    crate::update_tray_tooltip(app, true);

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        // A kill request, or the handle being dropped, both stop the process
        let status = tokio::select! {
            status = child.wait() => status,
            _ = kill_rx => {
                let _ = child.kill().await;
                child.wait().await
            }
        };
        let code = status.ok().and_then(|s| s.code());
        let _ = exit_tx.send(Some(code));
        let _ = app.emit("tunnel-exit", json!({ "code": code }));

        // Only clear the slot if this is still the active process
        let state = app.state::<TunnelState>();
        let mut running = state.running.lock().unwrap();
        if running.as_ref().is_some_and(|r| r.id == id) {
            *running = None;
            drop(running);
            crate::update_tray_tooltip(&app, false);
        }
    });

    Ok((pid, exit_rx))
}

// Stop the active tunnel process, waiting up to 5 seconds for it to exit
pub async fn stop(app: &AppHandle) {
    let running = app.state::<TunnelState>().running.lock().unwrap().take();
    let Some(mut running) = running else {
        return;
    };
    if let Some(kill) = running.kill.take() {
        let _ = kill.send(());
    }
    let _ = tokio::time::timeout(Duration::from_secs(5), running.exited.wait_for(|e| e.is_some())).await;
    crate::update_tray_tooltip(app, false);
}

// Give a freshly spawned process time to fail, then report how it went
pub async fn check_started(pid: Option<u32>, exited: &ExitWatch, wait: Duration) -> Result<Option<u32>, String> {
    tokio::time::sleep(wait).await;
    match *exited.borrow() {
        Some(code) => Err(match code {
            Some(c) => format!("Process exited with code {c}"),
            None => "Process exited immediately".into(),
        }),
        None => Ok(pid),
    }
}
