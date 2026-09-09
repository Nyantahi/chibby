//! `chibby env` and `chibby secrets` subcommands (environments, secrets, env vars).

use crate::args::{EnvCmd, EnvVarsCmd, SecretsCmd};
use crate::cli::{self, icons, Printer};
use chibby_lib::engine::models::{Environment, SecretRef};
use chibby_lib::engine::secret_audit::{self as secret_audit_engine, Provenance};
use chibby_lib::engine::{pipeline, preflight, secrets as secrets_engine};
use owo_colors::OwoColorize;

pub(crate) async fn handle_secrets(printer: &Printer, cmd: &SecretsCmd) -> anyhow::Result<()> {
    match cmd {
        SecretsCmd::List { project } => {
            let path = crate::project_path(project.as_ref());
            let config = pipeline::load_secrets_config(&path)?;
            printer.header(&format!("{} Declared Secrets", icons::LOCK));
            if config.secrets.is_empty() {
                printer.info("No secrets declared. Run `chibby secrets add <NAME>` to add one.");
                return Ok(());
            }
            for s in &config.secrets {
                let scope = if s.environments.is_empty() {
                    "all environments".to_string()
                } else {
                    s.environments.join(", ")
                };
                printer.kv(&s.name, &scope);
            }
        }

        SecretsCmd::Add { name, env, project } => {
            let path = crate::project_path(project.as_ref());
            pipeline::add_secret_ref(
                &path,
                SecretRef {
                    name: name.clone(),
                    environments: env.clone(),
                },
            )?;
            let scope = if env.is_empty() {
                "all environments".to_string()
            } else {
                env.join(", ")
            };
            printer.success(&format!("Added secret reference '{}' ({})", name, scope));
            printer.info(&format!(
                "Set the value with: chibby secrets set {} --env <env>",
                name
            ));
        }

        SecretsCmd::Remove { name, project } => {
            let path = crate::project_path(project.as_ref());
            pipeline::remove_secret_ref(&path, name)?;
            printer.success(&format!("Removed secret reference '{}'", name));
            printer.warn(
                "Stored keychain values for this secret were NOT deleted. \
                 Use `chibby secrets delete` per environment to remove them.",
            );
        }

        SecretsCmd::Set {
            name,
            env,
            value,
            project,
        } => {
            let path = crate::project_path(project.as_ref());
            let path_str = path.to_string_lossy().to_string();
            let value = match value {
                Some(v) => v.clone(),
                None => rpassword::prompt_password(format!("Value for {} ({}): ", name, env))?,
            };
            if value.is_empty() {
                anyhow::bail!("Secret value cannot be empty");
            }
            secrets_engine::set_secret(&path_str, env, name, &value)?;
            secret_audit_engine::record_set_quietly(&path_str, env, name, Provenance::Cli);
            printer.success(&format!(
                "Saved '{}' to OS keychain for env '{}'",
                name, env
            ));
        }

        SecretsCmd::Rotate { name, env, project } => {
            let path = crate::project_path(project.as_ref());
            let path_str = path.to_string_lossy().to_string();
            let value = rpassword::prompt_password(format!("New value for {} ({}): ", name, env))?;
            if value.is_empty() {
                anyhow::bail!("Secret value cannot be empty");
            }
            secrets_engine::set_secret(&path_str, env, name, &value)?;
            secret_audit_engine::record_set_quietly(&path_str, env, name, Provenance::Cli);
            printer.success(&format!("Rotated '{}' for env '{}'", name, env));
        }

        SecretsCmd::Delete { name, env, project } => {
            let path = crate::project_path(project.as_ref());
            let path_str = path.to_string_lossy().to_string();
            secrets_engine::delete_secret(&path_str, env, name)?;
            secret_audit_engine::record_delete_quietly(&path_str, env, name, Provenance::Cli);
            printer.success(&format!(
                "Deleted '{}' from keychain for env '{}'",
                name, env
            ));
        }

        SecretsCmd::Status { env, project } => {
            let path = crate::project_path(project.as_ref());
            let path_str = path.to_string_lossy().to_string();
            let secrets_config = pipeline::load_secrets_config(&path)?;
            let envs_config = pipeline::load_environments_layered(&path)?;
            printer.header(&format!("{} Secret Status", icons::LOCK));

            if secrets_config.secrets.is_empty() {
                printer.info("No secrets declared.");
                return Ok(());
            }

            let envs_to_check: Vec<String> = match env {
                Some(e) => vec![e.clone()],
                None => {
                    let mut names: Vec<String> = secrets_config
                        .secrets
                        .iter()
                        .flat_map(|s| s.environments.iter().cloned())
                        .chain(envs_config.environments.iter().map(|e| e.name.clone()))
                        .collect();
                    names.sort();
                    names.dedup();
                    if names.is_empty() {
                        names.push("default".to_string());
                    }
                    names
                }
            };

            for env_name in envs_to_check {
                printer.subheader(&format!("Environment: {}", env_name));
                let statuses =
                    secrets_engine::check_secrets_status(&path_str, &env_name, &secrets_config);
                for s in statuses {
                    printer.secret(&s.name, s.is_set);
                }
                printer.newline();
            }
        }
    }
    Ok(())
}

