//! Deploy pipeline and default-environment generation.

#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use crate::engine::models::{
    Backend, DeploymentConfig, DeploymentMethod, Environment, EnvironmentsConfig, FileConflict,
    HealthCheck, Pipeline, PipelineValidation, PipelineWarning, Stage, WarningSeverity,
};
use std::collections::HashMap;
use std::path::Path;

/// Generate a deployment pipeline based on the deployment method.
pub fn generate_deployment_pipeline(
    repo_name: &str,
    deploy_config: &DeploymentConfig,
    repo_path: &Path,
) -> Option<Pipeline> {
    let mut stages = Vec::new();

    match deploy_config.method {
        DeploymentMethod::DockerComposeSsh => {
            stages.push(local_stage("docker-build", vec!["docker compose build"]));

            let compose_file = deploy_config
                .compose_file
                .as_deref()
                .unwrap_or("docker-compose.yml");
            let compose_cmd =
                if compose_file != "docker-compose.yml" && compose_file != "compose.yml" {
                    format!(
                        "docker compose -f {} pull && docker compose -f {} up -d --remove-orphans",
                        compose_file, compose_file
                    )
                } else {
                    "docker compose pull && docker compose up -d --remove-orphans".to_string()
                };

            stages.push(Stage {
                name: "deploy".to_string(),
                commands: vec![compose_cmd],
                backend: Backend::Ssh,
                working_dir: None,
                fail_fast: true,
                health_check: None,
            });

            // Add health check if configured
            if let Some(health_url) = &deploy_config.health_check_url {
                stages.push(Stage {
                    name: "health-check".to_string(),
                    commands: vec![format!("curl -sf http://localhost{} || exit 1", health_url)],
                    backend: Backend::Ssh,
                    working_dir: None,
                    fail_fast: true,
                    health_check: Some(HealthCheck {
                        command: format!("curl -sf http://localhost{}", health_url),
                        retries: 5,
                        delay_secs: 10,
                    }),
                });
            }
        }

        DeploymentMethod::DockerRegistry => {
            let registry = deploy_config
                .docker_registry
                .as_deref()
                .unwrap_or("ghcr.io");
            let image_name = format!("{}/{}", registry, repo_name.to_lowercase());

            stages.push(local_stage(
                "docker-build",
                vec![
                    &format!("docker build -t {} .", image_name),
                    &format!("docker push {}", image_name),
                ],
            ));

            stages.push(Stage {
                name: "deploy".to_string(),
                commands: vec![
                    format!("docker pull {}", image_name),
                    format!("docker stop {} || true", repo_name),
                    format!("docker rm {} || true", repo_name),
                    format!("docker run -d --name {} {}", repo_name, image_name),
                ],
                backend: Backend::Ssh,
                working_dir: None,
                fail_fast: true,
                health_check: None,
            });
        }

        DeploymentMethod::CargoPublish => {
            let mut commands = Vec::new();
            if deploy_config.dry_run_first {
                commands.push("cargo publish --dry-run".to_string());
            }
            commands.push("cargo publish".to_string());

            stages.push(Stage {
                name: "cargo-publish".to_string(),
                commands,
                backend: Backend::Local,
                working_dir: None,
                fail_fast: true,
                health_check: None,
            });
        }

        DeploymentMethod::NpmPublish => {
            let mut commands = Vec::new();
            if deploy_config.dry_run_first {
                commands.push("npm publish --dry-run".to_string());
            }
            commands.push("npm publish".to_string());

            stages.push(Stage {
                name: "npm-publish".to_string(),
                commands,
                backend: Backend::Local,
                working_dir: None,
                fail_fast: true,
                health_check: None,
            });
        }

        DeploymentMethod::GithubRelease => {
            // Try to detect version source
            let version_cmd = detect_version_command(repo_path);

            stages.push(Stage {
                name: "github-release".to_string(),
                commands: vec![format!(
                    "gh release create v$({}) --generate-notes --draft ./dist/*",
                    version_cmd
                )],
                backend: Backend::Local,
                working_dir: None,
                fail_fast: true,
                health_check: None,
            });
        }

        DeploymentMethod::SshRsync => {
            let ssh_host = deploy_config.ssh_host.as_deref().unwrap_or("{{ssh_host}}");

            stages.push(local_stage(
                "deploy",
                vec![&format!("rsync -avz --delete ./dist/ {}:~/app/", ssh_host)],
            ));

            stages.push(Stage {
                name: "restart".to_string(),
                commands: vec!["systemctl --user restart myapp || pm2 restart all".to_string()],
                backend: Backend::Ssh,
                working_dir: None,
                fail_fast: true,
                health_check: None,
            });
        }

        DeploymentMethod::Flyio => {
            stages.push(local_stage("deploy", vec!["fly deploy"]));

            if let Some(health_url) = &deploy_config.health_check_url {
                let app_name = deploy_config
                    .platform_project
                    .as_deref()
                    .unwrap_or(repo_name);
                stages.push(local_stage(
                    "health-check",
                    vec![&format!(
                        "curl -sf https://{}.fly.dev{} || exit 1",
                        app_name, health_url
                    )],
                ));
            }
        }

        DeploymentMethod::Render => {
            // Render typically auto-deploys on push, but we can trigger via API
            stages.push(local_stage(
                "deploy",
                vec![
                    "echo 'Render deploys automatically on push. Manual trigger:'",
                    "# curl -X POST https://api.render.com/deploy/srv-xxx?key=xxx",
                ],
            ));
        }

        DeploymentMethod::Railway => {
            stages.push(local_stage("deploy", vec!["railway up"]));
        }

        DeploymentMethod::Netlify => {
            // Detect build output directory
            let dist_dir = detect_dist_directory(repo_path);
            stages.push(local_stage(
                "deploy",
                vec![&format!("netlify deploy --prod --dir={}", dist_dir)],
            ));
        }

        DeploymentMethod::Vercel => {
            stages.push(local_stage("deploy", vec!["vercel --prod"]));
        }

        DeploymentMethod::S3Static => {
            let dist_dir = detect_dist_directory(repo_path);
            let bucket = deploy_config
                .platform_project
                .as_deref()
                .unwrap_or("{{s3_bucket}}");

            stages.push(local_stage(
                "deploy",
                vec![&format!(
                    "aws s3 sync ./{} s3://{} --delete",
                    dist_dir, bucket
                )],
            ));

            // Optional CloudFront invalidation
            stages.push(local_stage("invalidate-cache", vec![
                "# aws cloudfront create-invalidation --distribution-id {{distribution_id}} --paths '/*'",
            ]));
        }

        DeploymentMethod::AutoDetect => {
            // Parse GitHub Actions deploy workflows and convert to stages
            let workflows = parse_github_workflows(repo_path);
            let deploy_workflows: Vec<_> = workflows
                .iter()
                .filter(|w| {
                    let name_lower = w.name.to_lowercase();
                    let file_lower = w.file_path.to_lowercase();
                    name_lower.contains("deploy")
                        || name_lower.contains("release")
                        || name_lower.contains("publish")
                        || file_lower.contains("deploy")
                        || file_lower.contains("release")
                        || file_lower.contains("publish")
                })
                .cloned()
                .collect();

            if !deploy_workflows.is_empty() {
                let workflow_stages = workflows_to_stages(&deploy_workflows);
                for stage in workflow_stages {
                    let is_ssh_stage = stage.commands.iter().any(|cmd| {
                        cmd.contains("ssh ") || cmd.contains("rsync") || cmd.contains("scp ")
                    });

                    stages.push(Stage {
                        name: stage.name,
                        commands: stage.commands,
                        backend: if is_ssh_stage {
                            Backend::Ssh
                        } else {
                            Backend::Local
                        },
                        working_dir: stage.working_dir,
                        fail_fast: stage.fail_fast,
                        health_check: stage.health_check,
                    });
                }
            }

            // If no stages from workflows, fall back to Docker if available
            if stages.is_empty() && has_any_docker_compose(repo_path) {
                stages.push(local_stage("docker-build", vec!["docker compose build"]));
                stages.push(Stage {
                    name: "deploy".to_string(),
                    commands: vec![
                        "docker compose pull".to_string(),
                        "docker compose up -d --remove-orphans".to_string(),
                    ],
                    backend: Backend::Ssh,
                    working_dir: None,
                    fail_fast: true,
                    health_check: None,
                });
            }
        }

        DeploymentMethod::Skip => {
            return None;
        }
    }

    if stages.is_empty() {
        return None;
    }

    Some(Pipeline {
        name: format!("{} Deploy", repo_name),
        stages,
    })
}

