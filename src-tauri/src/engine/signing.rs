use crate::engine::models::{SigningConfig, SigningPlatform, SigningResult};
use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

// ---------------------------------------------------------------------------
// Signing config persistence (.chibby/signing.toml)
// ---------------------------------------------------------------------------

/// Save signing config to .chibby/signing.toml.
pub fn save_signing_config(repo_path: &Path, config: &SigningConfig) -> Result<()> {
    let chibby_dir = repo_path.join(".chibby");
    std::fs::create_dir_all(&chibby_dir)?;

    let toml_str = toml::to_string_pretty(config)
        .context("Failed to serialize signing config")?;

    let file_path = chibby_dir.join("signing.toml");
    std::fs::write(&file_path, &toml_str)
        .with_context(|| format!("Failed to write {}", file_path.display()))?;

    log::info!("Saved signing config to {}", file_path.display());
    Ok(())
}

/// Load signing config from .chibby/signing.toml.
pub fn load_signing_config(repo_path: &Path) -> Result<SigningConfig> {
    let file_path = repo_path.join(".chibby").join("signing.toml");
    if !file_path.exists() {
        return Ok(SigningConfig::default());
    }
    let content = std::fs::read_to_string(&file_path)
        .with_context(|| format!("Failed to read {}", file_path.display()))?;

    let config: SigningConfig = toml::from_str(&content)
        .with_context(|| format!("Failed to parse {}", file_path.display()))?;

    Ok(config)
}

// ---------------------------------------------------------------------------
// Code signing operations
// ---------------------------------------------------------------------------

/// Detect which platform we're running on.
pub fn detect_platform() -> SigningPlatform {
    if cfg!(target_os = "macos") {
        SigningPlatform::Macos
    } else if cfg!(target_os = "windows") {
        SigningPlatform::Windows
    } else {
        SigningPlatform::Linux
    }
}

/// Sign an artifact using the appropriate platform tool.
pub fn sign_artifact(
    artifact_path: &Path,
    config: &SigningConfig,
) -> Result<SigningResult> {
    if !config.enabled {
        return Ok(SigningResult {
            success: true,
            platform: detect_platform(),
            artifact_path: artifact_path.display().to_string(),
            notarized: false,
            message: "Signing disabled — artifact is unsigned".to_string(),
        });
    }

    let platform = detect_platform();
    match platform {
        SigningPlatform::Macos => sign_macos(artifact_path, config),
        SigningPlatform::Windows => sign_windows(artifact_path, config),
        SigningPlatform::Linux => sign_linux(artifact_path, config),
    }
}

/// macOS: codesign + notarytool.
fn sign_macos(artifact_path: &Path, config: &SigningConfig) -> Result<SigningResult> {
    let identity = config
        .macos_identity
        .as_deref()
        .context("macOS signing identity not configured (macos_identity)")?;

    // Step 1: codesign
    let output = Command::new("codesign")
        .args([
            "--force",
            "--options", "runtime",
            "--sign", identity,
            &artifact_path.display().to_string(),
        ])
        .output()
        .context("Failed to run codesign — is Xcode Command Line Tools installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Ok(SigningResult {
            success: false,
            platform: SigningPlatform::Macos,
            artifact_path: artifact_path.display().to_string(),
            notarized: false,
            message: format!("codesign failed: {stderr}"),
        });
    }

    log::info!("Signed {}", artifact_path.display());

    // Step 2: notarize if team ID and bundle ID are set
    let notarized = if config.macos_team_id.is_some() && config.macos_bundle_id.is_some() {
        match notarize_macos(artifact_path, config) {
            Ok(()) => true,
            Err(e) => {
                log::warn!("Notarization failed: {e}");
                false
            }
        }
    } else {
        false
    };

    Ok(SigningResult {
        success: true,
        platform: SigningPlatform::Macos,
        artifact_path: artifact_path.display().to_string(),
        notarized,
        message: if notarized {
            "Signed and notarized".to_string()
        } else {
            "Signed (not notarized — set macos_team_id and macos_bundle_id to enable)".to_string()
        },
    })
}

