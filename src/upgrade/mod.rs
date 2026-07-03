//! Self-upgrade: fetch GitHub releases and replace the running `paper` binary.

use anyhow::{Context, Result, bail};
use flate2::read::GzDecoder;
use serde::Serialize;
use std::fs::{self, File};

use std::path::{Path, PathBuf};
use tar::Archive;

pub const DEFAULT_REPO: &str = "tsui66/paper-trading-terminal";
const PACKAGE_NAME: &str = "paper-trading-terminal";
const BIN_NAME: &str = "paper";

#[derive(Debug, Clone, Serialize)]
pub struct UpgradeStatus {
    pub current: String,
    pub latest: String,
    pub update_available: bool,
    pub installed: bool,
    pub target: Option<String>,
}

#[derive(Debug, Clone, Copy)]
struct Platform {
    os: &'static str,
    arch: &'static str,
    libc_musl: bool,
}

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn normalize_tag(tag: &str) -> String {
    tag.trim().trim_start_matches('v').to_string()
}

pub fn version_lt(current: &str, latest: &str) -> bool {
    parse_triple(current) < parse_triple(latest)
}

fn parse_triple(v: &str) -> (u64, u64, u64) {
    let normalized = normalize_tag(v);
    let mut parts = normalized.split('.');
    let a = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let b = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let c = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    (a, b, c)
}

fn detect_platform() -> Platform {
    let arch = match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        other => other,
    };
    let os = std::env::consts::OS;
    Platform {
        os,
        arch,
        libc_musl: os == "linux",
    }
}

fn artifact_names(platform: Platform) -> Vec<String> {
    if platform.os == "windows" {
        return vec![format!("{PACKAGE_NAME}-windows-{}.zip", platform.arch)];
    }
    let mut names = Vec::new();
    if platform.libc_musl {
        names.push(format!(
            "{PACKAGE_NAME}-{}-musl-{}.tar.gz",
            platform.os, platform.arch
        ));
    }
    names.push(format!(
        "{PACKAGE_NAME}-{}-{}.tar.gz",
        platform.os, platform.arch
    ));
    names
}

pub fn fetch_latest_tag(repo: &str) -> Result<String> {
    let url = format!("https://github.com/{repo}/releases/latest");
    let client = http_client()?;
    let resp = client
        .get(&url)
        .send()
        .with_context(|| format!("GET {url}"))?;
    let final_url = resp.url().to_string();
    final_url
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .filter(|tag| tag.starts_with('v'))
        .ok_or_else(|| anyhow::anyhow!("could not parse latest release tag from {final_url}"))
}

fn http_client() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .user_agent(format!("paper/{}", current_version()))
        .redirect(reqwest::redirect::Policy::limited(8))
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .context("build HTTP client")
}

fn download_release(repo: &str, tag: &str, platform: Platform) -> Result<Vec<u8>> {
    let client = http_client()?;
    let mut last_err = None;
    for artifact in artifact_names(platform) {
        let url = format!("https://github.com/{repo}/releases/download/{tag}/{artifact}");
        match client.get(&url).send() {
            Ok(resp) if resp.status().is_success() => {
                return resp
                    .bytes()
                    .context("read release bytes")
                    .map(|b| b.to_vec());
            }
            Ok(resp) => {
                last_err = Some(format!("{} -> HTTP {}", url, resp.status()));
            }
            Err(e) => {
                last_err = Some(format!("{url} -> {e}"));
            }
        }
    }
    bail!(
        "no release artifact found for this platform (try install script instead)\n{}",
        last_err.unwrap_or_else(|| "unknown error".into())
    )
}

fn extract_unix(bytes: &[u8], dest: &Path) -> Result<()> {
    let decoder = GzDecoder::new(bytes);
    let mut archive = Archive::new(decoder);
    for entry in archive.entries().context("read tar entries")? {
        let mut entry = entry.context("tar entry")?;
        let path = entry.path().context("tar path")?;
        if path.file_name().and_then(|n| n.to_str()) != Some(BIN_NAME) {
            continue;
        }
        let out = dest.join(BIN_NAME);
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut outfile =
            File::create(&out).with_context(|| format!("create {}", out.display()))?;
        std::io::copy(&mut entry, &mut outfile).context("extract binary")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = outfile.metadata()?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&out, perms)?;
        }
        return Ok(());
    }
    bail!("archive did not contain '{BIN_NAME}' binary")
}

fn extract_windows(bytes: &[u8], dest: &Path) -> Result<()> {
    let reader = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(reader).context("open zip")?;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).context("zip index")?;
        let name = file.name().replace('\\', "/");
        if !name.ends_with("paper.exe") {
            continue;
        }
        let out = dest.join("paper.exe");
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut outfile =
            File::create(&out).with_context(|| format!("create {}", out.display()))?;
        std::io::copy(&mut file, &mut outfile).context("extract binary")?;
        return Ok(());
    }
    bail!("zip did not contain paper.exe")
}