/// Generate default environments based on deployment method.
///
/// SSH-based deployments get production + staging environments.
/// PaaS deployments get production only.
/// Package publishing (Cargo, npm) and GitHub releases don't need environments.
pub fn generate_default_environments(
    deploy_config: &DeploymentConfig,
) -> Option<EnvironmentsConfig> {
    let envs = match deploy_config.method {
        // SSH-based deployments: production + staging
        DeploymentMethod::DockerComposeSsh
        | DeploymentMethod::DockerRegistry
        | DeploymentMethod::SshRsync => {
            vec![
                Environment {
                    name: "production".to_string(),
                    ssh_host: deploy_config.ssh_host.clone(),
                    ssh_port: None,
                    variables: HashMap::new(),
                },
                Environment {
                    name: "staging".to_string(),
                    ssh_host: None, // User fills in later
                    ssh_port: None,
                    variables: HashMap::new(),
                },
            ]
        }

        // PaaS deployments: production only
        DeploymentMethod::Flyio
        | DeploymentMethod::Render
        | DeploymentMethod::Railway
        | DeploymentMethod::Vercel
        | DeploymentMethod::Netlify
        | DeploymentMethod::S3Static => {
            vec![Environment {
                name: "production".to_string(),
                ssh_host: None,
                ssh_port: None,
                variables: HashMap::new(),
            }]
        }

        // No environments needed for these
        DeploymentMethod::CargoPublish
        | DeploymentMethod::NpmPublish
        | DeploymentMethod::GithubRelease
        | DeploymentMethod::AutoDetect
        | DeploymentMethod::Skip => {
            return None;
        }
    };

    Some(EnvironmentsConfig { environments: envs })
}