/// macOS notarization via notarytool.
fn notarize_macos(artifact_path: &Path, config: &SigningConfig) -> Result<()> {
    let team_id = config.macos_team_id.as_deref().unwrap();

    // notarytool requires the artifact to be in a zip or dmg
    let ext = artifact_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    let submit_path = if ext == "dmg" || ext == "zip" || ext == "pkg" {
        artifact_path.to_path_buf()
    } else {
        // Create a temporary zip for notarization
        let zip_path = artifact_path.with_extension("zip");
        let output = Command::new("ditto")
            .args([
                "-c", "-k", "--keepParent",
                &artifact_path.display().to_string(),
                &zip_path.display().to_string(),
            ])
            .output()
            .context("Failed to zip artifact for notarization")?;
        if !output.status.success() {
            bail!("ditto failed: {}", String::from_utf8_lossy(&output.stderr));
        }
        zip_path
    };

    let output = Command::new("xcrun")
        .args([
            "notarytool", "submit",
            &submit_path.display().to_string(),
            "--team-id", team_id,
            "--keychain-profile", "chibby-notarize",
            "--wait",
        ])
        .output()
        .context("Failed to run notarytool — have you stored credentials with 'xcrun notarytool store-credentials chibby-notarize'?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("notarytool submit failed: {stderr}");
    }

    // Staple the notarization ticket
    if ext == "dmg" || ext == "pkg" || ext == "app" {
        let _ = Command::new("xcrun")
            .args(["stapler", "staple", &artifact_path.display().to_string()])
            .output();
    }

    log::info!("Notarized {}", artifact_path.display());
    Ok(())
}

/// Windows: signtool.
fn sign_windows(artifact_path: &Path, config: &SigningConfig) -> Result<SigningResult> {
    let cert_path = config
        .windows_cert_path
        .as_deref()
        .context("Windows certificate path not configured (windows_cert_path)")?;

    let output = Command::new("signtool")
        .args([
            "sign",
            "/f", cert_path,
            "/fd", "SHA256",
            "/tr", "http://timestamp.digicert.com",
            "/td", "SHA256",
            &artifact_path.display().to_string(),
        ])
        .output()
        .context("Failed to run signtool — is Windows SDK installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Ok(SigningResult {
            success: false,
            platform: SigningPlatform::Windows,
            artifact_path: artifact_path.display().to_string(),
            notarized: false,
            message: format!("signtool failed: {stderr}"),
        });
    }

    Ok(SigningResult {
        success: true,
        platform: SigningPlatform::Windows,
        artifact_path: artifact_path.display().to_string(),
        notarized: false,
        message: "Signed with Authenticode".to_string(),
    })
}

/// Linux: gpg --detach-sign.
fn sign_linux(artifact_path: &Path, config: &SigningConfig) -> Result<SigningResult> {
    let key_id = config
        .linux_gpg_key
        .as_deref()
        .context("Linux GPG key ID not configured (linux_gpg_key)")?;

    let output = Command::new("gpg")
        .args([
            "--detach-sign",
            "--armor",
            "--local-user", key_id,
            &artifact_path.display().to_string(),
        ])
        .output()
        .context("Failed to run gpg — is GnuPG installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Ok(SigningResult {
            success: false,
            platform: SigningPlatform::Linux,
            artifact_path: artifact_path.display().to_string(),
            notarized: false,
            message: format!("gpg signing failed: {stderr}"),
        });
    }

    Ok(SigningResult {
        success: true,
        platform: SigningPlatform::Linux,
        artifact_path: artifact_path.display().to_string(),
        notarized: false,
        message: format!(
            "GPG signed — signature at {}.asc",
            artifact_path.display()
        ),
    })
}

/// Check whether the current platform has signing tools available.
pub fn check_signing_tools() -> Vec<String> {
    let mut issues = Vec::new();
    let platform = detect_platform();

    match platform {
        SigningPlatform::Macos => {
            if Command::new("codesign").arg("--version").output().is_err() {
                issues.push("codesign not found — install Xcode Command Line Tools".to_string());
            }
            if Command::new("xcrun")
                .args(["notarytool", "--version"])
                .output()
                .is_err()
            {
                issues.push("xcrun notarytool not found — requires Xcode 13+".to_string());
            }
        }
        SigningPlatform::Windows => {
            if Command::new("signtool").arg("/?").output().is_err() {
                issues.push("signtool not found — install Windows SDK".to_string());
            }
        }
        SigningPlatform::Linux => {
            if Command::new("gpg").arg("--version").output().is_err() {
                issues.push("gpg not found — install GnuPG".to_string());
            }
        }
    }

    issues
}
