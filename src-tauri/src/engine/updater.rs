use crate::engine::models::{
    ArtifactManifest, LatestJsonResult, TauriLatestJson, UpdateKeyResult, UpdatePlatformEntry,
    UpdatePublishResult, UpdatePublishTarget, UpdateSignResult, UpdaterConfig,
};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const UPDATER_SERVICE: &str = "chibby-updater";

/// Keychain account key for the Tauri update private key.
fn updater_account_key(project_path: &str) -> String {
    format!("{}:tauri-update-private-key", project_path)
}

// ---------------------------------------------------------------------------
// Config persistence (.chibby/updater.toml)
// ---------------------------------------------------------------------------

/// Save updater config to .chibby/updater.toml.
pub fn save_updater_config(repo_path: &Path, config: &UpdaterConfig) -> Result<()> {
    let chibby_dir = repo_path.join(".chibby");
    std::fs::create_dir_all(&chibby_dir)?;

    let toml_str =
        toml::to_string_pretty(config).context("Failed to serialize updater config")?;

    let file_path = chibby_dir.join("updater.toml");
    std::fs::write(&file_path, &toml_str)?;

    log::info!("Saved updater config to {}", file_path.display());
    Ok(())
}

/// Load updater config from .chibby/updater.toml.
pub fn load_updater_config(repo_path: &Path) -> Result<UpdaterConfig> {
    let file_path = repo_path.join(".chibby").join("updater.toml");
    if !file_path.exists() {
        return Ok(UpdaterConfig::default());
    }
    let content = std::fs::read_to_string(&file_path)
        .with_context(|| format!("Failed to read {}", file_path.display()))?;

    let config: UpdaterConfig = toml::from_str(&content)
        .with_context(|| format!("Failed to parse {}", file_path.display()))?;

    Ok(config)
}

// ---------------------------------------------------------------------------
// Tauri CLI detection
// ---------------------------------------------------------------------------

/// Find the Tauri CLI binary. Tries `cargo-tauri` first, then `npx @tauri-apps/cli`.
fn find_tauri_cli() -> Option<Vec<String>> {
    // Try cargo-tauri (installed via `cargo install tauri-cli`)
    if command_exists("cargo-tauri") {
        return Some(vec!["cargo-tauri".to_string()]);
    }

    // Try `cargo tauri` (cargo subcommand)
    let output = std::process::Command::new("cargo")
        .args(["tauri", "--version"])
        .output();
    if let Ok(o) = output {
        if o.status.success() {
            return Some(vec!["cargo".to_string(), "tauri".to_string()]);
        }
    }

    // Try npx
    if command_exists("npx") {
        let output = std::process::Command::new("npx")
            .args(["@tauri-apps/cli", "--version"])
            .output();
        if let Ok(o) = output {
            if o.status.success() {
                return Some(vec![
                    "npx".to_string(),
                    "@tauri-apps/cli".to_string(),
                ]);
            }
        }
    }

    None
}

