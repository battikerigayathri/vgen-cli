use crate::api::{find_hitl_by_slug, get_record};
use crate::output::{emit_error, emit_success, CliContext, CliExitCode, OutputMode};
use crate::specs::normalize::normalize_mongo_oids;
use crate::specs::{
    default_agents_dir, default_assistants_dir, default_hitl_dir, default_tools_dir,
    default_workflows_dir, parse_assistant_workflow_slug, upsert_agent_from_api_data,
    upsert_tool_from_api_data, write_assistant_yaml_from_record, write_hitl_from_record,
    write_workflow_from_definition,
};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Debug, Clone)]
pub struct SyncOptions {
    pub assistants_dir: PathBuf,
    pub agents_dir: PathBuf,
    pub tools_dir: PathBuf,
    pub workflows_dir: PathBuf,
    pub hitl_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SyncReport {
    pub assistants: u32,
    pub workflows: u32,
    pub hitl: u32,
    pub agents: u32,
    pub tools: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

impl Default for SyncReport {
    fn default() -> Self {
        Self {
            assistants: 0,
            workflows: 0,
            hitl: 0,
            agents: 0,
            tools: 0,
            warnings: Vec::new(),
        }
    }
}

pub fn sync_options_from_cmd(
    assistants_dir: Option<PathBuf>,
    agents_dir: Option<PathBuf>,
    tools_dir: Option<PathBuf>,
    workflows_dir: Option<PathBuf>,
    hitl_dir: Option<PathBuf>,
) -> SyncOptions {
    SyncOptions {
        assistants_dir: assistants_dir.unwrap_or_else(default_assistants_dir),
        agents_dir: agents_dir.unwrap_or_else(default_agents_dir),
        tools_dir: tools_dir.unwrap_or_else(default_tools_dir),
        workflows_dir: workflows_dir.unwrap_or_else(default_workflows_dir),
        hitl_dir: hitl_dir.unwrap_or_else(default_hitl_dir),
    }
}

pub async fn run_sync(opts: SyncOptions) -> Result<SyncReport, String> {
    if !opts.assistants_dir.exists() || !opts.assistants_dir.is_dir() {
        return Err(format!(
            "Assistants directory not found: {}",
            opts.assistants_dir.display()
        ));
    }

    let mut report = SyncReport::default();
    let entries = std::fs::read_dir(&opts.assistants_dir)
        .map_err(|e| format!("Cannot read assistants dir: {}", e))?;

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                report.warnings.push(format!("Skipping dir entry: {}", e));
                continue;
            }
        };
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "yaml" && ext != "yml" {
            continue;
        }

