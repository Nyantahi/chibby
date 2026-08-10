//! `chibby import` / `chibby export` subcommands (.env, Vercel, Railway, Fly.io).

use crate::args::{ExportCmd, ImportCmd};
use crate::cli::{icons, Printer};
use chibby_lib::engine::bootstrap::Classification;
use chibby_lib::engine::importers::{
    self, dotenv::DotEnvImporter, flyio::FlyImporter, railway::RailwayImporter,
    vercel::VercelImporter, ApplyOptions, ImportContext, ImportReport, Importer,
};

pub(crate) async fn handle_import(printer: &Printer, cmd: &ImportCmd) -> anyhow::Result<()> {
    let (report, repo_path) = match cmd {
        ImportCmd::Dotenv {
            path,
            env,
            with_values,
            project,
        } => {
            let repo = crate::project_path(project.as_ref());
            let ctx = ImportContext {
                repo_path: repo.clone(),
                env_name: env.clone(),
                source_path: Some(path.clone()),
                include_values: *with_values,
            };
            (DotEnvImporter.run(&ctx)?, repo)
        }
        ImportCmd::Vercel {
            env,
            with_values,
            project,
        } => {
            let repo = crate::project_path(project.as_ref());
            let ctx = ImportContext {
                repo_path: repo.clone(),
                env_name: env.clone(),
                source_path: None,
                include_values: *with_values,
            };
            (VercelImporter.run(&ctx)?, repo)
        }
        ImportCmd::Railway {
            env,
            with_values,
            project,
        } => {
            let repo = crate::project_path(project.as_ref());
            let ctx = ImportContext {
                repo_path: repo.clone(),
                env_name: env.clone(),
                source_path: None,
                include_values: *with_values,
            };
            (RailwayImporter.run(&ctx)?, repo)
        }
        ImportCmd::Fly { env, project } => {
            let repo = crate::project_path(project.as_ref());
            let ctx = ImportContext {
                repo_path: repo.clone(),
                env_name: env.clone(),
                source_path: None,
                include_values: false,
            };
            (FlyImporter.run(&ctx)?, repo)
        }
    };

    printer.header(&format!("{} Import from {}", icons::GEAR, report.source));
    printer.kv("Env", &report.env_name);
    printer.kv("Detected", &report.entries.len().to_string());
    printer.newline();

    print_import_report(printer, &report);

    let applied = importers::apply_report(&report, &repo_path, ApplyOptions::default())?;
    printer.newline();
    printer.success(&format!(
        "Variables: {} added ({} with values), Secret refs: {} added ({} values stored in keychain)",
        applied.variables_added,
        applied.variables_value_set,
        applied.secrets_ref_added,
        applied.secrets_value_saved
    ));
    if applied.secrets_value_saved == 0
        && report
            .entries
            .iter()
            .any(|e| e.classification == Classification::Secret)
    {
        printer.info(
            "No secret values were stored. Re-run with `--with-values` (where supported) or set them with `chibby secrets set NAME --env <env>`.",
        );
    }
    Ok(())
}

fn print_import_report(printer: &Printer, report: &ImportReport) {
    let mut secrets: Vec<&_> = report
        .entries
        .iter()
        .filter(|e| e.classification == Classification::Secret)
        .collect();
    let mut vars: Vec<&_> = report
        .entries
        .iter()
        .filter(|e| e.classification == Classification::Variable)
        .collect();
    secrets.sort_by(|a, b| a.name.cmp(&b.name));
    vars.sort_by(|a, b| a.name.cmp(&b.name));

    if !vars.is_empty() {
        printer.subheader(&format!("Variables ({})", vars.len()));
        for v in &vars {
            let label = if v.value.is_some() {
                "value"
            } else {
                "name only"
            };
            printer.kv(&v.name, label);
        }
        printer.newline();
    }
    if !secrets.is_empty() {
        printer.subheader(&format!("Secrets ({})", secrets.len()));
        for s in &secrets {
            let label = if s.value.is_some() {
                "value"
            } else {
                "name only"
            };
            printer.kv(&s.name, label);
        }
    }
}

pub(crate) async fn handle_export(printer: &Printer, cmd: &ExportCmd) -> anyhow::Result<()> {
    let ExportCmd::Dotenv { env, out, project } = cmd;
    let repo = crate::project_path(project.as_ref());
    let lines = importers::export_dotenv(&repo, env, out)?;
    printer.success(&format!("Wrote {} lines to {}", lines, out.display()));
    printer.info(
        "This file may contain plaintext secrets — keep it out of git and treat it like a credential.",
    );
    Ok(())
}
