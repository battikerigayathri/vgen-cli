//! Manual workflow definition push until ResMate CLI ships (Phase 9.7).
//!
//! ```bash
//! cargo run -p smriti_client --bin push_workflow_definition -- \
//!   --path lib/smriti_client/tests/fixtures/purchase-requisition-split \
//!   [--format auto|yaml|json] [--validate-only] [--dry-run]
//! ```

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use smriti_client::{
    AuthorFormat, HttpSmritiClient, SmritiClientConfig, WorkflowDefinitionLoader,
    push_workflow_definition, resolve_workflow_path,
};

#[derive(Debug, Default)]
struct Args {
    path: Option<String>,
    format: Option<String>,
    validate_only: bool,
    dry_run: bool,
}

fn parse_args() -> Args {
    let mut args = Args::default();
    let mut iter = env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--path" => args.path = iter.next(),
            "--format" => args.format = iter.next(),
            "--validate-only" => args.validate_only = true,
            "--dry-run" => args.dry_run = true,
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            other if other.starts_with('-') => {
                eprintln!("unknown flag: {other}");
                print_help();
                std::process::exit(2);
            }
            path => args.path = Some(path.to_string()),
        }
    }
    args
}

fn print_help() {
    eprintln!(
        r#"push_workflow_definition — load, validate, and push workflow definitions

USAGE:
    push_workflow_definition --path <dir-or-file> [OPTIONS]

OPTIONS:
    --path <PATH>           Workflow directory or bundle file (required)
    --format auto|yaml|json Author format override (default: auto)
    --validate-only         Load + validate only; no Mongo write
    --dry-run               Print normalized JSON; no Mongo write
    -h, --help              Show this help

ENVIRONMENT:
    SMRITI_URL              Smriti base URL (default: http://localhost:6222)
    SMRITI_TIMEOUT_MS       HTTP timeout ms (default: 10000)
    WORKFLOWS_DIR           Root for relative --path (default: workflows)

Append-only: re-pushing the same (slug, version) returns DuplicateDefinition.
Push order: hitl → workflow → tool → agent → assistant

Pre-push checklist:
    Indexes applied?  node scripts/mongo/ensure-workflow-indexes.js
    Smriti up?        curl -sf "$SMRITI_URL/health" || just infra-up
"#
    );
}

fn resolve_format_override(
    raw: Option<&str>,
    resolved: &std::path::Path,
) -> Result<Option<AuthorFormat>, String> {
    match raw.unwrap_or("auto") {
        "auto" => Ok(None),
        "yaml" => {
            if resolved.is_file() {
                Ok(Some(AuthorFormat::BundleYaml))
            } else {
                WorkflowDefinitionLoader::detect_yaml_format(resolved)
                    .map(Some)
                    .map_err(|e| e.to_string())
            }
        }
        "json" => Ok(Some(AuthorFormat::BundleJson)),
        other => Err(format!("invalid --format {other}; use auto|yaml|json")),
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = parse_args();
    let Some(path_arg) = args.path else {
        eprintln!("error: --path is required");
        print_help();
        return ExitCode::from(2);
    };

    let workflows_dir = env::var("WORKFLOWS_DIR").unwrap_or_else(|_| "workflows".into());
    let resolved = resolve_workflow_path(PathBuf::from(workflows_dir).as_path(), &path_arg);

    let format_override = match resolve_format_override(args.format.as_deref(), &resolved) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };

    let def = match WorkflowDefinitionLoader::load(&resolved, format_override) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("load failed: {e}");
            return ExitCode::from(1);
        }
    };

    eprintln!(
        "loaded {} v{} ({:?}) from {}",
        def.slug,
        def.version,
        def.source_format,
        resolved.display()
    );

    if args.validate_only {
        eprintln!("validation OK (--validate-only)");
        return ExitCode::SUCCESS;
    }

    if args.dry_run {
        let (_, payload) = match smriti_client::prepare_for_push(def) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("normalize failed: {e}");
                return ExitCode::from(1);
            }
        };
        println!("{}", serde_json::to_string_pretty(&payload).expect("json"));
        return ExitCode::SUCCESS;
    }

    let client = match HttpSmritiClient::new(SmritiClientConfig::from_env()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Smriti client error: {e}");
            return ExitCode::from(1);
        }
    };

    match push_workflow_definition(&client, def).await {
        Ok(pushed) => {
            eprintln!(
                "pushed {} v{} (_id={})",
                pushed.slug,
                pushed.version,
                pushed.id.as_deref().unwrap_or("<unknown>")
            );
            eprintln!("Pre-push checklist:");
            eprintln!("  Indexes applied?  node scripts/mongo/ensure-workflow-indexes.js");
            eprintln!(
                "  Smriti up?        curl -sf \"${{SMRITI_URL:-http://localhost:6222}}/health\""
            );
            eprintln!("  Push order:       hitl → workflow → tool → agent → assistant");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("push failed: {e}");
            ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_format_values() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/purchase-requisition-split");
        assert!(
            resolve_format_override(Some("auto"), &dir)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            resolve_format_override(Some("yaml"), &dir).unwrap(),
            Some(AuthorFormat::SplitYaml)
        );
        assert!(resolve_format_override(Some("nope"), &dir).is_err());
    }
}
