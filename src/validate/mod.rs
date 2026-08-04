mod artifact_rules;
mod graph_rules;
mod handler_lint;
mod push_readiness;
pub mod remote_rules;
mod secrets;
mod workflow_rules;

use crate::graph::{build_graph, Graph, GraphError};
use crate::specs::ResourceIndex;
use crate::workspace::Workspace;
use serde::Serialize;
use std::path::Path;

pub use graph_rules::check_graph_rules;
pub use workflow_rules::check_workflow_rules;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    Assistant,
    Agent,
    Tool,
    Hitl,
    Workflow,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ResourceRef {
    pub kind: ResourceKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Finding {
    pub code: String,
    pub severity: Severity,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<ResourceRef>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ValidationSummary {
    pub error_count: usize,
    pub warning_count: usize,
    pub info_count: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ValidationReport {
    pub findings: Vec<Finding>,
    pub summary: ValidationSummary,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ValidateOptions {
    pub strict: bool,
    pub offline: bool,
    pub remote: bool,
}

#[derive(Debug)]
pub enum ValidateError {
    Graph(GraphError),
}

impl std::fmt::Display for ValidateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidateError::Graph(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for ValidateError {}

impl From<GraphError> for ValidateError {
    fn from(value: GraphError) -> Self {
        ValidateError::Graph(value)
    }
}

pub fn run_workspace_validation(
    ws: &Workspace,
    opts: ValidateOptions,
) -> Result<ValidationReport, ValidateError> {
    let graph = build_graph(ws)?;
    let index = ResourceIndex::build(ws).map_err(GraphError::Io)?;
    Ok(collect_validation_report(ws, &graph, &index, opts))
}

pub async fn run_workspace_validation_async(
    ws: &Workspace,
    opts: ValidateOptions,
) -> Result<ValidationReport, ValidateError> {
    let graph = build_graph(ws)?;
    let index = ResourceIndex::build(ws).map_err(GraphError::Io)?;
    let mut report = collect_validation_report(ws, &graph, &index, opts);

    if opts.remote {
        if opts.offline {
            push_finding(
                &mut report.findings,
                Finding {
                    code: "REMOTE_SKIPPED_OFFLINE".to_string(),
                    severity: Severity::Info,
                    message: "remote ID checks skipped because --offline was set".to_string(),
                    path: None,
                    resource: None,
                },
            );
        } else {
            let mut cache = crate::remote_cache::RemoteCache::new();
            report
                .findings
                .extend(remote_rules::check_remote_rules(ws, &mut cache).await);
        }
        report.summary = summarize(&report.findings);
    }

    Ok(report)
}

pub(crate) fn collect_validation_report(
    ws: &Workspace,
    graph: &Graph,
    index: &ResourceIndex,
    _opts: ValidateOptions,
) -> ValidationReport {
    let mut findings = Vec::new();
    findings.extend(check_graph_rules(ws, graph, index));
    findings.extend(artifact_rules::check_artifact_rules(ws));
    findings.extend(handler_lint::check_handler_lint(ws));
    findings.extend(check_workflow_rules(ws, graph, index));
    findings.extend(secrets::check_secrets(ws));
    findings.extend(push_readiness::check_push_readiness(ws, graph, index));

    findings.sort_by(|a, b| {
        (
            severity_rank(&a.severity),
            a.path.as_deref().unwrap_or(""),
            a.code.as_str(),
        )
            .cmp(&(
                severity_rank(&b.severity),
                b.path.as_deref().unwrap_or(""),
                b.code.as_str(),
            ))
    });

    let summary = summarize(&findings);
    ValidationReport { findings, summary }
}

fn severity_rank(severity: &Severity) -> u8 {
    match severity {
        Severity::Error => 0,
        Severity::Warning => 1,
        Severity::Info => 2,
    }
}

fn summarize(findings: &[Finding]) -> ValidationSummary {
    let mut error_count = 0;
    let mut warning_count = 0;
    let mut info_count = 0;
    for finding in findings {
        match finding.severity {
            Severity::Error => error_count += 1,
            Severity::Warning => warning_count += 1,
            Severity::Info => info_count += 1,
        }
    }
    ValidationSummary {
        error_count,
        warning_count,
        info_count,
        total: findings.len(),
    }
}

pub(crate) fn relative_path(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

pub(crate) fn push_finding(findings: &mut Vec<Finding>, finding: Finding) {
    findings.push(finding);
}

pub(crate) fn read_yaml(path: &Path) -> Result<serde_json::Value, std::io::Error> {
    let content = std::fs::read_to_string(path)?;
    serde_yaml::from_str(&content)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

pub(crate) fn string_field(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

pub(crate) fn missing_required_field(
    findings: &mut Vec<Finding>,
    path: &Path,
    ws: &Workspace,
    resource: ResourceRef,
    field: &str,
) {
    push_finding(
        findings,
        Finding {
            code: "MISSING_REQUIRED_FIELD".to_string(),
            severity: Severity::Error,
            message: format!("missing required field `{field}`"),
            path: Some(relative_path(path, &ws.root)),
            resource: Some(resource),
        },
    );
}