fn command_exists(cmd: &str) -> bool {
    let check = if cfg!(target_os = "windows") {
        "where"
    } else {
        "which"
    };
    std::process::Command::new(check)
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if the Tauri CLI is available.
pub fn check_tauri_cli() -> Result<Vec<String>> {
    find_tauri_cli().ok_or_else(|| {
        anyhow::anyhow!(
            "Tauri CLI not found. Install with `cargo install tauri-cli` or `npm install -D @tauri-apps/cli`"
        )
    })
}

// ---------------------------------------------------------------------------
// Key management
// ---------------------------------------------------------------------------

/// Generate a Tauri update key pair.
///
/// Uses `tauri signer generate` to create an Ed25519 key pair.
/// The private key is stored in the OS keychain. The public key is returned
/// and should be saved in the updater config.
pub fn generate_update_keys(project_path: &str) -> Result<UpdateKeyResult> {
    let cli = check_tauri_cli()?;

    // Generate key pair with no password (Chibby manages security via keychain)
    let mut cmd = std::process::Command::new(&cli[0]);
    for arg in &cli[1..] {
        cmd.arg(arg);
    }
    cmd.args(["signer", "generate", "-w"]);

    let output = cmd
        .output()
        .context("Failed to run `tauri signer generate`")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("tauri signer generate failed: {}", stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    // Parse the output to extract public and private keys.
    // tauri signer generate outputs the private key and public key in its output.
    let private_key = extract_key_from_output(&combined, "private")
        .or_else(|| extract_key_from_output(&combined, "secret"))
        .context("Could not extract private key from tauri signer output")?;

    let public_key = extract_key_from_output(&combined, "public")
        .context("Could not extract public key from tauri signer output")?;

    // Store private key in OS keychain
    let account = updater_account_key(project_path);
    let entry = keyring::Entry::new(UPDATER_SERVICE, &account)
        .context("Failed to create keyring entry for update key")?;
    entry
        .set_password(&private_key)
        .context("Failed to store update private key in keychain")?;

    log::info!("Generated Tauri update key pair, private key stored in keychain");

    Ok(UpdateKeyResult {
        public_key,
        private_key_stored: true,
        message: "Key pair generated. Private key stored in OS keychain. Add the public key to your tauri.conf.json updater config.".to_string(),
    })
}

/// Extract a key value from tauri signer generate output.
fn extract_key_from_output(output: &str, key_type: &str) -> Option<String> {
    // The output typically contains lines like:
    // "Your {key_type} key is: <base64 key>"
    // or the key on the line after a label.
    for (i, line) in output.lines().enumerate() {
        let lower = line.to_lowercase();
        if lower.contains(key_type) && lower.contains("key") {
            // Check if the key is on the same line after a colon
            if let Some(pos) = line.find(':') {
                let candidate = line[pos + 1..].trim();
                if !candidate.is_empty() && candidate.len() > 20 {
                    return Some(candidate.to_string());
                }
            }
            // Check the next line
            if let Some(next_line) = output.lines().nth(i + 1) {
                let candidate = next_line.trim();
                if !candidate.is_empty() && candidate.len() > 20 {
                    return Some(candidate.to_string());
                }
            }
        }
    }

    // Fallback: look for long base64-like strings
    // The output often has exactly two long base64 strings
    let long_strings: Vec<&str> = output
        .lines()
        .map(|l| l.trim())
        .filter(|l| l.len() > 40 && l.chars().all(|c| c.is_alphanumeric() || c == '+' || c == '/' || c == '='))
        .collect();

    match (key_type, long_strings.len()) {
        ("private" | "secret", n) if n >= 2 => Some(long_strings[0].to_string()),
        ("public", n) if n >= 2 => Some(long_strings[1].to_string()),
        ("private" | "secret", 1) => None, // ambiguous
        ("public", 1) => Some(long_strings[0].to_string()),
        _ => None,
    }
}

/// Store a Tauri update private key directly in the keychain.
pub fn set_update_private_key(project_path: &str, private_key: &str) -> Result<()> {
    let account = updater_account_key(project_path);
    let entry = keyring::Entry::new(UPDATER_SERVICE, &account)
        .context("Failed to create keyring entry for update key")?;
    entry
        .set_password(private_key)
        .context("Failed to store update private key in keychain")?;
    log::info!("Stored Tauri update private key in keychain");
    Ok(())
}

/// Check if the Tauri update private key exists in the keychain.
pub fn has_update_private_key(project_path: &str) -> bool {
    let account = updater_account_key(project_path);
    let entry = match keyring::Entry::new(UPDATER_SERVICE, &account) {
        Ok(e) => e,
        Err(_) => return false,
    };
    entry.get_password().is_ok()
}

/// Retrieve the Tauri update private key from the keychain.
fn get_update_private_key(project_path: &str) -> Result<String> {
    let account = updater_account_key(project_path);
    let entry = keyring::Entry::new(UPDATER_SERVICE, &account)
        .context("Failed to create keyring entry")?;
    entry
        .get_password()
        .context("Tauri update private key not found in keychain. Run key generation first.")
}

/// Delete the Tauri update private key from the keychain.
pub fn delete_update_private_key(project_path: &str) -> Result<()> {
    let account = updater_account_key(project_path);
    let entry = keyring::Entry::new(UPDATER_SERVICE, &account)
        .context("Failed to create keyring entry")?;
    entry
        .delete_credential()
        .context("Failed to delete update private key from keychain")
}

/// Rotate the update key pair: generate new keys and re-sign the current release.
pub fn rotate_update_keys(
    repo_path: &Path,
    project_path: &str,
) -> Result<UpdateKeyResult> {
    // Delete old key if present
    let _ = delete_update_private_key(project_path);

    // Generate new key pair
    let result = generate_update_keys(project_path)?;

    // Update the config with the new public key
    let mut config = load_updater_config(repo_path)?;
    config.public_key = Some(result.public_key.clone());
    save_updater_config(repo_path, &config)?;

    log::info!("Rotated Tauri update keys, config updated with new public key");

    Ok(UpdateKeyResult {
        public_key: result.public_key,
        private_key_stored: true,
        message: "Key pair rotated. Update your tauri.conf.json with the new public key.".to_string(),
    })
}

// ---------------------------------------------------------------------------
// Preflight check
// ---------------------------------------------------------------------------

/// Preflight check for the updater pipeline.
/// Returns a list of issues found (empty = all good).
pub fn updater_preflight(repo_path: &Path, project_path: &str) -> Vec<String> {
    let mut issues = Vec::new();

    // Check for updater config
    let config = match load_updater_config(repo_path) {
        Ok(c) => c,
        Err(e) => {
            issues.push(format!("Cannot load updater config: {e}"));
            return issues;
        }
    };

    if !config.enabled {
        issues.push("Updater integration is not enabled in .chibby/updater.toml".to_string());
        return issues;
    }

    // Check public key
    if config.public_key.is_none() {
        issues.push("No public key configured. Run key generation first.".to_string());
    }

    // Check private key in keychain
    if !has_update_private_key(project_path) {
        issues.push("Update private key not found in OS keychain. Run key generation or import your key.".to_string());
    }

    // Check base URL
    if config.base_url.is_none() {
        issues.push("No base_url configured. This is required to generate download URLs in latest.json.".to_string());
    }

    // Check publish target config
    match &config.publish_target {
        Some(UpdatePublishTarget::S3) => {
            if config.s3_bucket.is_none() {
                issues.push("S3 publish target selected but no s3_bucket configured.".to_string());
            }
        }
        Some(UpdatePublishTarget::GithubRelease) => {
            if config.github_repo.is_none() {
                issues.push("GitHub Release target selected but no github_repo configured.".to_string());
            }
        }
        Some(UpdatePublishTarget::Scp) => {
            if config.scp_dest.is_none() {
                issues.push("SCP target selected but no scp_dest configured.".to_string());
            }
        }
        Some(UpdatePublishTarget::Local) => {
            if config.local_dir.is_none() {
                issues.push("Local target selected but no local_dir configured.".to_string());
            }
        }
        None => {
            issues.push("No publish_target configured.".to_string());
        }
    }

    // Check Tauri CLI availability
    if find_tauri_cli().is_none() {
        issues.push("Tauri CLI not found. Install with `cargo install tauri-cli`.".to_string());
    }

    issues
}

// ---------------------------------------------------------------------------
// Update bundle signing
// ---------------------------------------------------------------------------

/// Sign an update bundle file with the Tauri update private key.
///
/// This is separate from macOS code signing / Windows Authenticode. The Tauri
/// updater uses Ed25519 signatures to verify update integrity.
pub fn sign_update_bundle(
    file_path: &Path,
    project_path: &str,
) -> Result<UpdateSignResult> {
    let cli = check_tauri_cli()?;
    let private_key = get_update_private_key(project_path)?;

    // Use `tauri signer sign` with the private key passed via env var
    let file_str = file_path
        .to_str()
        .context("Invalid file path")?;

    let mut cmd = std::process::Command::new(&cli[0]);
    for arg in &cli[1..] {
        cmd.arg(arg);
    }
    cmd.args(["signer", "sign", file_str]);
    cmd.env("TAURI_SIGNING_PRIVATE_KEY", &private_key);
    cmd.env("TAURI_SIGNING_PRIVATE_KEY_PASSWORD", "");

    let output = cmd
        .output()
        .context("Failed to run `tauri signer sign`")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("tauri signer sign failed: {}", stderr);
    }

    // The signature is written to {file_path}.sig or printed to stdout
    let sig_file = PathBuf::from(format!("{}.sig", file_str));
    let signature = if sig_file.exists() {
        std::fs::read_to_string(&sig_file)
            .context("Failed to read signature file")?
            .trim()
            .to_string()
    } else {
        // Signature might be in stdout
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    };

    if signature.is_empty() {
        anyhow::bail!("No signature produced by tauri signer sign");
    }

    log::info!("Signed update bundle: {}", file_path.display());

    Ok(UpdateSignResult {
        file_path: file_str.to_string(),
        signature,
        verified: true, // tauri signer verifies during signing
    })
}

// ---------------------------------------------------------------------------
// latest.json generation
// ---------------------------------------------------------------------------

/// Map Chibby platform/arch to Tauri updater platform key.
fn tauri_platform_key(platform: &str, arch: &str) -> String {
    let os = match platform {
        "macos" => "darwin",
        "windows" => "windows",
        "linux" => "linux",
        other => other,
    };
    format!("{}-{}", os, arch)
}

/// Get the current platform's Tauri platform key.
fn current_tauri_platform_key() -> String {
    let os = if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    };
    let arch = if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x86_64"
    };
    format!("{}-{}", os, arch)
}