fn extract_binary(bytes: &[u8], dest: &Path) -> Result<PathBuf> {
    if cfg!(windows) {
        extract_windows(bytes, dest)?;
        Ok(dest.join("paper.exe"))
    } else {
        extract_unix(bytes, dest)?;
        Ok(dest.join(BIN_NAME))
    }
}

#[cfg(unix)]
fn replace_in_place(current: &Path, new_bin: &Path) -> Result<()> {
    fs::copy(new_bin, current).with_context(|| {
        format!(
            "replace {} — try: curl -sSL https://github.com/{DEFAULT_REPO}/raw/main/install | sh",
            current.display()
        )
    })?;
    Ok(())
}

#[cfg(windows)]
fn replace_in_place(current: &Path, new_bin: &Path) -> Result<()> {
    let script = std::env::temp_dir().join(format!("paper-upgrade-{}.ps1", std::process::id()));
    let ps1 = format!(
        r#"$ErrorActionPreference = 'Stop'
Start-Sleep -Seconds 2
Copy-Item -LiteralPath '{new}' -Destination '{dest}' -Force
Remove-Item -LiteralPath '{script}' -Force
"#,
        new = new_bin.display(),
        dest = current.display(),
        script = script.display()
    );
    fs::write(&script, ps1).context("write upgrade script")?;
    std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            &script.to_string_lossy(),
        ])
        .spawn()
        .context("spawn upgrade script")?;
    Ok(())
}

pub fn run_upgrade(repo: &str, version: Option<&str>, check_only: bool) -> Result<UpgradeStatus> {
    let current = current_version().to_string();
    let latest_tag = match version {
        Some(v) => {
            if v.starts_with('v') {
                v.to_string()
            } else {
                format!("v{v}")
            }
        }
        None => fetch_latest_tag(repo)?,
    };
    let latest = normalize_tag(&latest_tag);
    let update_available = version_lt(&current, &latest);

    let target = std::env::current_exe().ok();

    if check_only {
        return Ok(UpgradeStatus {
            current,
            latest,
            update_available,
            installed: false,
            target: target.map(|p| p.display().to_string()),
        });
    }

    if !update_available && version.is_none() {
        return Ok(UpgradeStatus {
            current,
            latest,
            update_available: false,
            installed: false,
            target: target.map(|p| p.display().to_string()),
        });
    }

    let platform = detect_platform();
    let bytes = download_release(repo, &latest_tag, platform)?;
    let tmp = tempfile::tempdir().context("tempdir")?;
    let new_bin = extract_binary(&bytes, tmp.path())?;
    let current_exe = std::env::current_exe().context("current_exe")?;
    replace_in_place(&current_exe, &new_bin)?;

    Ok(UpgradeStatus {
        current: current.clone(),
        latest,
        update_available: true,
        installed: true,
        target: Some(current_exe.display().to_string()),
    })
}

pub fn cmd_upgrade(json: bool, args: &crate::cli::UpgradeArgs) -> Result<()> {
    let repo = args
        .repo
        .as_deref()
        .unwrap_or(DEFAULT_REPO)
        .trim()
        .trim_end_matches('/')
        .to_string();
    if repo.is_empty() || !repo.contains('/') {
        bail!("invalid repo (expected owner/name)");
    }

    let status = run_upgrade(&repo, args.version.as_deref(), args.check)?;

    if json {
        crate::utils::output_json(&status)?;
        return Ok(());
    }

    if args.check {
        if status.update_available {
            println!("Update available: {} -> v{}", status.current, status.latest);
            println!("Run: paper upgrade");
        } else {
            println!(
                "paper {} is up to date (latest v{})",
                status.current, status.latest
            );
        }
        return Ok(());
    }

    if !status.installed {
        println!(
            "paper {} is already the latest release (v{})",
            status.current, status.latest
        );
        return Ok(());
    }

    println!("Upgraded paper {} -> v{}", status.current, status.latest);
    if let Some(path) = &status.target {
        println!("Installed to: {path}");
    }
    if cfg!(windows) {
        println!("Restart your terminal, then run: paper --version");
    } else {
        println!("Run: paper --version");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_lt_orders_semver_triples() {
        assert!(version_lt("0.0.1", "0.0.2"));
        assert!(version_lt("0.0.9", "0.1.0"));
        assert!(!version_lt("0.1.0", "0.0.9"));
        assert!(!version_lt("1.0.0", "1.0.0"));
    }

    #[test]
    fn normalize_tag_strips_v() {
        assert_eq!(normalize_tag("v0.0.1"), "0.0.1");
        assert_eq!(normalize_tag("0.0.1"), "0.0.1");
    }

    #[test]
    fn linux_artifact_prefers_musl_first() {
        let names = artifact_names(Platform {
            os: "linux",
            arch: "amd64",
            libc_musl: true,
        });
        assert!(names[0].contains("musl"));
    }

    #[test]
    fn windows_artifact_is_zip() {
        let names = artifact_names(Platform {
            os: "windows",
            arch: "amd64",
            libc_musl: false,
        });
        assert_eq!(names.len(), 1);
        assert!(names[0].ends_with(".zip"));
    }
}