pub(crate) async fn handle_env(printer: &Printer, cmd: &EnvCmd) -> anyhow::Result<()> {
    match cmd {
        EnvCmd::List { project } => {
            let path = crate::project_path(project.as_ref());
            let config = pipeline::load_environments_layered(&path)?;
            printer.header(&format!("{} Environments", icons::GEAR));
            if config.environments.is_empty() {
                printer.info("No environments defined. Run `chibby env add <NAME>` to add one.");
                return Ok(());
            }
            for env in &config.environments {
                println!("  {} {}", icons::SUCCESS.green(), env.name.white().bold());
                if let Some(host) = &env.ssh_host {
                    printer.kv("Host", host);
                }
                if let Some(port) = env.ssh_port {
                    printer.kv("Port", &port.to_string());
                }
                printer.kv("Variables", &env.variables.len().to_string());
                printer.newline();
            }
        }

        EnvCmd::Show { name, project } => {
            let path = crate::project_path(project.as_ref());
            let config = pipeline::load_environments_layered(&path)?;
            let env = config
                .environments
                .iter()
                .find(|e| e.name == *name)
                .ok_or_else(|| anyhow::anyhow!("Environment '{}' not found", name))?;
            printer.header(&format!("{} Environment: {}", icons::GEAR, name));
            printer.kv(
                "Host",
                env.ssh_host.as_deref().unwrap_or("(none — local only)"),
            );
            printer.kv(
                "Port",
                &env.ssh_port.map(|p| p.to_string()).unwrap_or_default(),
            );
            if !env.variables.is_empty() {
                printer.subheader("Variables");
                let mut keys: Vec<&String> = env.variables.keys().collect();
                keys.sort();
                for k in keys {
                    printer.kv(k, &env.variables[k]);
                }
            }
        }

        EnvCmd::Add {
            name,
            ssh_host,
            ssh_port,
            project,
        } => {
            let path = crate::project_path(project.as_ref());
            pipeline::add_environment(
                &path,
                Environment {
                    name: name.clone(),
                    ssh_host: ssh_host.clone(),
                    ssh_port: *ssh_port,
                    variables: Default::default(),
                },
            )?;
            printer.success(&format!("Added environment '{}'", name));
        }

        EnvCmd::Remove { name, project } => {
            let path = crate::project_path(project.as_ref());
            pipeline::remove_environment(&path, name)?;
            printer.success(&format!("Removed environment '{}'", name));
        }

        EnvCmd::Edit { project } => {
            let path = crate::project_path(project.as_ref());
            let file = path.join(".chibby").join("environments.toml");
            if !file.exists() {
                std::fs::create_dir_all(path.join(".chibby"))?;
                std::fs::write(&file, "")?;
            }
            let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
            let status = std::process::Command::new(&editor).arg(&file).status()?;
            if !status.success() {
                anyhow::bail!("Editor '{}' exited non-zero", editor);
            }
            // Validate after edit
            pipeline::load_environments(&path)
                .map_err(|e| anyhow::anyhow!("environments.toml is invalid after edit: {e}"))?;
            printer.success(&format!("Saved {}", file.display()));
        }

        EnvCmd::Copy { from, to, project } => {
            let path = crate::project_path(project.as_ref());
            let mut config = pipeline::load_environments(&path)?;
            let src = config
                .environments
                .iter()
                .find(|e| e.name == *from)
                .ok_or_else(|| anyhow::anyhow!("Source environment '{}' not found", from))?
                .clone();
            if config.environments.iter().any(|e| e.name == *to) {
                anyhow::bail!("Destination environment '{}' already exists", to);
            }
            config.environments.push(Environment {
                name: to.clone(),
                ..src
            });
            pipeline::save_environments(&path, &config)?;
            printer.success(&format!("Copied '{}' -> '{}'", from, to));
        }

        EnvCmd::Test { name, project } => {
            let path = crate::project_path(project.as_ref());
            let config = pipeline::load_environments_layered(&path)?;
            let env = config
                .environments
                .iter()
                .find(|e| e.name == *name)
                .ok_or_else(|| anyhow::anyhow!("Environment '{}' not found", name))?;
            let host = env
                .ssh_host
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Environment '{}' has no ssh_host", name))?;
            let spin = cli::spinner(&format!("Testing SSH to {}...", host));
            let result = preflight::test_ssh_connectivity(host, env.ssh_port).await;
            spin.finish_and_clear();
            match result {
                Ok(msg) => printer.success(&msg),
                Err(e) => {
                    printer.error(&format!("SSH test failed: {e}"));
                    return Err(e.into());
                }
            }
        }

        EnvCmd::Vars(vars_cmd) => return handle_env_vars(printer, vars_cmd).await,

        EnvCmd::Diff { from, to, project } => {
            let path = crate::project_path(project.as_ref());
            let config = pipeline::load_environments_layered(&path)?;
            let a = config
                .environments
                .iter()
                .find(|e| e.name == *from)
                .ok_or_else(|| anyhow::anyhow!("Environment '{}' not found", from))?;
            let b = config
                .environments
                .iter()
                .find(|e| e.name == *to)
                .ok_or_else(|| anyhow::anyhow!("Environment '{}' not found", to))?;

            printer.header(&format!("{} Env diff: {} vs {}", icons::GEAR, from, to));
            printer.newline();
            printer.subheader("Variables");

            let mut keys: std::collections::BTreeSet<&String> =
                a.variables.keys().chain(b.variables.keys()).collect();
            let mut any_var_diff = false;
            for k in keys.iter() {
                let av = a.variables.get(*k);
                let bv = b.variables.get(*k);
                match (av, bv) {
                    (Some(_), None) => {
                        any_var_diff = true;
                        printer.kv(&format!("- {}", k), &format!("only in {}", from));
                    }
                    (None, Some(_)) => {
                        any_var_diff = true;
                        printer.kv(&format!("+ {}", k), &format!("only in {}", to));
                    }
                    (Some(va), Some(vb)) if va != vb => {
                        any_var_diff = true;
                        printer.kv(&format!("~ {}", k), &format!("{} | {}", va, vb));
                    }
                    _ => {}
                }
            }
            if !any_var_diff {
                printer.info("Variables identical.");
            }
            keys.clear();

            printer.newline();
            printer.subheader("Secrets");
            let secrets_config = pipeline::load_secrets_config(&path)?;
            let in_a: std::collections::BTreeSet<&str> = secrets_config
                .secrets
                .iter()
                .filter(|s| s.environments.is_empty() || s.environments.iter().any(|e| e == from))
                .map(|s| s.name.as_str())
                .collect();
            let in_b: std::collections::BTreeSet<&str> = secrets_config
                .secrets
                .iter()
                .filter(|s| s.environments.is_empty() || s.environments.iter().any(|e| e == to))
                .map(|s| s.name.as_str())
                .collect();
            let mut any_secret_diff = false;
            for name in in_a.difference(&in_b) {
                any_secret_diff = true;
                printer.kv(&format!("- {}", name), &format!("only in {}", from));
            }
            for name in in_b.difference(&in_a) {
                any_secret_diff = true;
                printer.kv(&format!("+ {}", name), &format!("only in {}", to));
            }
            if !any_secret_diff {
                printer.info("Secret references identical.");
            }
        }

        EnvCmd::ScanLeaks { project } => {
            let path = crate::project_path(project.as_ref());
            let hits = pipeline::scan_environments_for_leaks(&path)?;
            printer.header(&format!("{} Leak scan", icons::LOCK));
            if hits.is_empty() {
                printer.success("No suspicious values found in environments.toml");
                return Ok(());
            }
            printer.warn(&format!(
                "{} value(s) in environments.toml look like real credentials. \
                 Consider moving them to secrets.toml + the keychain.",
                hits.len()
            ));
            for hit in &hits {
                printer.kv(
                    &format!("{}/{}", hit.env, hit.variable),
                    &format!("{} ({})", hit.match_.rule, hit.match_.preview),
                );
            }
            anyhow::bail!("Leak scan found {} suspicious value(s)", hits.len());
        }
    }
    Ok(())
}