        let assistant_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let local_yaml = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                report
                    .warnings
                    .push(format!("Skipping {}: {}", path.display(), e));
                continue;
            }
        };
        let local_value: Value = match serde_yaml::from_str(&local_yaml) {
            Ok(v) => v,
            Err(e) => {
                report
                    .warnings
                    .push(format!("Skipping {} (invalid YAML): {}", path.display(), e));
                continue;
            }
        };
        let local_value = normalize_mongo_oids(local_value);
        let assistant_id = match local_value
            .get("id")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            Some(id) => id.to_string(),
            None => {
                report.warnings.push(format!(
                    "Skipping assistants/{}.yaml — no id (push first)",
                    assistant_name
                ));
                continue;
            }
        };

        let res = match get_record("chat", &assistant_id).await {
            Ok(r) => r,
            Err(e) => {
                report.warnings.push(format!(
                    "Failed to pull assistant '{}': {}",
                    assistant_name, e
                ));
                continue;
            }
        };
        let data = match res.get("data") {
            Some(d) => normalize_mongo_oids(d.clone()),
            None => {
                report.warnings.push(format!(
                    "No data in response for assistant '{}'",
                    assistant_name
                ));
                continue;
            }
        };
        if let Err(e) = write_assistant_yaml_from_record(&path, &data) {
            report.warnings.push(format!(
                "Failed to write assistant YAML '{}': {}",
                assistant_name, e
            ));
            continue;
        }
        report.assistants += 1;

        if let Some(sys_context) = data.get("systemContext").and_then(|v| v.as_str()) {
            if let Some(workflow_slug) = parse_assistant_workflow_slug(sys_context) {
                match get_workflow_definition_and_sync(
                    &workflow_slug,
                    &opts.workflows_dir,
                    &opts.hitl_dir,
                    &mut report,
                )
                .await
                {
                    Ok(()) => {}
                    Err(msg) => report.warnings.push(msg),
                }
            }
        }

        let agent_ids: Vec<String> = data
            .get("agents")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        for agent_id in &agent_ids {
            let agent_res = match get_record("agentConfig", agent_id).await {
                Ok(r) => r,
                Err(e) => {
                    report
                        .warnings
                        .push(format!("Failed to pull agent '{}': {}", agent_id, e));
                    continue;
                }
            };
            let agent_data = match agent_res.get("data") {
                Some(d) => normalize_mongo_oids(d.clone()),
                None => {
                    report
                        .warnings
                        .push(format!("No data in response for agent '{}'", agent_id));
                    continue;
                }
            };
            let agent_slug = match upsert_agent_from_api_data(&opts.agents_dir, &agent_data) {
                Ok(s) => s,
                Err(e) => {
                    report
                        .warnings
                        .push(format!("Failed to write agent '{}': {}", agent_id, e));
                    continue;
                }
            };
            report.agents += 1;
            let _ = agent_slug;

            let tool_ids: Vec<String> = agent_data
                .get("skills")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            for tool_id in &tool_ids {
                let tool_res = match get_record("skillConfig", tool_id).await {
                    Ok(r) => r,
                    Err(e) => {
                        report
                            .warnings
                            .push(format!("Failed to pull tool '{}': {}", tool_id, e));
                        continue;
                    }
                };
                let tool_data = match tool_res.get("data") {
                    Some(d) => normalize_mongo_oids(d.clone()),
                    None => {
                        report
                            .warnings
                            .push(format!("No data in response for tool '{}'", tool_id));
                        continue;
                    }
                };
                match upsert_tool_from_api_data(&opts.tools_dir, &tool_data) {
                    Ok(_folder) => report.tools += 1,
                    Err(e) => report
                        .warnings
                        .push(format!("Failed to write tool '{}': {}", tool_id, e)),
                }
            }
        }
    }

    Ok(report)
}

async fn get_workflow_definition_and_sync(
    workflow_slug: &str,
    workflows_dir: &Path,
    hitl_dir: &Path,
    report: &mut SyncReport,
) -> Result<(), String> {
    use crate::api::get_workflow_definition;

    let wf_res = get_workflow_definition(workflow_slug, None)
        .await
        .map_err(|e| format!("Failed to pull workflow '{}': {}", workflow_slug, e))?;
    let wf_data = wf_res
        .get("data")
        .ok_or_else(|| format!("No data in response for workflow '{}'", workflow_slug))?;
    let wf_dir = workflows_dir.join(workflow_slug);
    write_workflow_from_definition(&wf_dir, wf_data)
        .map_err(|e| format!("Failed to write workflow '{}': {}", workflow_slug, e))?;
    report.workflows += 1;

    sync_hitl_from_workflow(wf_data, hitl_dir, report).await;
    Ok(())
}

pub async fn sync_hitl_from_workflow(wf_data: &Value, hitl_dir: &Path, report: &mut SyncReport) {
    let slugs = extract_hitl_slugs(wf_data);
    let mut synced = HashSet::new();

    for slug in slugs {
        if !synced.insert(slug.clone()) {
            continue;
        }
        match pull_hitl_by_slug(&slug, hitl_dir).await {
            Ok(()) => report.hitl += 1,
            Err(msg) => report.warnings.push(msg),
        }
    }
}

