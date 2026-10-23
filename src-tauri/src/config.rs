use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Map, Value};

pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_default()
}

pub fn cloudflared_dir() -> PathBuf {
    home_dir().join(".cloudflared")
}

pub fn config_path() -> PathBuf {
    cloudflared_dir().join("config.yml")
}

pub fn parse_yaml(raw: &str) -> Result<Value, String> {
    let yaml: serde_yaml::Value = serde_yaml::from_str(raw).map_err(|e| e.to_string())?;
    serde_json::to_value(yaml).map_err(|e| e.to_string())
}

pub fn dump_yaml(config: &Value) -> Result<String, String> {
    serde_yaml::to_string(config).map_err(|e| e.to_string())
}

// The parsed config.yml as an object, or None when the file does not exist
pub fn read_config() -> Result<Option<Map<String, Value>>, String> {
    let path = config_path();
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    match parse_yaml(&raw)? {
        Value::Object(map) => Ok(Some(map)),
        _ => Ok(Some(Map::new())),
    }
}

// Write config.yml, backing up the existing file first
pub fn write_config(content: &str) -> Result<(), String> {
    let path = config_path();
    let dir = cloudflared_dir();

    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    if path.exists() {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
        fs::copy(&path, dir.join(format!("config.yml.backup.{now}"))).map_err(|e| e.to_string())?;

        // Keep only the latest 10 backups
        let mut backups: Vec<(u128, PathBuf)> = fs::read_dir(&dir)
            .map_err(|e| e.to_string())?
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                let time = name.strip_prefix("config.yml.backup.")?.parse().unwrap_or(0);
                Some((time, entry.path()))
            })
            .collect();
        backups.sort_by(|a, b| b.0.cmp(&a.0));
        for (_, old) in backups.into_iter().skip(10) {
            if let Err(e) = fs::remove_file(&old) {
                eprintln!("Failed to delete old backup: {e}");
            }
        }
    }

    fs::write(&path, content).map_err(|e| e.to_string())
}

fn write_config_map(config: Map<String, Value>) -> Result<(), String> {
    write_config(&dump_yaml(&Value::Object(config))?)
}

// config.yml may reference the tunnel by UUID or by name
pub fn is_configured_tunnel(config: Option<&Map<String, Value>>, tunnel: &Value) -> bool {
    let Some(configured) = config.and_then(|c| c.get("tunnel")).and_then(Value::as_str) else {
        return false;
    };
    tunnel["id"].as_str() == Some(configured) || tunnel["name"].as_str() == Some(configured)
}

pub fn tunnel_credentials_path(config: Option<&Map<String, Value>>, tunnel: &Value) -> PathBuf {
    if is_configured_tunnel(config, tunnel) {
        if let Some(file) = config.and_then(|c| c.get("credentials-file")).and_then(Value::as_str) {
            return PathBuf::from(file);
        }
    }
    cloudflared_dir().join(format!("{}.json", tunnel["id"].as_str().unwrap_or_default()))
}

pub fn set_configured_tunnel(tunnel_id: &str, credentials_file: &str) -> Result<(), String> {
    let mut config = read_config()?.unwrap_or_default();
    config.insert("tunnel".into(), json!(tunnel_id));
    config.insert("credentials-file".into(), json!(credentials_file));
    // cloudflared refuses to run without a catch-all ingress rule
    let has_rules = config.get("ingress").and_then(Value::as_array).is_some_and(|r| !r.is_empty());
    if !has_rules {
        config.insert("ingress".into(), json!([{ "service": "http_status:404" }]));
    }
    write_config_map(config)
}

pub fn clear_configured_tunnel() -> Result<(), String> {
    let Some(mut config) = read_config()? else {
        return Ok(());
    };
    config.remove("tunnel");
    config.remove("credentials-file");
    write_config_map(config)
}

// Add (or replace) the ingress rule for a hostname, keeping the catch-all last
pub fn add_ingress_rule(hostname: &str, service: &str, no_tls_verify: bool) -> Result<(), String> {
    let mut config = read_config()?.unwrap_or_default();
    let mut rule = json!({ "hostname": hostname, "service": service });
    if service.starts_with("https://") && no_tls_verify {
        rule["originRequest"] = json!({ "noTLSVerify": true });
    }

    let existing = config.get("ingress").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut rules: Vec<Value> = existing
        .iter()
        .filter(|r| r["hostname"].as_str().is_some_and(|h| h != hostname))
        .cloned()
        .collect();
    let catch_all = existing
        .iter()
        .find(|r| r.get("hostname").is_none())
        .cloned()
        .unwrap_or_else(|| json!({ "service": "http_status:404" }));
    rules.push(rule);
    rules.push(catch_all);

    config.insert("ingress".into(), Value::Array(rules));
    write_config_map(config)
}