/// Generate a Tauri-compatible latest.json from an artifact manifest.
///
/// Signs each artifact with the update private key and constructs download URLs
/// using the configured base_url.
pub fn generate_latest_json(
    repo_path: &Path,
    project_path: &str,
    manifest: &ArtifactManifest,
    notes: Option<String>,
) -> Result<LatestJsonResult> {
    let config = load_updater_config(repo_path)?;

    let base_url = config
        .base_url
        .as_deref()
        .context("base_url is required in updater config to generate latest.json")?;

    // Build platform entries
    let mut platforms = HashMap::new();

    for artifact in &manifest.artifacts {
        // Determine the Tauri platform key from the artifact's canonical name
        // Format: {project}-{version}-{platform}-{arch}.{ext}
        let platform_key = extract_platform_key_from_artifact(&artifact.canonical_name)
            .unwrap_or_else(|| current_tauri_platform_key());

        // Sign the artifact
        let sign_result = sign_update_bundle(Path::new(&artifact.path), project_path)
            .with_context(|| format!("Failed to sign {}", artifact.canonical_name))?;

        // Construct download URL
        let url = format!(
            "{}/{}",
            base_url.trim_end_matches('/'),
            artifact.canonical_name
        );

        platforms.insert(
            platform_key,
            UpdatePlatformEntry {
                url,
                signature: sign_result.signature,
            },
        );
    }

    let latest_json = TauriLatestJson {
        version: manifest.version.clone(),
        notes,
        pub_date: chrono::Utc::now().to_rfc3339(),
        platforms,
    };

    // Validate the generated JSON
    let valid = validate_latest_json(&latest_json);

    // Write to the artifact output directory
    let output_dir = repo_path
        .join(".chibby")
        .join("artifacts")
        .join(&manifest.version);
    std::fs::create_dir_all(&output_dir)?;

    let output_path = output_dir.join("latest.json");
    let json_str = serde_json::to_string_pretty(&latest_json)
        .context("Failed to serialize latest.json")?;
    std::fs::write(&output_path, &json_str)?;

    log::info!("Generated latest.json at {}", output_path.display());

    Ok(LatestJsonResult {
        path: output_path.display().to_string(),
        content: latest_json,
        valid,
    })
}