/// Detect the command to get version from project files.
fn detect_version_command(repo_path: &Path) -> String {
    // Check for VERSION file
    if repo_path.join("VERSION").exists() {
        return "cat VERSION".to_string();
    }

    // Check for Cargo.toml
    if repo_path.join("Cargo.toml").exists() {
        return "cargo metadata --format-version 1 --no-deps | jq -r '.packages[0].version'"
            .to_string();
    }

    // Check for package.json
    if repo_path.join("package.json").exists() {
        return "node -p \"require('./package.json').version\"".to_string();
    }

    // Check for pyproject.toml
    if repo_path.join("pyproject.toml").exists() {
        return "grep -m1 'version' pyproject.toml | cut -d'\"' -f2".to_string();
    }

    // Fallback
    "cat VERSION".to_string()
}

/// Detect the build output directory for static sites.
fn detect_dist_directory(repo_path: &Path) -> String {
    // Common output directories
    for dir in &["dist", "build", "out", "public", "_site", ".next/out"] {
        if repo_path.join(dir).is_dir() {
            return dir.to_string();
        }
    }

    // Check netlify.toml for publish directory
    if let Ok(content) = std::fs::read_to_string(repo_path.join("netlify.toml")) {
        for line in content.lines() {
            if line.trim().starts_with("publish") {
                if let Some(value) = line.split('=').nth(1) {
                    return value.trim().trim_matches('"').to_string();
                }
            }
        }
    }

    // Default
    "dist".to_string()
}

// ---------------------------------------------------------------------------
// CI Workflow Parsing (GitHub Actions, etc.)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_generate_default_environments_ssh_deploys() {
        // SSH-based deployments should create production + staging
        for method in [
            DeploymentMethod::DockerComposeSsh,
            DeploymentMethod::DockerRegistry,
            DeploymentMethod::SshRsync,
        ] {
            let config = DeploymentConfig {
                method,
                ssh_host: Some("user@example.com".to_string()),
                ..Default::default()
            };
            let result = generate_default_environments(&config);
            assert!(
                result.is_some(),
                "Expected environments for {:?}",
                config.method
            );
            let envs = result.unwrap();
            assert_eq!(envs.environments.len(), 2);
            assert_eq!(envs.environments[0].name, "production");
            assert_eq!(
                envs.environments[0].ssh_host,
                Some("user@example.com".to_string())
            );
            assert_eq!(envs.environments[1].name, "staging");
            assert!(envs.environments[1].ssh_host.is_none());
        }
    }

    #[test]
    fn test_generate_default_environments_paas() {
        // PaaS deployments should create production only
        for method in [
            DeploymentMethod::Flyio,
            DeploymentMethod::Render,
            DeploymentMethod::Railway,
            DeploymentMethod::Vercel,
            DeploymentMethod::Netlify,
            DeploymentMethod::S3Static,
        ] {
            let config = DeploymentConfig {
                method,
                ..Default::default()
            };
            let result = generate_default_environments(&config);
            assert!(
                result.is_some(),
                "Expected environments for {:?}",
                config.method
            );
            let envs = result.unwrap();
            assert_eq!(envs.environments.len(), 1);
            assert_eq!(envs.environments[0].name, "production");
            assert!(envs.environments[0].ssh_host.is_none());
        }
    }

    #[test]
    fn test_generate_default_environments_no_envs() {
        // These methods should not create any environments
        for method in [
            DeploymentMethod::CargoPublish,
            DeploymentMethod::NpmPublish,
            DeploymentMethod::GithubRelease,
            DeploymentMethod::AutoDetect,
            DeploymentMethod::Skip,
        ] {
            let config = DeploymentConfig {
                method,
                ..Default::default()
            };
            let result = generate_default_environments(&config);
            assert!(
                result.is_none(),
                "Expected no environments for {:?}",
                config.method
            );
        }
    }
}