pub(crate) async fn handle_env_vars(printer: &Printer, cmd: &EnvVarsCmd) -> anyhow::Result<()> {
    match cmd {
        EnvVarsCmd::List { env, project } => {
            let path = crate::project_path(project.as_ref());
            let config = pipeline::load_environments_layered(&path)?;
            let env_obj = config
                .environments
                .iter()
                .find(|e| e.name == *env)
                .ok_or_else(|| anyhow::anyhow!("Environment '{}' not found", env))?;
            printer.header(&format!("{} Variables for '{}'", icons::GEAR, env));
            if env_obj.variables.is_empty() {
                printer.info("No variables set.");
                return Ok(());
            }
            let mut keys: Vec<&String> = env_obj.variables.keys().collect();
            keys.sort();
            for k in keys {
                printer.kv(k, &env_obj.variables[k]);
            }
        }

        EnvVarsCmd::Set {
            env,
            key,
            value,
            local,
            project,
        } => {
            let path = crate::project_path(project.as_ref());
            if *local {
                let mut config = pipeline::load_environments_local(&path)?;
                if let Some(e) = config.environments.iter_mut().find(|e| e.name == *env) {
                    e.variables.insert(key.clone(), value.clone());
                } else {
                    let mut vars = std::collections::HashMap::new();
                    vars.insert(key.clone(), value.clone());
                    config.environments.push(Environment {
                        name: env.clone(),
                        ssh_host: None,
                        ssh_port: None,
                        variables: vars,
                    });
                }
                pipeline::save_environments_local(&path, &config)?;
                printer.success(&format!(
                    "Set {}={} on env '{}' (local override)",
                    key, value, env
                ));
            } else {
                pipeline::set_env_variable(&path, env, key, value)?;
                printer.success(&format!("Set {}={} on env '{}'", key, value, env));
            }
        }

        EnvVarsCmd::Get { env, key, project } => {
            let path = crate::project_path(project.as_ref());
            let config = pipeline::load_environments_layered(&path)?;
            let env_obj = config
                .environments
                .iter()
                .find(|e| e.name == *env)
                .ok_or_else(|| anyhow::anyhow!("Environment '{}' not found", env))?;
            match env_obj.variables.get(key) {
                Some(v) => println!("{}", v),
                None => anyhow::bail!("Variable '{}' not set on env '{}'", key, env),
            }
        }

        EnvVarsCmd::Delete { env, key, project } => {
            let path = crate::project_path(project.as_ref());
            pipeline::remove_env_variable(&path, env, key)?;
            printer.success(&format!("Removed '{}' from env '{}'", key, env));
        }
    }
    Ok(())
}