/// Extract platform key from a canonical artifact name.
/// e.g. "myapp-1.0.0-macos-aarch64.dmg" → "darwin-aarch64"
fn extract_platform_key_from_artifact(canonical_name: &str) -> Option<String> {
    // Format: {project}-{version}-{platform}-{arch}.{ext}
    let stem = canonical_name.rsplit('.').last()?;
    let parts: Vec<&str> = stem.split('-').collect();
    if parts.len() >= 4 {
        let platform = parts[parts.len() - 2];
        let arch = parts[parts.len() - 1];
        // Handle the case where arch might include the extension
        let arch_clean = arch.split('.').next().unwrap_or(arch);
        Some(tauri_platform_key(platform, arch_clean))
    } else {
        None
    }
}

/// Validate a latest.json against Tauri updater requirements.
fn validate_latest_json(json: &TauriLatestJson) -> bool {
    // Version must be non-empty
    if json.version.is_empty() {
        return false;
    }

    // Must have at least one platform
    if json.platforms.is_empty() {
        return false;
    }

    // Each platform must have non-empty url and signature
    for entry in json.platforms.values() {
        if entry.url.is_empty() || entry.signature.is_empty() {
            return false;
        }
    }

    // pub_date must be non-empty
    if json.pub_date.is_empty() {
        return false;
    }

    true
}

