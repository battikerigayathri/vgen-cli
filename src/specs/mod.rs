mod agent;
mod assistant;
mod hitl;
pub mod normalize;
mod tool;
mod workflow;
pub mod workflow_dag;

use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub use agent::{
    default_agents_dir, list_agent_files, load_agent, upsert_agent_from_api_data,
    write_agent_id_to_yaml, write_agent_yaml_from_record,
};
pub use assistant::{
    default_assistants_dir, list_assistant_files, load_assistant, parse_assistant_workflow_slug,
    write_assistant_id_to_yaml, write_assistant_yaml_from_record,
};
pub use hitl::{
    default_hitl_dir, get_hitl_id_and_meta_path, is_hitl_dir, list_hitl_dirs, load_hitl_from_dir,
    write_hitl_from_record, write_hitl_id_to_meta,
};
pub use tool::{
    default_tools_dir, get_tool_id_and_yaml_path, is_tool_dir, list_tool_dirs, load_tool_from_dir,
    load_tool_yaml, upsert_tool_from_api_data, write_tool_from_record, write_tool_id_to_yaml,
};
pub use workflow::{
    default_workflows_dir, detect_format, increment_workflow_version, is_workflow_dir,
    list_workflow_dirs, load_workflow_from_dir, workflow_api_slug, write_workflow_from_definition,
    write_workflow_id, AuthorFormat,
};
pub use workflow_dag::{
    bundle_gates, condition_summary, gate_composition_edges, gate_refs_in_condition,
    stage_declaration_order, stage_transitions, transition_is_back_edge, transition_ref_value,
    transition_reset_fields, Condition, TransitionRule, WorkflowGateDef,
};

use crate::workspace::Workspace;

/// Lookup tables for resolving cross-artifact references by platform id or slug.
#[derive(Debug, Clone, Default)]
pub struct ResourceIndex {
    pub assistants_by_id: HashMap<String, PathBuf>,
    pub assistants_by_slug: HashMap<String, PathBuf>,
    pub assistants_by_stem: HashMap<String, PathBuf>,
    pub agents_by_id: HashMap<String, PathBuf>,
    pub agents_by_slug: HashMap<String, PathBuf>,
    pub agents_by_stem: HashMap<String, PathBuf>,
    pub tools_by_id: HashMap<String, PathBuf>,
    pub hitl_by_slug: HashMap<String, PathBuf>,
    pub workflows_by_slug: HashMap<String, PathBuf>,
}

impl ResourceIndex {
    pub fn build(ws: &Workspace) -> Result<Self, std::io::Error> {
        let mut index = Self::default();

        for (stem, path) in list_assistant_files(&ws.assistants_dir) {
            if let Ok(value) = read_yaml(&path) {
                index.assistants_by_stem.insert(stem, path.clone());
                if let Some(id) = string_field(&value, "id") {
                    index.assistants_by_id.insert(id, path.clone());
                }
                if let Some(slug) = string_field(&value, "slug") {
                    index.assistants_by_slug.insert(slug, path);
                }
            }
        }

        for (stem, path) in list_agent_files(&ws.agents_dir) {
            if let Ok(value) = read_yaml(&path) {
                index.agents_by_stem.insert(stem, path.clone());
                if let Some(id) = string_field(&value, "id") {
                    index.agents_by_id.insert(id, path.clone());
                }
                if let Some(slug) = string_field(&value, "slug") {
                    index.agents_by_slug.insert(slug, path);
                }
            }
        }

        for tool_dir in list_tool_dirs(&ws.tools_dir) {
            if let Ok((value, _)) = load_tool_yaml(&tool_dir) {
                if let Some(id) = string_field(&value, "id") {
                    index.tools_by_id.insert(id, tool_dir);
                }
            }
        }

        for hitl_dir in list_hitl_dirs(&ws.hitl_dir) {
            let meta_path = hitl_meta_path(&hitl_dir);
            if let Some(meta_path) = meta_path {
                if let Ok(value) = read_yaml(&meta_path) {
                    if let Some(slug) = string_field(&value, "slug") {
                        index.hitl_by_slug.insert(slug, hitl_dir);
                    }
                }
            }
        }

        for workflow_dir in list_workflow_dirs(&ws.workflows_dir) {
            if let Ok((bundle, _, _)) = load_workflow_from_dir(&workflow_dir) {
                let slug = workflow_slug_from_bundle(&bundle);
                if let Some(slug) = slug {
                    index.workflows_by_slug.insert(slug, workflow_dir);
                }
            }
        }

        Ok(index)
    }
}

pub(crate) fn list_yaml_files(base: &Path) -> Vec<(String, PathBuf)> {
    if !base.is_dir() {
        return Vec::new();
    }
    std::fs::read_dir(base)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_file() {
                return None;
            }
            let ext = path.extension().and_then(|s| s.to_str())?;
            if ext != "yaml" && ext != "yml" {
                return None;
            }
            let stem = path.file_stem()?.to_str()?.to_string();
            Some((stem, path))
        })
        .collect()
}

fn read_yaml(path: &Path) -> Result<Value, std::io::Error> {
    let content = std::fs::read_to_string(path)?;
    serde_yaml::from_str(&content)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn hitl_meta_path(hitl_dir: &Path) -> Option<PathBuf> {
    let yaml = hitl_dir.join("meta.yaml");
    if yaml.is_file() {
        return Some(yaml);
    }
    let yml = hitl_dir.join("meta.yml");
    if yml.is_file() {
        return Some(yml);
    }
    None
}

fn workflow_slug_from_bundle(bundle: &Value) -> Option<String> {
    bundle
        .get("meta")
        .and_then(|m| string_field(m, "slug"))
        .or_else(|| string_field(bundle, "slug"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_assistant_workflow_slug_reads_value() {
        let ctx = "workflow_type: oracle_purchase_requisition\nworkflow_definition_slug: oracle-purchase-requisition-v1\n";
        assert_eq!(
            parse_assistant_workflow_slug(ctx),
            Some("oracle-purchase-requisition-v1".to_string())
        );
    }
}
