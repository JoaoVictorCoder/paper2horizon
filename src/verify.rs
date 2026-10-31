//! The check after writing a crate: `cargo check`, and where in `src/lib.rs` it fails.

use horizon_plugin_api::Value;
use std::path::Path;
use std::process::Command;

/// The lines of `src/lib.rs` where `cargo check` finds errors (empty: it compiles).
pub fn error_lines(dir: &Path) -> Result<Vec<usize>, String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let out = Command::new(&cargo)
        .args(["check", "--message-format=json", "--quiet"])
        .current_dir(dir)
        .output()
        .map_err(|e| format!("couldn't run {cargo}: {e}"))?;
    let mut lines = Vec::new();
    let mut errors = 0;
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let Ok(v) = Value::from_json(line) else { continue };
        if v.get("reason").and_then(Value::as_str) != Some("compiler-message") {
            continue;
        }
        let Some(msg) = v.get("message") else { continue };
        if msg.get("level").and_then(Value::as_str) != Some("error") {
            continue;
        }
        errors += 1;
        for span in msg.get("spans").and_then(Value::as_list).unwrap_or_default() {
            if span.get("file_name").and_then(Value::as_str) == Some("src/lib.rs") {
                if let Some(n) = span.get("line_start").and_then(Value::as_i64) {
                    lines.push(n as usize);
                }
            }
        }
    }
    if !out.status.success() && lines.is_empty() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let tail: Vec<&str> = stderr.lines().rev().take(6).collect();
        let tail: Vec<&str> = tail.into_iter().rev().collect();
        return Err(if errors > 0 {
            format!("{errors} errors outside src/lib.rs")
        } else {
            format!("cargo check failed: {}", tail.join(" / "))
        });
    }
    lines.sort_unstable();
    lines.dedup();
    Ok(lines)
}
