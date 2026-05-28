//! Self-update: check GitHub Releases for a newer `gfit-cli` and replace this
//! binary in place. Talks to GitHub, not the GFIT API — no login required.

use serde_json::Value;
use std::fs;
use std::path::PathBuf;

const OWNER: &str = "dickwu";
const REPO: &str = "gfit-cli";

/// Exact target triple this binary was built for (set by `build.rs`). It is also
/// the suffix of the matching release asset, e.g. `gfit-cli-aarch64-apple-darwin`.
const TARGET: &str = env!("GFIT_TARGET");

fn current() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Name of the release asset built for this platform.
fn asset_basename() -> String {
    if cfg!(target_os = "windows") {
        format!("gfit-cli-{TARGET}.exe")
    } else {
        format!("gfit-cli-{TARGET}")
    }
}

/// Parse `"1.2.3"` (tolerating a leading `v` and `-`/`+` suffixes) into a
/// comparable `(major, minor, patch)` tuple.
fn semver(s: &str) -> (u64, u64, u64) {
    let s = s.trim().trim_start_matches('v');
    let mut it = s.split(['.', '-', '+']);
    let n = |o: Option<&str>| o.and_then(|x| x.parse::<u64>().ok()).unwrap_or(0);
    (n(it.next()), n(it.next()), n(it.next()))
}

fn is_newer(latest: &str, cur: &str) -> bool {
    semver(latest) > semver(cur)
}

struct Latest {
    version: String,
    tag: String,
    asset_url: Option<String>,
    asset_name: String,
}

fn fetch_latest() -> Result<Latest, String> {
    let url = format!("https://api.github.com/repos/{OWNER}/{REPO}/releases/latest");
    let v = crate::client::get_json(&url)?;
    let tag = v
        .get("tag_name")
        .and_then(Value::as_str)
        .ok_or("no published release found for this repo yet")?
        .to_string();

    let want = asset_basename();
    let asset_url = v
        .get("assets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|a| a.get("name").and_then(Value::as_str) == Some(want.as_str()))
        .and_then(|a| a.get("browser_download_url").and_then(Value::as_str))
        .map(String::from);

    Ok(Latest {
        version: tag.trim_start_matches('v').to_string(),
        tag,
        asset_url,
        asset_name: want,
    })
}

/// `--check` only reports; otherwise download + install when newer (or `--force`).
pub fn run(check_only: bool, force: bool) -> Result<(), String> {
    let cur = current();
    println!("current version: {cur}  ({TARGET})");
    println!("checking {OWNER}/{REPO} releases…");
    let latest = fetch_latest()?;
    println!("latest version:  {}", latest.version);

    let newer = is_newer(&latest.version, cur);

    if check_only {
        if newer {
            println!("\nUpdate available: {cur} -> {}", latest.version);
            println!("Run `gfit-cli self.update` to upgrade.");
        } else {
            println!("\nYou are on the latest version.");
        }
        return Ok(());
    }

    if !newer && !force {
        println!("\nAlready up to date.");
        return Ok(());
    }

    let asset_url = latest.asset_url.ok_or_else(|| {
        format!(
            "release {} has no asset for this platform ({})",
            latest.tag, latest.asset_name
        )
    })?;

    println!("\ndownloading {} …", latest.asset_name);
    let bytes = crate::client::download_bytes(&asset_url)?;
    if bytes.len() < 1024 {
        return Err(format!(
            "downloaded file looks wrong ({} bytes) — aborting",
            bytes.len()
        ));
    }

    install(&bytes)?;
    println!(
        "Upgraded {} -> {}. Run `gfit-cli --version` to confirm.",
        cur, latest.version
    );
    Ok(())
}

/// Write the new binary next to the current one and swap it in. Uses an atomic
/// rename where the OS allows replacing a running binary (Linux/macOS); falls
/// back to moving the current binary aside (Windows / busy file).
fn install(bytes: &[u8]) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("cannot locate current executable: {e}"))?;
    let dir = exe.parent().ok_or("current executable has no parent directory")?;
    let tmp: PathBuf = dir.join(format!(".gfit-cli-update-{}", std::process::id()));

    fs::write(&tmp, bytes).map_err(|e| {
        format!(
            "cannot write to {} ({e}). If gfit-cli is in a system directory, re-run with elevated permissions (e.g. sudo).",
            dir.display()
        )
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(e) = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755)) {
            let _ = fs::remove_file(&tmp);
            return Err(format!("cannot mark update executable: {e}"));
        }
    }

    if fs::rename(&tmp, &exe).is_ok() {
        return Ok(());
    }

    // Fallback for platforms that refuse to replace a running binary.
    let old = exe.with_extension("old");
    let _ = fs::remove_file(&old);
    if let Err(e) = fs::rename(&exe, &old) {
        let _ = fs::remove_file(&tmp);
        return Err(format!(
            "cannot replace current binary: {e}. Try with elevated permissions (e.g. sudo)."
        ));
    }
    if let Err(e) = fs::rename(&tmp, &exe) {
        let _ = fs::rename(&old, &exe); // best-effort rollback
        let _ = fs::remove_file(&tmp);
        return Err(format!("cannot install update: {e}"));
    }
    let _ = fs::remove_file(&old);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_parses_and_orders() {
        assert_eq!(semver("v1.2.3"), (1, 2, 3));
        assert_eq!(semver("0.1.0"), (0, 1, 0));
        assert_eq!(semver("2.0.0-rc1"), (2, 0, 0));
        assert!(is_newer("0.2.0", "0.1.9"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.2.0"));
    }

    #[test]
    fn asset_name_includes_target() {
        assert!(asset_basename().starts_with("gfit-cli-"));
        assert!(asset_basename().contains(TARGET));
    }
}
