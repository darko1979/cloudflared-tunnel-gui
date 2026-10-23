use std::env;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Stdio;

use tokio::process::Command;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

// The app may inherit a PATH from before cloudflared was installed (or, on
// macOS, no shell PATH at all), so add the folders the installers use
fn extra_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if cfg!(windows) {
        let program_files_x86 = env::var("ProgramFiles(x86)").unwrap_or_else(|_| r"C:\Program Files (x86)".into());
        let program_files = env::var("ProgramFiles").unwrap_or_else(|_| r"C:\Program Files".into());
        paths.push(PathBuf::from(program_files_x86).join("cloudflared"));
        paths.push(PathBuf::from(program_files).join("cloudflared"));
        if let Ok(local) = env::var("LOCALAPPDATA") {
            paths.push(PathBuf::from(local).join("Microsoft").join("WinGet").join("Links"));
        }
    } else {
        for p in ["/opt/homebrew/bin", "/usr/local/bin", "/opt/local/bin", "/usr/bin", "/bin"] {
            paths.push(PathBuf::from(p));
        }
        if let Ok(home) = env::var("HOME") {
            paths.push(PathBuf::from(home).join(".local").join("bin"));
        }
    }
    paths
}

fn search_paths() -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = env::var_os("PATH")
        .map(|p| env::split_paths(&p).collect())
        .unwrap_or_default();
    for p in extra_paths() {
        if !paths.contains(&p) {
            paths.push(p);
        }
    }
    paths
}

pub fn fixed_path() -> OsString {
    env::join_paths(search_paths()).unwrap_or_default()
}

pub fn find_executable() -> Option<PathBuf> {
    let name = if cfg!(windows) { "cloudflared.exe" } else { "cloudflared" };
    search_paths().into_iter().map(|dir| dir.join(name)).find(|p| p.is_file())
}

// A cloudflared command with the fixed PATH and no console window
pub fn command() -> Command {
    let program = find_executable().map(OsString::from).unwrap_or_else(|| "cloudflared".into());
    let mut cmd = Command::new(program);
    cmd.env("PATH", fixed_path());
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

pub struct RunOutput {
    pub stdout: String,
    pub stderr: String,
    pub error: Option<String>,
}

impl RunOutput {
    // Mirrors the Electron version's `stderr || error.message`
    pub fn error_message(&self) -> String {
        if !self.stderr.trim().is_empty() {
            self.stderr.trim().to_string()
        } else {
            self.error.clone().unwrap_or_default()
        }
    }
}

// Run cloudflared with an argument list (no shell, so tunnel names are passed verbatim)
pub async fn run(args: &[&str]) -> RunOutput {
    let output = command()
        .args(args)
        .stdin(Stdio::null())
        .output()
        .await;

    match output {
        Ok(out) => RunOutput {
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            error: if out.status.success() {
                None
            } else {
                Some(format!("cloudflared exited with {}", out.status))
            },
        },
        Err(e) => RunOutput {
            stdout: String::new(),
            stderr: String::new(),
            error: Some(e.to_string()),
        },
    }
}