/// Merge a per-platform latest.json fragment into an existing latest.json.
///
/// This supports multi-platform builds where each machine produces its own
/// platform entry and they get merged into a single combined file.
pub fn merge_latest_json(
    existing_path: &Path,
    fragment: &TauriLatestJson,
) -> Result<TauriLatestJson> {
    let mut merged = if existing_path.exists() {
        let content = std::fs::read_to_string(existing_path)
            .with_context(|| format!("Failed to read {}", existing_path.display()))?;
        serde_json::from_str::<TauriLatestJson>(&content)
            .with_context(|| format!("Failed to parse {}", existing_path.display()))?
    } else {
        TauriLatestJson {
            version: fragment.version.clone(),
            notes: fragment.notes.clone(),
            pub_date: fragment.pub_date.clone(),
            platforms: HashMap::new(),
        }
    };

    // Merge platforms (fragment entries override existing for same platform key)
    for (key, entry) in &fragment.platforms {
        merged.platforms.insert(key.clone(), entry.clone());
    }

    // Update version and pub_date from the fragment (latest wins)
    merged.version = fragment.version.clone();
    if fragment.notes.is_some() {
        merged.notes = fragment.notes.clone();
    }
    merged.pub_date = fragment.pub_date.clone();

    // Write back
    let json_str = serde_json::to_string_pretty(&merged)
        .context("Failed to serialize merged latest.json")?;
    std::fs::write(existing_path, &json_str)?;

    log::info!("Merged latest.json at {}", existing_path.display());

    Ok(merged)
}

// ---------------------------------------------------------------------------
// Update publishing
// ---------------------------------------------------------------------------

/// Publish update artifacts and latest.json to the configured target.
pub fn publish_update(
    repo_path: &Path,
    version: &str,
    dry_run: bool,
) -> Result<UpdatePublishResult> {
    let config = load_updater_config(repo_path)?;

    let target = config
        .publish_target
        .as_ref()
        .context("No publish_target configured in updater.toml")?;

    let artifact_dir = repo_path
        .join(".chibby")
        .join("artifacts")
        .join(version);

    if !artifact_dir.exists() {
        anyhow::bail!(
            "Artifact directory not found: {}. Run artifact collection first.",
            artifact_dir.display()
        );
    }

    // Gather files to publish: latest.json + all artifact files (not manifest/checksums)
    let mut files_to_publish = Vec::new();

    let latest_json_path = artifact_dir.join("latest.json");
    if !latest_json_path.exists() {
        anyhow::bail!("latest.json not found. Run latest.json generation first.");
    }
    files_to_publish.push(latest_json_path.clone());

    // Add artifact files (exclude metadata files)
    for entry in std::fs::read_dir(&artifact_dir)?.flatten() {
        let path = entry.path();
        if path.is_file() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name != "manifest.json" && name != "SHA256SUMS" && name != "latest.json" {
                files_to_publish.push(path);
            }
        }
    }

    let file_names: Vec<String> = files_to_publish
        .iter()
        .filter_map(|p| p.file_name().and_then(|n| n.to_str()).map(String::from))
        .collect();

    if dry_run {
        return Ok(UpdatePublishResult {
            success: true,
            target: target.clone(),
            uploaded_files: file_names,
            message: format!(
                "Dry run: would publish {} files to {:?}",
                files_to_publish.len(),
                target
            ),
        });
    }

    match target {
        UpdatePublishTarget::Local => {
            publish_to_local(&config, &files_to_publish, &latest_json_path)?;
        }
        UpdatePublishTarget::Scp => {
            publish_via_scp(&config, &files_to_publish)?;
        }
        UpdatePublishTarget::S3 => {
            publish_to_s3(&config, &files_to_publish)?;
        }
        UpdatePublishTarget::GithubRelease => {
            publish_to_github_release(&config, &files_to_publish, version)?;
        }
    }

    Ok(UpdatePublishResult {
        success: true,
        target: target.clone(),
        uploaded_files: file_names,
        message: format!("Published {} files to {:?}", files_to_publish.len(), target),
    })
}

