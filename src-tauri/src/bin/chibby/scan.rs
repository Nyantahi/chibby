//! `chibby scan` subcommands (secrets, deps, SAST, IaC, license, container, commit-lint).

use crate::args::ScanCmd;
use crate::cli::{self, icons, Printer};
use chibby_lib::engine::gates;
use owo_colors::OwoColorize;
use std::path::PathBuf;

fn resolve_project_path(project: Option<&PathBuf>) -> anyhow::Result<PathBuf> {
    let p = project
        .cloned()
        .unwrap_or_else(|| std::env::current_dir().unwrap());
    let abs = p.canonicalize().unwrap_or(p);
    Ok(abs)
}

pub(crate) async fn handle_scan(printer: &Printer, cmd: &ScanCmd) -> anyhow::Result<()> {
    match cmd {
        ScanCmd::Secrets { project, baseline } => {
            let repo = resolve_project_path(project.as_ref())?;
            printer.header(&format!("{} Secret Scan", icons::SCAN));
            printer.kv("Project", &repo.display().to_string());
            printer.newline();

            if *baseline {
                let spin = cli::spinner("Creating baseline...");
                let msg = gates::create_secret_scan_baseline(&repo)?;
                spin.finish_and_clear();
                printer.success(&msg);
                return Ok(());
            }

            let config = gates::load_gates_config(&repo).unwrap_or_default();
            let spin = cli::spinner("Scanning for leaked secrets...");
            let result = gates::run_secret_scan(&repo, &config)?;
            spin.finish_and_clear();

            printer.kv("Scanner", &result.scanner);
            if result.passed && result.findings.is_empty() {
                printer.success(&result.message);
            } else {
                printer.warn(&format!("{} finding(s)", result.findings.len()));
                for f in result.findings.iter().take(50) {
                    println!(
                        "  {} {}:{} — {} ({})",
                        icons::WARN.yellow(),
                        f.file.bright_black(),
                        f.line,
                        f.rule.red(),
                        f.preview.bright_black()
                    );
                }
                if result.findings.len() > 50 {
                    printer.info(&format!("+ {} more", result.findings.len() - 50));
                }
                if !result.passed {
                    std::process::exit(1);
                }
            }
        }
        ScanCmd::Deps { project, severity } => {
            let repo = resolve_project_path(project.as_ref())?;
            printer.header(&format!("{} Dependency Scan", icons::BUG));
            printer.kv("Project", &repo.display().to_string());
            printer.kv("Severity threshold", severity);
            printer.newline();

            let mut config = gates::load_gates_config(&repo).unwrap_or_default();
            config.audit_severity_threshold = severity.clone();

            let spin = cli::spinner("Scanning dependencies...");
            let result = gates::run_dependency_audit(&repo, &config)?;
            spin.finish_and_clear();

            printer.kv("Scanner", &result.scanner);
            if result.passed && result.findings.is_empty() {
                printer.success(&result.message);
            } else {
                printer.warn(&format!("{} vulnerability(ies)", result.findings.len()));
                for f in result.findings.iter().take(50) {
                    println!(
                        "  {} {} {} — {} ({})",
                        icons::WARN.yellow(),
                        f.package.bright_white(),
                        f.installed_version.bright_black(),
                        f.advisory_id.red(),
                        format!("{:?}", f.severity).to_lowercase()
                    );
                    if let Some(fixed) = &f.fixed_version {
                        println!("      fixed in {}", fixed.green());
                    }
                }
                if result.findings.len() > 50 {
                    printer.info(&format!("+ {} more", result.findings.len() - 50));
                }
                if !result.passed {
                    std::process::exit(1);
                }
            }
        }
        ScanCmd::Commits { project, since } => {
            let repo = resolve_project_path(project.as_ref())?;
            printer.header(&format!("{} Commit Lint", icons::CHECK));
            printer.kv("Project", &repo.display().to_string());
            if let Some(s) = since {
                printer.kv("Since", s);
            }
            printer.newline();

            let config = gates::load_gates_config(&repo).unwrap_or_default();
            let spin = cli::spinner("Checking commit messages...");
            let result = gates::run_commit_lint(&repo, &config)?;
            spin.finish_and_clear();

            printer.kv("Commits checked", &result.commits_checked.to_string());
            if result.passed {
                printer.success(&result.message);
            } else {
                printer.warn(&format!("{} violation(s)", result.violations.len()));
                for v in result.violations.iter().take(50) {
                    println!(
                        "  {} {} — {}",
                        v.hash.bright_black(),
                        v.subject.white(),
                        v.rule.red()
                    );
                    println!("      expected: {}", v.expected.bright_black());
                }
                std::process::exit(1);
            }
        }
        ScanCmd::Sast { project } => {
            let repo = resolve_project_path(project.as_ref())?;
            printer.header(&format!("{} SAST (semgrep)", icons::SCAN));
            printer.kv("Project", &repo.display().to_string());
            printer.newline();

            let config = gates::load_gates_config(&repo).unwrap_or_default();
            let spin = cli::spinner("Running semgrep...");
            let result = gates::run_sast(&repo, &config)?;
            spin.finish_and_clear();

            printer.kv("Scanner", &result.scanner);
            if result.passed && result.findings.is_empty() {
                printer.success(&result.message);
            } else {
                printer.warn(&result.message);
                for f in result.findings.iter().take(50) {
                    println!(
                        "  {} {}:{} — {} {}",
                        icons::WARN.yellow(),
                        f.file.bright_black(),
                        f.line,
                        f.rule.red(),
                        format!("[{:?}]", f.severity).bright_black()
                    );
                    if !f.message.is_empty() {
                        println!("      {}", f.message.bright_black());
                    }
                }
                if result.findings.len() > 50 {
                    printer.info(&format!("+ {} more", result.findings.len() - 50));
                }
                if !result.passed {
                    std::process::exit(1);
                }
            }
        }
        ScanCmd::Container { project } => {
            let repo = resolve_project_path(project.as_ref())?;
            printer.header(&format!("{} Container Scan (trivy)", icons::SCAN));
            printer.kv("Project", &repo.display().to_string());
            printer.newline();

            let config = gates::load_gates_config(&repo).unwrap_or_default();
            let spin = cli::spinner("Running trivy image...");
            let result = gates::run_container_scan(&repo, &config)?;
            spin.finish_and_clear();

            printer.kv("Scanner", &result.scanner);
            printer.kv("Targets", &result.targets.join(", "));
            if result.passed && result.findings.is_empty() {
                printer.success(&result.message);
            } else {
                printer.warn(&result.message);
                for f in result.findings.iter().take(50) {
                    println!(
                        "  {} {} {} — {} [{:?}]",
                        icons::WARN.yellow(),
                        f.package.bright_white(),
                        f.installed_version.bright_black(),
                        f.advisory_id.red(),
                        f.severity
                    );
                    if let Some(fixed) = &f.fixed_version {
                        println!("      fixed in {}", fixed.green());
                    }
                }
                if !result.passed {
                    std::process::exit(1);
                }
            }
        }
        ScanCmd::Iac { project } => {
            let repo = resolve_project_path(project.as_ref())?;
            printer.header(&format!("{} IaC Scan (trivy config)", icons::SCAN));
            printer.kv("Project", &repo.display().to_string());
            printer.newline();

            let config = gates::load_gates_config(&repo).unwrap_or_default();
            let spin = cli::spinner("Running trivy config...");
            let result = gates::run_iac_scan(&repo, &config)?;
            spin.finish_and_clear();

            printer.kv("Scanner", &result.scanner);
            if result.passed && result.findings.is_empty() {
                printer.success(&result.message);
            } else {
                printer.warn(&result.message);
                for f in result.findings.iter().take(50) {
                    let loc = match f.line {
                        Some(l) => format!("{}:{}", f.file, l),
                        None => f.file.clone(),
                    };
                    println!(
                        "  {} {} — {} [{:?}]",
                        icons::WARN.yellow(),
                        loc.bright_black(),
                        f.rule.red(),
                        f.severity
                    );
                    if !f.message.is_empty() {
                        println!("      {}", f.message.bright_black());
                    }
                    if let Some(r) = &f.resolution {
                        println!("      fix: {}", r.green());
                    }
                }
                if !result.passed {
                    std::process::exit(1);
                }
            }
        }
        ScanCmd::License { project } => {
            let repo = resolve_project_path(project.as_ref())?;
            printer.header(&format!("{} License Check", icons::SCAN));
            printer.kv("Project", &repo.display().to_string());
            printer.newline();

            let config = gates::load_gates_config(&repo).unwrap_or_default();
            let spin = cli::spinner("Checking dependency licenses...");
            let result = gates::run_license_check(&repo, &config)?;
            spin.finish_and_clear();

            printer.kv("Scanner", &result.scanner);
            if result.passed && result.findings.is_empty() {
                printer.success(&result.message);
            } else {
                printer.warn(&result.message);
                for f in result.findings.iter().take(50) {
                    println!(
                        "  {} {} {} — {} ({})",
                        icons::WARN.yellow(),
                        f.package.bright_white(),
                        f.version.bright_black(),
                        f.license.red(),
                        f.reason.bright_black()
                    );
                }
                if !result.passed {
                    std::process::exit(1);
                }
            }
        }
    }
    Ok(())
}