pub fn extract_hitl_slugs(wf_data: &Value) -> Vec<String> {
    let mut slugs = Vec::new();
    let flow = wf_data
        .get("authorBundle")
        .and_then(|b| b.get("flow"))
        .or_else(|| wf_data.get("flow"));

    if let Some(stages) = flow
        .and_then(|f| f.get("stages"))
        .and_then(|s| s.as_array())
    {
        for stage in stages {
            if let Some(hitl_slug) = stage
                .get("hitlSlug")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                slugs.push(hitl_slug.to_string());
            }
        }
    }
    slugs.sort();
    slugs.dedup();
    slugs
}

fn sanitize_folder_name(slug: &str) -> String {
    slug.replace('/', "-").replace('\\', "-")
}

fn resolve_hitl_folder(hitl_dir: &Path, slug: &str) -> PathBuf {
    if hitl_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(hitl_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let meta_path = if path.join("meta.yaml").is_file() {
                    path.join("meta.yaml")
                } else if path.join("meta.yml").is_file() {
                    path.join("meta.yml")
                } else {
                    continue;
                };
                if let Ok(content) = std::fs::read_to_string(&meta_path) {
                    if let Ok(value) = serde_yaml::from_str::<Value>(&content) {
                        if value
                            .get("slug")
                            .and_then(|v| v.as_str())
                            .map(|s| s == slug)
                            .unwrap_or(false)
                        {
                            return path;
                        }
                    }
                }
            }
        }
    }
    hitl_dir.join(sanitize_folder_name(slug))
}

async fn pull_hitl_by_slug(slug: &str, hitl_dir: &Path) -> Result<(), String> {
    let folder = resolve_hitl_folder(hitl_dir, slug);
    std::fs::create_dir_all(&folder)
        .map_err(|e| format!("Failed to create HITL dir {}: {}", folder.display(), e))?;

    let record = find_hitl_by_slug(slug)
        .await
        .map_err(|e| format!("Failed to resolve HITL slug '{}': {}", slug, e))?;
    let data = record.get("data").cloned().unwrap_or(record);
    write_hitl_from_record(&folder, &data)
        .map_err(|e| format!("Failed to write HITL '{}': {}", slug, e))?;
    Ok(())
}

pub async fn run_sync_command(ctx: &CliContext, opts: SyncOptions) -> ExitCode {
    match run_sync(opts).await {
        Ok(report) => {
            match ctx.mode {
                OutputMode::Human => print_human_sync(&report),
                OutputMode::Json => emit_success(ctx, report),
            }
            CliExitCode::Success.into()
        }
        Err(msg) => emit_error(ctx, "SYNC_FAILED", msg, None, CliExitCode::RuntimeError),
    }
}

fn print_human_sync(report: &SyncReport) {
    for warning in &report.warnings {
        eprintln!("[warn] {}", warning);
    }
    println!(
        "\nSync complete: {} assistant(s), {} agent(s), {} workflow(s), {} hitl(s), {} tool(s) synced.",
        report.assistants,
        report.agents,
        report.workflows,
        report.hitl,
        report.tools
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_hitl_slugs_from_author_bundle() {
        let wf = json!({
            "authorBundle": {
                "flow": {
                    "stages": [
                        { "hitlSlug": "roc-select-requester-form" },
                        { "hitlSlug": "roc-select-requester-form" },
                        { "agentSlug": "oracle-pr-agent" }
                    ]
                }
            }
        });
        let slugs = extract_hitl_slugs(&wf);
        assert_eq!(slugs, vec!["roc-select-requester-form"]);
    }

    #[test]
    fn extract_hitl_slugs_from_flat_flow() {
        let wf = json!({
            "flow": {
                "stages": [{ "hitlSlug": "form-a" }, { "hitlSlug": "form-b" }]
            }
        });
        let slugs = extract_hitl_slugs(&wf);
        assert_eq!(slugs, vec!["form-a", "form-b"]);
    }
}