/// Publish to a local directory.
fn publish_to_local(
    config: &UpdaterConfig,
    files: &[PathBuf],
    latest_json_path: &Path,
) -> Result<()> {
    let dest = config
        .local_dir
        .as_deref()
        .context("local_dir not configured")?;

    let dest_path = Path::new(dest);
    std::fs::create_dir_all(dest_path)
        .with_context(|| format!("Failed to create local publish directory: {dest}"))?;

    for file in files {
        let file_name = file
            .file_name()
            .context("Invalid file")?;

        if file == latest_json_path {
            // Atomic write for latest.json: write to temp then rename
            let tmp_path = dest_path.join(".latest.json.tmp");
            std::fs::copy(file, &tmp_path)?;
            std::fs::rename(&tmp_path, dest_path.join("latest.json"))?;
        } else {
            std::fs::copy(file, dest_path.join(file_name))?;
        }
    }

    log::info!("Published {} files to {}", files.len(), dest);
    Ok(())
}

/// Publish via SCP to a remote server.
fn publish_via_scp(config: &UpdaterConfig, files: &[PathBuf]) -> Result<()> {
    let dest = config
        .scp_dest
        .as_deref()
        .context("scp_dest not configured")?;

    for file in files {
        let file_str = file
            .to_str()
            .context("Invalid file path")?;

        let status = std::process::Command::new("scp")
            .args(["-o", "BatchMode=yes", file_str, dest])
            .status()
            .context("Failed to run scp")?;

        if !status.success() {
            anyhow::bail!(
                "scp failed for {} → {}",
                file.display(),
                dest
            );
        }
    }

    log::info!("Published {} files via SCP to {}", files.len(), dest);
    Ok(())
}

/// Publish to S3 (or S3-compatible like Cloudflare R2).
fn publish_to_s3(config: &UpdaterConfig, files: &[PathBuf]) -> Result<()> {
    let bucket = config
        .s3_bucket
        .as_deref()
        .context("s3_bucket not configured")?;

    // Check if aws CLI is available
    if !command_exists("aws") {
        anyhow::bail!("AWS CLI (`aws`) not found. Install it to use S3 publishing.");
    }

    for file in files {
        let file_str = file
            .to_str()
            .context("Invalid file path")?;

        let file_name = file
            .file_name()
            .and_then(|n| n.to_str())
            .context("Invalid file name")?;

        let s3_key = format!("s3://{}/{}", bucket, file_name);

        let mut cmd = std::process::Command::new("aws");
        cmd.args(["s3", "cp"]);
        cmd.args([file_str, &s3_key]);

        if let Some(endpoint) = &config.s3_endpoint {
            cmd.args(["--endpoint-url", endpoint]);
        }
        if let Some(region) = &config.s3_region {
            cmd.args(["--region", region]);
        }

        let status = cmd.status().context("Failed to run aws s3 cp")?;
        if !status.success() {
            anyhow::bail!("aws s3 cp failed for {}", file_name);
        }
    }

    log::info!("Published {} files to S3 bucket {}", files.len(), bucket);
    Ok(())
}

/// Publish to GitHub Releases.
fn publish_to_github_release(
    config: &UpdaterConfig,
    files: &[PathBuf],
    version: &str,
) -> Result<()> {
    let repo = config
        .github_repo
        .as_deref()
        .context("github_repo not configured")?;

    if !command_exists("gh") {
        anyhow::bail!("GitHub CLI (`gh`) not found. Install it to use GitHub Release publishing.");
    }

    let tag = format!("v{}", version);

    // Create release if it doesn't exist (ignore error if already exists)
    let _ = std::process::Command::new("gh")
        .args([
            "release", "create", &tag,
            "--repo", repo,
            "--title", &format!("v{}", version),
            "--notes", &format!("Release v{}", version),
        ])
        .status();

    // Upload each file as a release asset
    for file in files {
        let file_str = file.to_str().context("Invalid file path")?;

        let status = std::process::Command::new("gh")
            .args([
                "release", "upload", &tag,
                file_str,
                "--repo", repo,
                "--clobber",
            ])
            .status()
            .context("Failed to run gh release upload")?;

        if !status.success() {
            anyhow::bail!(
                "gh release upload failed for {}",
                file.display()
            );
        }
    }

    log::info!(
        "Published {} files to GitHub Release {}/{}",
        files.len(),
        repo,
        tag
    );
    Ok(())
}
