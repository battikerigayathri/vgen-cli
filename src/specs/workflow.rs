use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorFormat {
    SplitYaml,
    BundleYaml,
    BundleJson,
}

impl AuthorFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SplitYaml => "split_yaml",
            Self::BundleYaml => "bundle_yaml",
            Self::BundleJson => "bundle_json",
        }
    }
}

/// API slug for workflow pull: `meta.yaml` slug when present, else folder name.
pub fn workflow_api_slug(workflow_dir: &Path, folder_name: &str) -> String {
    for name in ["meta.yaml", "meta.yml"] {
        let path = workflow_dir.join(name);
        if !path.is_file() {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(meta) = serde_yaml::from_str::<Value>(&content) else {
            continue;
        };
        if let Some(slug) = meta.get("slug").and_then(|v| v.as_str()) {
            let slug = slug.trim();
            if !slug.is_empty() {
                return slug.to_string();
            }
        }
    }
    folder_name.to_string()
}

/// True when a directory contains a recognised workflow author layout.
pub fn is_workflow_dir(path: &Path) -> bool {
    crate::workflow_loader::is_workflow_dir(path)
}

/// List workflow directories that pass [`detect_format`].
pub fn list_workflow_dirs(base: &Path) -> Vec<PathBuf> {
    if !base.is_dir() {
        return Vec::new();
    }
    std::fs::read_dir(base)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_dir() && is_workflow_dir(path))
        .collect()
}

pub fn default_workflows_dir() -> PathBuf {
    std::env::var("VGEN_WORKFLOWS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("workflows"))
}

pub fn detect_format(dir: &Path) -> Result<AuthorFormat, Box<dyn std::error::Error + Send + Sync>> {
    crate::workflow_loader::detect_format(dir)
}

pub fn load_workflow_from_dir(
    dir: &Path,
) -> Result<(Value, PathBuf, AuthorFormat), Box<dyn std::error::Error + Send + Sync>> {
    crate::workflow_loader::load_workflow_from_dir(dir)
}

pub fn increment_workflow_version(
    file_path: &Path,
    format: AuthorFormat,
) -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
    let content = std::fs::read_to_string(file_path)?;
    let mut value: Value = match format {
        AuthorFormat::BundleJson => serde_json::from_str(&content)?,
        AuthorFormat::BundleYaml | AuthorFormat::SplitYaml => serde_yaml::from_str(&content)?,
    };

    let new_version = match format {
        AuthorFormat::SplitYaml => {
            let obj = value
                .as_object_mut()
                .ok_or("meta.yaml root is not an object")?;
            let version = obj.get("version").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let next_v = version + 1;
            obj.insert("version".to_string(), Value::Number(next_v.into()));
            next_v
        }
        AuthorFormat::BundleYaml | AuthorFormat::BundleJson => {
            let obj = value
                .as_object_mut()
                .ok_or("workflow root is not an object")?;
            let meta = obj
                .get_mut("meta")
                .and_then(|m| m.as_object_mut())
                .ok_or("workflow has no meta object")?;
            let version = meta.get("version").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let next_v = version + 1;
            meta.insert("version".to_string(), Value::Number(next_v.into()));
            next_v
        }
    };

    let out = match format {
        AuthorFormat::BundleJson => serde_json::to_string_pretty(&value)?,
        AuthorFormat::BundleYaml | AuthorFormat::SplitYaml => serde_yaml::to_string(&value)?,
    };
    std::fs::write(file_path, out)?;
    Ok(new_version)
}

pub fn write_workflow_id(
    file_path: &Path,
    format: AuthorFormat,
    id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let content = std::fs::read_to_string(file_path)?;
    let mut value: Value = match format {
        AuthorFormat::BundleJson => serde_json::from_str(&content)?,
        AuthorFormat::BundleYaml | AuthorFormat::SplitYaml => serde_yaml::from_str(&content)?,
    };

    match format {
        AuthorFormat::SplitYaml => {
            let obj = value
                .as_object_mut()
                .ok_or("meta.yaml root is not an object")?;
            obj.insert("id".to_string(), Value::String(id.to_string()));
        }
        AuthorFormat::BundleYaml | AuthorFormat::BundleJson => {
            let obj = value
                .as_object_mut()
                .ok_or("workflow root is not an object")?;
            let meta = obj
                .get_mut("meta")
                .and_then(|m| m.as_object_mut())
                .ok_or("workflow has no meta object")?;
            meta.insert("id".to_string(), Value::String(id.to_string()));
        }
    }

    let out = match format {
        AuthorFormat::BundleJson => serde_json::to_string_pretty(&value)?,
        AuthorFormat::BundleYaml | AuthorFormat::SplitYaml => serde_yaml::to_string(&value)?,
    };
    std::fs::write(file_path, out)?;
    Ok(())
}

pub fn write_workflow_from_definition(
    dir: &Path,
    data: &Value,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    std::fs::create_dir_all(dir)?;

    let format = detect_format(dir).unwrap_or(AuthorFormat::SplitYaml);

    let data = super::normalize::normalize_mongo_oids(data.clone());
    let data_obj = data.as_object().ok_or("workflow data is not an object")?;

    let id = data_obj
        .get("id")
        .or_else(|| data_obj.get("_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let slug = data_obj.get("slug").and_then(|v| v.as_str()).unwrap_or("");
    let name = data_obj.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let workflow_type = data_obj
        .get("workflowType")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let version = data_obj
        .get("version")
        .and_then(|v| v.as_u64())
        .unwrap_or(1);
    let description = data_obj
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let fields = data_obj
        .get("fields")
        .cloned()
        .unwrap_or_else(|| Value::Array(Vec::new()));
    let stages = data_obj
        .get("stages")
        .cloned()
        .unwrap_or_else(|| Value::Array(Vec::new()));
    let gates = data_obj
        .get("gates")
        .cloned()
        .unwrap_or_else(|| Value::Array(Vec::new()));
    let initial_stage = data_obj
        .get("initialStage")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    match format {
        AuthorFormat::BundleJson => {
            let bundle = serde_json::json!({
                "meta": {
                    "id": id,
                    "slug": slug,
                    "name": name,
                    "workflowType": workflow_type,
                    "version": version,
                    "description": description
                },
                "schema": {
                    "fields": fields
                },
                "flow": {
                    "initialStage": initial_stage,
                    "stages": stages
                },
                "gates": gates
            });
            let path = dir.join("workflow.json");
            std::fs::write(&path, serde_json::to_string_pretty(&bundle)?)?;
        }
        AuthorFormat::BundleYaml => {
            let bundle = serde_json::json!({
                "meta": {
                    "id": id,
                    "slug": slug,
                    "name": name,
                    "workflowType": workflow_type,
                    "version": version,
                    "description": description
                },
                "schema": {
                    "fields": fields
                },
                "flow": {
                    "initialStage": initial_stage,
                    "stages": stages
                },
                "gates": gates
            });
            let path = dir.join("workflow.yaml");
            std::fs::write(&path, serde_yaml::to_string(&bundle)?)?;
        }
        AuthorFormat::SplitYaml => {
            let meta = serde_json::json!({
                "id": id,
                "name": name,
                "slug": slug,
                "workflowType": workflow_type,
                "version": version,
                "description": description
            });
            std::fs::write(dir.join("meta.yaml"), serde_yaml::to_string(&meta)?)?;

            let schema = serde_json::json!({
                "fields": fields
            });
            std::fs::write(dir.join("schema.yaml"), serde_yaml::to_string(&schema)?)?;

            // Pulled stages carry their `playbook` inline (mirrors the Mongo-flattened
            // `WorkflowStageDef` shape); split the split-YAML playbooks back into their
            // own file rather than duplicating them inline in flow.yaml.
            let (stages_for_flow, playbooks) = split_stage_playbooks(&stages);

            let flow = serde_json::json!({
                "initialStage": initial_stage,
                "stages": stages_for_flow
            });
            std::fs::write(dir.join("flow.yaml"), serde_yaml::to_string(&flow)?)?;

            if !gates.as_array().is_none_or(|a| a.is_empty()) {
                std::fs::write(dir.join("gates.yaml"), serde_yaml::to_string(&gates)?)?;
            } else {
                let gates_path = dir.join("gates.yaml");
                if gates_path.exists() {
                    let _ = std::fs::remove_file(gates_path);
                }
            }

            // Prefer the singular `playbook.yaml` name only if that's the file already
            // present locally (and `playbooks.yaml` isn't); otherwise use the canonical
            // plural name, matching `WorkflowDefinitionLoader::load_split_yaml`.
            let playbooks_path =
                if dir.join("playbook.yaml").is_file() && !dir.join("playbooks.yaml").is_file() {
                    dir.join("playbook.yaml")
                } else {
                    dir.join("playbooks.yaml")
                };
            if !playbooks.is_empty() {
                std::fs::write(
                    &playbooks_path,
                    serde_yaml::to_string(&Value::Object(playbooks))?,
                )?;
            } else if playbooks_path.exists() {
                let _ = std::fs::remove_file(&playbooks_path);
            }
        }
    }
    Ok(())
}

/// Extract each stage's inline `playbook` field into a stage-id-keyed map, returning
/// the stages with `playbook` removed. Used when writing the split-YAML layout so
/// `playbooks.yaml` (not `flow.yaml`) owns playbook data, matching how push reads it.
fn split_stage_playbooks(stages: &Value) -> (Value, serde_json::Map<String, Value>) {
    let mut playbooks = serde_json::Map::new();
    let stripped = match stages.as_array() {
        Some(arr) => Value::Array(
            arr.iter()
                .map(|stage| {
                    let mut stage = stage.clone();
                    if let Some(obj) = stage.as_object_mut() {
                        if let Some(playbook) = obj.remove("playbook") {
                            if !playbook.is_null() {
                                if let Some(stage_id) = obj.get("id").and_then(|v| v.as_str()) {
                                    playbooks.insert(stage_id.to_string(), playbook);
                                }
                            }
                        }
                    }
                    stage
                })
                .collect(),
        ),
        None => stages.clone(),
    };
    (stripped, playbooks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn setup_temp_dir() -> PathBuf {
        let temp_dir = std::env::current_dir()
            .unwrap()
            .join("target")
            .join(format!("test_wf_{}", uuid_like()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        temp_dir
    }

    fn uuid_like() -> String {
        uuid::Uuid::new_v4().to_string()
    }

    #[test]
    fn test_detect_format_split() {
        let dir = setup_temp_dir();
        std::fs::write(dir.join("meta.yaml"), "version: 1").unwrap();
        std::fs::write(dir.join("flow.yaml"), "stages: []").unwrap();

        assert_eq!(detect_format(&dir).unwrap(), AuthorFormat::SplitYaml);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn test_detect_format_bundle_yaml() {
        let dir = setup_temp_dir();
        let _ = std::fs::remove_file(dir.join("workflow.json"));
        std::fs::write(dir.join("workflow.yaml"), "meta: {}").unwrap();

        assert_eq!(detect_format(&dir).unwrap(), AuthorFormat::BundleYaml);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn test_detect_format_bundle_json() {
        let dir = setup_temp_dir();
        std::fs::write(dir.join("workflow.json"), "{}").unwrap();

        assert_eq!(detect_format(&dir).unwrap(), AuthorFormat::BundleJson);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn test_increment_version_split() {
        let dir = setup_temp_dir();
        let meta_path = dir.join("meta.yaml");
        std::fs::write(&meta_path, "version: 2\nname: Test").unwrap();

        let new_v = increment_workflow_version(&meta_path, AuthorFormat::SplitYaml).unwrap();
        assert_eq!(new_v, 3);

        let content = std::fs::read_to_string(&meta_path).unwrap();
        let val: Value = serde_yaml::from_str(&content).unwrap();
        assert_eq!(val.get("version").unwrap().as_u64().unwrap(), 3);
        assert_eq!(val.get("name").unwrap().as_str().unwrap(), "Test");

        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn test_write_workflow_id_split() {
        let dir = setup_temp_dir();
        let meta_path = dir.join("meta.yaml");
        std::fs::write(&meta_path, "version: 1\nname: Test").unwrap();

        write_workflow_id(&meta_path, AuthorFormat::SplitYaml, "my_db_id").unwrap();

        let content = std::fs::read_to_string(&meta_path).unwrap();
        let val: Value = serde_yaml::from_str(&content).unwrap();
        assert_eq!(val.get("id").unwrap().as_str().unwrap(), "my_db_id");
        assert_eq!(val.get("version").unwrap().as_u64().unwrap(), 1);

        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn test_write_and_load_workflow_from_definition() {
        let dir = setup_temp_dir();
        let def = json!({
            "id": "wf_123",
            "slug": "test-wf",
            "name": "Test Workflow",
            "workflowType": "test_type",
            "version": 5,
            "description": "A description",
            "initialStage": "start",
            "fields": [
                {
                    "key": "f1",
                    "label": "F1",
                    "type": "string",
                    "bag": "inputs"
                }
            ],
            "stages": [
                {
                    "id": "start",
                    "label": "Start",
                    "kind": "agent_task",
                    "agentSlug": "test-agent",
                    "expectedOutcome": "Complete the task",
                    "doneWhen": ["f1"]
                },
                {
                    "id": "done",
                    "label": "Done",
                    "kind": "terminal",
                    "doneWhen": "terminal"
                }
            ],
            "gates": []
        });

        write_workflow_from_definition(&dir, &def).unwrap();

        assert!(dir.join("meta.yaml").is_file());
        assert!(dir.join("schema.yaml").is_file());
        assert!(dir.join("flow.yaml").is_file());
        assert!(!dir.join("gates.yaml").is_file());

        let (bundle, _, format) = load_workflow_from_dir(&dir).unwrap();
        assert_eq!(format, AuthorFormat::SplitYaml);
        assert_eq!(
            bundle
                .get("meta")
                .unwrap()
                .get("slug")
                .unwrap()
                .as_str()
                .unwrap(),
            "test-wf"
        );
        assert_eq!(
            bundle
                .get("meta")
                .unwrap()
                .get("version")
                .unwrap()
                .as_u64()
                .unwrap(),
            5
        );
        assert_eq!(
            bundle
                .get("schema")
                .unwrap()
                .get("fields")
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            bundle
                .get("flow")
                .unwrap()
                .get("initialStage")
                .unwrap()
                .as_str()
                .unwrap(),
            "start"
        );

        std::fs::remove_dir_all(dir).unwrap();
    }

    /// Round-trips `playbooks.yaml` through `write_workflow_from_definition` (pull) and
    /// `load_workflow_from_dir` (push), the same asymmetry the bug report describes:
    /// push reads `playbooks.yaml` into `authorBundle.playbooks`, but pull previously
    /// dropped the field entirely when writing local files.
    #[test]
    fn test_pull_writes_and_push_reloads_playbooks_yaml() {
        let dir = setup_temp_dir();
        let def = json!({
            "id": "wf_456",
            "slug": "test-wf-playbook",
            "name": "Test Workflow With Playbook",
            "workflowType": "test_type",
            "version": 1,
            "initialStage": "collect_supplier",
            "fields": [
                {
                    "key": "supplierId",
                    "label": "Supplier ID",
                    "type": "string",
                    "bag": "inputs"
                }
            ],
            "stages": [
                {
                    "id": "collect_supplier",
                    "label": "Collect Supplier",
                    "kind": "agent_task",
                    "agentSlug": "test-agent",
                    "expectedOutcome": "Select a supplier",
                    "doneWhen": ["supplierId"],
                    "playbook": {
                        "replyPolicy": {
                            "tone": "professional",
                            "wordLimit": 120
                        },
                        "constraints": ["Never invent a supplier ID."],
                        "assistantBrief": "Guide the user to select a supplier."
                    }
                },
                {
                    "id": "done",
                    "label": "Done",
                    "kind": "terminal",
                    "doneWhen": "terminal"
                }
            ],
            "gates": []
        });

        write_workflow_from_definition(&dir, &def).unwrap();

        // Pull must materialize playbooks.yaml locally, mirroring flow.yaml/meta.yaml.
        assert!(dir.join("playbooks.yaml").is_file());

        // flow.yaml should not duplicate the playbook inline once it's split out.
        let flow_content = std::fs::read_to_string(dir.join("flow.yaml")).unwrap();
        let flow_value: Value = serde_yaml::from_str(&flow_content).unwrap();
        let collect_stage = flow_value
            .get("stages")
            .and_then(|s| s.as_array())
            .and_then(|arr| {
                arr.iter()
                    .find(|s| s.get("id").and_then(|v| v.as_str()) == Some("collect_supplier"))
            })
            .unwrap();
        assert!(collect_stage.get("playbook").is_none());

        let playbooks_content = std::fs::read_to_string(dir.join("playbooks.yaml")).unwrap();
        let playbooks_value: Value = serde_yaml::from_str(&playbooks_content).unwrap();
        assert_eq!(
            playbooks_value
                .get("collect_supplier")
                .and_then(|p| p.get("assistantBrief"))
                .and_then(|v| v.as_str()),
            Some("Guide the user to select a supplier.")
        );

        // Push-side loader must recombine playbooks.yaml back onto the stage, proving
        // the local files pull writes are exactly what push reads back out.
        let (bundle, _, format) = load_workflow_from_dir(&dir).unwrap();
        assert_eq!(format, AuthorFormat::SplitYaml);
        assert_eq!(
            bundle
                .get("playbooks")
                .and_then(|p| p.get("collect_supplier"))
                .and_then(|p| p.get("assistantBrief"))
                .and_then(|v| v.as_str()),
            Some("Guide the user to select a supplier.")
        );
        assert_eq!(
            bundle
                .get("playbooks")
                .and_then(|p| p.get("collect_supplier"))
                .and_then(|p| p.get("replyPolicy"))
                .and_then(|p| p.get("wordLimit"))
                .and_then(|v| v.as_u64()),
            Some(120)
        );

        std::fs::remove_dir_all(dir).unwrap();
    }

    /// When the remote definition has no playbooks, pull must remove a stale local
    /// `playbooks.yaml` rather than leaving out-of-date data behind (mirrors gates.yaml).
    #[test]
    fn test_pull_removes_stale_playbooks_yaml_when_absent_remotely() {
        let dir = setup_temp_dir();
        std::fs::write(
            dir.join("playbooks.yaml"),
            "collect_supplier:\n  assistantBrief: \"stale\"\n",
        )
        .unwrap();

        let def = json!({
            "id": "wf_789",
            "slug": "test-wf-no-playbook",
            "name": "Test Workflow Without Playbook",
            "workflowType": "test_type",
            "version": 1,
            "initialStage": "start",
            "fields": [],
            "stages": [
                { "id": "start", "label": "Start", "kind": "terminal", "doneWhen": "terminal" }
            ],
            "gates": []
        });

        write_workflow_from_definition(&dir, &def).unwrap();

        assert!(!dir.join("playbooks.yaml").is_file());

        std::fs::remove_dir_all(dir).unwrap();
    }

    fn transitions_sample_def() -> Value {
        json!({
            "id": "wf_transitions_1",
            "slug": "transitions-round-trip",
            "name": "Transitions Round Trip",
            "workflowType": "test_type",
            "version": 1,
            "initialStage": "collect",
            "fields": [
                {
                    "key": "vip",
                    "label": "VIP",
                    "type": "boolean",
                    "bag": "inputs"
                }
            ],
            "stages": [
                {
                    "id": "collect",
                    "label": "Collect",
                    "kind": "collect",
                    "hitlSlug": "transitions-form",
                    "doneWhen": ["vip"],
                    "transitions": [
                        {
                            "target": "express",
                            "when": {
                                "eq": { "field": "inputs.vip", "value": true }
                            }
                        }
                    ],
                    "next": "done"
                },
                {
                    "id": "express",
                    "label": "Express",
                    "kind": "review",
                    "hitlSlug": "transitions-express-form",
                    "doneWhen": "validator_pass",
                    "next": "done"
                },
                {
                    "id": "done",
                    "label": "Done",
                    "kind": "terminal",
                    "terminal": true,
                    "doneWhen": "terminal"
                }
            ],
            "gates": []
        })
    }

    /// Split YAML pull must preserve `transitions` on stages in `flow.yaml` and reload them
    /// for push — the same asymmetry that previously dropped playbooks/gates fields.
    #[test]
    fn test_transitions_round_trip_through_pull_write() {
        let dir = setup_temp_dir();
        let def = transitions_sample_def();

        write_workflow_from_definition(&dir, &def).unwrap();

        let flow_content = std::fs::read_to_string(dir.join("flow.yaml")).unwrap();
        let flow_value: Value = serde_yaml::from_str(&flow_content).unwrap();
        let collect_stage = flow_value
            .get("stages")
            .and_then(|s| s.as_array())
            .and_then(|arr| {
                arr.iter()
                    .find(|s| s.get("id").and_then(|v| v.as_str()) == Some("collect"))
            })
            .expect("collect stage in flow.yaml");
        let transitions = collect_stage
            .get("transitions")
            .and_then(|t| t.as_array())
            .expect("transitions on collect stage");
        assert_eq!(transitions.len(), 1);
        assert_eq!(
            transitions[0].get("target").and_then(|v| v.as_str()),
            Some("express")
        );
        assert_eq!(
            transitions[0]
                .get("when")
                .and_then(|w| w.get("eq"))
                .and_then(|eq| eq.get("field"))
                .and_then(|v| v.as_str()),
            Some("inputs.vip")
        );

        let (bundle, _, format) = load_workflow_from_dir(&dir).unwrap();
        assert_eq!(format, AuthorFormat::SplitYaml);
        let loaded_transitions = bundle
            .get("flow")
            .and_then(|f| f.get("stages"))
            .and_then(|s| s.as_array())
            .and_then(|arr| {
                arr.iter()
                    .find(|s| s.get("id").and_then(|v| v.as_str()) == Some("collect"))
            })
            .and_then(|s| s.get("transitions"))
            .and_then(|t| t.as_array())
            .expect("transitions survive load_workflow_from_dir");
        assert_eq!(loaded_transitions.len(), 1);
        assert_eq!(
            loaded_transitions[0].get("target").and_then(|v| v.as_str()),
            Some("express")
        );

        std::fs::remove_dir_all(dir).unwrap();
    }

    /// Bundle YAML pull/write must round-trip `transitions` inside `workflow.yaml`.
    #[test]
    fn test_transitions_bundle_yaml_round_trip() {
        let dir = setup_temp_dir();
        std::fs::write(dir.join("workflow.yaml"), "meta:\n  slug: placeholder\n").unwrap();

        let def = transitions_sample_def();
        write_workflow_from_definition(&dir, &def).unwrap();

        assert!(dir.join("workflow.yaml").is_file());
        assert!(!dir.join("flow.yaml").is_file());

        let bundle_content = std::fs::read_to_string(dir.join("workflow.yaml")).unwrap();
        let bundle_value: Value = serde_yaml::from_str(&bundle_content).unwrap();
        let transitions = bundle_value
            .get("flow")
            .and_then(|f| f.get("stages"))
            .and_then(|s| s.as_array())
            .and_then(|arr| {
                arr.iter()
                    .find(|s| s.get("id").and_then(|v| v.as_str()) == Some("collect"))
            })
            .and_then(|s| s.get("transitions"))
            .and_then(|t| t.as_array())
            .expect("transitions in bundle workflow.yaml");
        assert_eq!(transitions.len(), 1);
        assert_eq!(
            transitions[0].get("target").and_then(|v| v.as_str()),
            Some("express")
        );

        let (bundle, _, format) = load_workflow_from_dir(&dir).unwrap();
        assert_eq!(format, AuthorFormat::BundleYaml);
        let loaded = bundle
            .get("flow")
            .and_then(|f| f.get("stages"))
            .and_then(|s| s.as_array())
            .and_then(|arr| {
                arr.iter()
                    .find(|s| s.get("id").and_then(|v| v.as_str()) == Some("collect"))
            })
            .and_then(|s| s.get("transitions"))
            .and_then(|t| t.as_array())
            .expect("transitions survive bundle reload");
        assert_eq!(loaded.len(), 1);
        assert_eq!(
            loaded[0].get("target").and_then(|v| v.as_str()),
            Some("express")
        );

        std::fs::remove_dir_all(dir).unwrap();
    }

    fn required_from_stage_sample_def() -> Value {
        json!({
            "id": "wf_required_from_stage_1",
            "slug": "required-from-stage-round-trip",
            "name": "Required From Stage Round Trip",
            "workflowType": "test_type",
            "version": 1,
            "initialStage": "collect",
            "fields": [
                {
                    "key": "vip",
                    "label": "VIP",
                    "type": "boolean",
                    "bag": "inputs"
                },
                {
                    "key": "expressReason",
                    "label": "Express review reason",
                    "type": "string",
                    "bag": "inputs",
                    "requiredFromStage": "express_review"
                }
            ],
            "stages": [
                {
                    "id": "collect",
                    "label": "Collect",
                    "kind": "collect",
                    "hitlSlug": "required-from-stage-form",
                    "doneWhen": ["vip"],
                    "transitions": [
                        {
                            "target": "express_review",
                            "when": {
                                "eq": { "field": "inputs.vip", "value": true }
                            }
                        }
                    ],
                    "next": "done"
                },
                {
                    "id": "express_review",
                    "label": "Express review",
                    "kind": "collect",
                    "hitlSlug": "required-from-stage-express-form",
                    "doneWhen": ["expressReason"],
                    "next": "done"
                },
                {
                    "id": "done",
                    "label": "Done",
                    "kind": "terminal",
                    "terminal": true,
                    "doneWhen": "terminal"
                }
            ],
            "gates": []
        })
    }

    /// Split YAML pull must preserve `requiredFromStage` on fields in `schema.yaml`.
    #[test]
    fn test_required_from_stage_round_trip_through_pull_write() {
        let dir = setup_temp_dir();
        let def = required_from_stage_sample_def();

        write_workflow_from_definition(&dir, &def).unwrap();

        let schema_content = std::fs::read_to_string(dir.join("schema.yaml")).unwrap();
        let schema_value: Value = serde_yaml::from_str(&schema_content).unwrap();
        let express_field = schema_value
            .get("fields")
            .and_then(|f| f.as_array())
            .and_then(|arr| {
                arr.iter()
                    .find(|f| f.get("key").and_then(|v| v.as_str()) == Some("expressReason"))
            })
            .expect("expressReason field in schema.yaml");
        assert_eq!(
            express_field
                .get("requiredFromStage")
                .and_then(|v| v.as_str()),
            Some("express_review")
        );

        let (bundle, _, format) = load_workflow_from_dir(&dir).unwrap();
        assert_eq!(format, AuthorFormat::SplitYaml);
        let loaded_field = bundle
            .get("schema")
            .and_then(|s| s.get("fields"))
            .and_then(|f| f.as_array())
            .and_then(|arr| {
                arr.iter()
                    .find(|f| f.get("key").and_then(|v| v.as_str()) == Some("expressReason"))
            })
            .expect("expressReason survives load_workflow_from_dir");
        assert_eq!(
            loaded_field
                .get("requiredFromStage")
                .and_then(|v| v.as_str()),
            Some("express_review")
        );

        std::fs::remove_dir_all(dir).unwrap();
    }

    /// Bundle YAML pull/write must round-trip `requiredFromStage` inside `workflow.yaml`.
    #[test]
    fn test_required_from_stage_bundle_yaml_round_trip() {
        let dir = setup_temp_dir();
        std::fs::write(dir.join("workflow.yaml"), "meta:\n  slug: placeholder\n").unwrap();

        let def = required_from_stage_sample_def();
        write_workflow_from_definition(&dir, &def).unwrap();

        assert!(dir.join("workflow.yaml").is_file());
        assert!(!dir.join("schema.yaml").is_file());

        let bundle_content = std::fs::read_to_string(dir.join("workflow.yaml")).unwrap();
        let bundle_value: Value = serde_yaml::from_str(&bundle_content).unwrap();
        let express_field = bundle_value
            .get("schema")
            .and_then(|s| s.get("fields"))
            .and_then(|f| f.as_array())
            .and_then(|arr| {
                arr.iter()
                    .find(|f| f.get("key").and_then(|v| v.as_str()) == Some("expressReason"))
            })
            .expect("requiredFromStage in bundle workflow.yaml");
        assert_eq!(
            express_field
                .get("requiredFromStage")
                .and_then(|v| v.as_str()),
            Some("express_review")
        );

        let (bundle, _, format) = load_workflow_from_dir(&dir).unwrap();
        assert_eq!(format, AuthorFormat::BundleYaml);
        let loaded_field = bundle
            .get("schema")
            .and_then(|s| s.get("fields"))
            .and_then(|f| f.as_array())
            .and_then(|arr| {
                arr.iter()
                    .find(|f| f.get("key").and_then(|v| v.as_str()) == Some("expressReason"))
            })
            .expect("requiredFromStage survives bundle reload");
        assert_eq!(
            loaded_field
                .get("requiredFromStage")
                .and_then(|v| v.as_str()),
            Some("express_review")
        );

        std::fs::remove_dir_all(dir).unwrap();
    }

    fn reset_sample_def() -> Value {
        json!({
            "id": "wf_reset_1",
            "slug": "reset-round-trip",
            "name": "Reset Round Trip",
            "workflowType": "test_type",
            "version": 1,
            "initialStage": "collect_line_items",
            "fields": [
                {
                    "key": "lineItems",
                    "label": "Line items",
                    "type": "array",
                    "bag": "inputs",
                    "requiredFromStage": "collect_line_items"
                },
                {
                    "key": "reviewConfirmed",
                    "label": "Review confirmed",
                    "type": "boolean",
                    "bag": "inputs",
                    "requiredFromStage": "review_summary"
                }
            ],
            "stages": [
                {
                    "id": "collect_line_items",
                    "label": "Collect line items",
                    "kind": "collect",
                    "hitlSlug": "reset-round-trip-lines-form",
                    "doneWhen": ["lineItems"],
                    "next": "review_summary"
                },
                {
                    "id": "review_summary",
                    "label": "Review summary",
                    "kind": "review",
                    "hitlSlug": "reset-round-trip-review-form",
                    "doneWhen": ["reviewConfirmed"],
                    "transitions": [
                        {
                            "target": "submit",
                            "when": {
                                "eq": { "field": "inputs.reviewConfirmed", "value": true }
                            }
                        },
                        {
                            "target": "collect_line_items",
                            "when": {
                                "eq": { "field": "inputs.reviewConfirmed", "value": false }
                            },
                            "reset": ["reviewConfirmed", "lineItems"]
                        }
                    ],
                    "next": "submit"
                },
                {
                    "id": "submit",
                    "label": "Submitted",
                    "kind": "terminal",
                    "terminal": true,
                    "doneWhen": "terminal"
                }
            ],
            "gates": []
        })
    }

    /// Split YAML pull must preserve `reset` on transition rules in `flow.yaml`.
    #[test]
    fn test_reset_round_trip_through_pull_write() {
        let dir = setup_temp_dir();
        let def = reset_sample_def();

        write_workflow_from_definition(&dir, &def).unwrap();

        let flow_content = std::fs::read_to_string(dir.join("flow.yaml")).unwrap();
        let flow_value: Value = serde_yaml::from_str(&flow_content).unwrap();
        let review_stage = flow_value
            .get("stages")
            .and_then(|s| s.as_array())
            .and_then(|arr| {
                arr.iter()
                    .find(|s| s.get("id").and_then(|v| v.as_str()) == Some("review_summary"))
            })
            .expect("review_summary stage in flow.yaml");
        let transitions = review_stage
            .get("transitions")
            .and_then(|t| t.as_array())
            .expect("transitions on review_summary stage");
        assert_eq!(transitions.len(), 2);
        let reject_transition = &transitions[1];
        assert_eq!(
            reject_transition.get("target").and_then(|v| v.as_str()),
            Some("collect_line_items")
        );
        let reset = reject_transition
            .get("reset")
            .and_then(|r| r.as_array())
            .expect("reset on reject transition");
        assert_eq!(reset.len(), 2);
        assert_eq!(reset[0].as_str(), Some("reviewConfirmed"));
        assert_eq!(reset[1].as_str(), Some("lineItems"));

        let (bundle, _, format) = load_workflow_from_dir(&dir).unwrap();
        assert_eq!(format, AuthorFormat::SplitYaml);
        let loaded_reset = bundle
            .get("flow")
            .and_then(|f| f.get("stages"))
            .and_then(|s| s.as_array())
            .and_then(|arr| {
                arr.iter()
                    .find(|s| s.get("id").and_then(|v| v.as_str()) == Some("review_summary"))
            })
            .and_then(|s| s.get("transitions"))
            .and_then(|t| t.as_array())
            .and_then(|arr| arr.get(1))
            .and_then(|t| t.get("reset"))
            .and_then(|r| r.as_array())
            .expect("reset survives load_workflow_from_dir");
        assert_eq!(loaded_reset.len(), 2);
        assert_eq!(loaded_reset[0].as_str(), Some("reviewConfirmed"));
        assert_eq!(loaded_reset[1].as_str(), Some("lineItems"));

        std::fs::remove_dir_all(dir).unwrap();
    }

    /// Bundle YAML pull/write must round-trip `reset` inside `transitions` in `workflow.yaml`.
    #[test]
    fn test_reset_bundle_yaml_round_trip() {
        let dir = setup_temp_dir();
        std::fs::write(dir.join("workflow.yaml"), "meta:\n  slug: placeholder\n").unwrap();

        let def = reset_sample_def();
        write_workflow_from_definition(&dir, &def).unwrap();

        assert!(dir.join("workflow.yaml").is_file());
        assert!(!dir.join("flow.yaml").is_file());

        let bundle_content = std::fs::read_to_string(dir.join("workflow.yaml")).unwrap();
        let bundle_value: Value = serde_yaml::from_str(&bundle_content).unwrap();
        let reset = bundle_value
            .get("flow")
            .and_then(|f| f.get("stages"))
            .and_then(|s| s.as_array())
            .and_then(|arr| {
                arr.iter()
                    .find(|s| s.get("id").and_then(|v| v.as_str()) == Some("review_summary"))
            })
            .and_then(|s| s.get("transitions"))
            .and_then(|t| t.as_array())
            .and_then(|arr| arr.get(1))
            .and_then(|t| t.get("reset"))
            .and_then(|r| r.as_array())
            .expect("reset in bundle workflow.yaml");
        assert_eq!(reset.len(), 2);
        assert_eq!(reset[0].as_str(), Some("reviewConfirmed"));
        assert_eq!(reset[1].as_str(), Some("lineItems"));

        let (bundle, _, format) = load_workflow_from_dir(&dir).unwrap();
        assert_eq!(format, AuthorFormat::BundleYaml);
        let loaded_reset = bundle
            .get("flow")
            .and_then(|f| f.get("stages"))
            .and_then(|s| s.as_array())
            .and_then(|arr| {
                arr.iter()
                    .find(|s| s.get("id").and_then(|v| v.as_str()) == Some("review_summary"))
            })
            .and_then(|s| s.get("transitions"))
            .and_then(|t| t.as_array())
            .and_then(|arr| arr.get(1))
            .and_then(|t| t.get("reset"))
            .and_then(|r| r.as_array())
            .expect("reset survives bundle reload");
        assert_eq!(loaded_reset.len(), 2);
        assert_eq!(loaded_reset[0].as_str(), Some("reviewConfirmed"));
        assert_eq!(loaded_reset[1].as_str(), Some("lineItems"));

        std::fs::remove_dir_all(dir).unwrap();
    }

    fn named_gates_sample_def() -> Value {
        json!({
            "id": "wf_gates_1",
            "slug": "gates-round-trip",
            "name": "Gates Round Trip",
            "workflowType": "test_type",
            "version": 1,
            "initialStage": "collect_request",
            "fields": [
                {
                    "key": "totalAmount",
                    "label": "Total amount",
                    "type": "number",
                    "bag": "inputs",
                    "requiredFromStage": "collect_request"
                },
                {
                    "key": "managerApprovalCode",
                    "label": "Manager approval code",
                    "type": "string",
                    "bag": "inputs",
                    "requiredFromStage": "await_approval"
                }
            ],
            "stages": [
                {
                    "id": "collect_request",
                    "label": "Collect request",
                    "kind": "collect",
                    "hitlSlug": "gates-round-trip-request-form",
                    "doneWhen": ["totalAmount"],
                    "transitions": [
                        {
                            "target": "await_approval",
                            "when": { "gate": "requires_manager_approval" }
                        }
                    ],
                    "next": "submit"
                },
                {
                    "id": "await_approval",
                    "label": "Await manager approval",
                    "kind": "collect",
                    "hitlSlug": "gates-round-trip-approval-form",
                    "doneWhen": ["managerApprovalCode"],
                    "next": "submit"
                },
                {
                    "id": "submit",
                    "label": "Submitted",
                    "kind": "terminal",
                    "terminal": true,
                    "doneWhen": "terminal"
                }
            ],
            "gates": [
                {
                    "id": "is_high_value",
                    "name": "High Value Requisition",
                    "when": {
                        "eq": { "field": "inputs.totalAmount", "value": 10000 }
                    }
                },
                {
                    "id": "requires_manager_approval",
                    "name": "Requires Manager Approval",
                    "when": {
                        "all": [
                            { "gate": "is_high_value" },
                            { "not": { "present": "inputs.managerApprovalCode" } }
                        ]
                    }
                }
            ]
        })
    }

    /// Split YAML pull must materialize `gates.yaml` and preserve transition `gate` refs
    /// plus composed gate `when` trees through reload.
    #[test]
    fn test_gates_yaml_round_trip_through_pull_write() {
        let dir = setup_temp_dir();
        let def = named_gates_sample_def();

        write_workflow_from_definition(&dir, &def).unwrap();

        assert!(
            dir.join("gates.yaml").is_file(),
            "gates.yaml must be written"
        );

        let gates_content = std::fs::read_to_string(dir.join("gates.yaml")).unwrap();
        let gates_value: Value = serde_yaml::from_str(&gates_content).unwrap();
        let gates = gates_value.as_array().expect("gates.yaml root is an array");
        assert_eq!(gates.len(), 2);
        let composed_gate = gates
            .iter()
            .find(|g| g.get("id").and_then(|v| v.as_str()) == Some("requires_manager_approval"))
            .expect("requires_manager_approval gate in gates.yaml");
        let all_conditions = composed_gate
            .get("when")
            .and_then(|w| w.get("all"))
            .and_then(|a| a.as_array())
            .expect("composed gate when.all in gates.yaml");
        assert_eq!(
            all_conditions[0].get("gate").and_then(|v| v.as_str()),
            Some("is_high_value")
        );

        let flow_content = std::fs::read_to_string(dir.join("flow.yaml")).unwrap();
        let flow_value: Value = serde_yaml::from_str(&flow_content).unwrap();
        let collect_stage = flow_value
            .get("stages")
            .and_then(|s| s.as_array())
            .and_then(|arr| {
                arr.iter()
                    .find(|s| s.get("id").and_then(|v| v.as_str()) == Some("collect_request"))
            })
            .expect("collect_request stage in flow.yaml");
        let transition_gate = collect_stage
            .get("transitions")
            .and_then(|t| t.as_array())
            .and_then(|arr| arr.first())
            .and_then(|t| t.get("when"))
            .and_then(|w| w.get("gate"))
            .and_then(|v| v.as_str())
            .expect("transition when.gate in flow.yaml");
        assert_eq!(transition_gate, "requires_manager_approval");

        let (bundle, _, format) = load_workflow_from_dir(&dir).unwrap();
        assert_eq!(format, AuthorFormat::SplitYaml);
        let loaded_gates = bundle
            .get("gates")
            .and_then(|g| g.as_array())
            .expect("gates survive load_workflow_from_dir");
        assert_eq!(loaded_gates.len(), 2);
        let loaded_transition_gate = bundle
            .get("flow")
            .and_then(|f| f.get("stages"))
            .and_then(|s| s.as_array())
            .and_then(|arr| {
                arr.iter()
                    .find(|s| s.get("id").and_then(|v| v.as_str()) == Some("collect_request"))
            })
            .and_then(|s| s.get("transitions"))
            .and_then(|t| t.as_array())
            .and_then(|arr| arr.first())
            .and_then(|t| t.get("when"))
            .and_then(|w| w.get("gate"))
            .and_then(|v| v.as_str())
            .expect("transition gate ref survives load_workflow_from_dir");
        assert_eq!(loaded_transition_gate, "requires_manager_approval");
        let loaded_composed = loaded_gates
            .iter()
            .find(|g| g.get("id").and_then(|v| v.as_str()) == Some("requires_manager_approval"))
            .and_then(|g| g.get("when"))
            .and_then(|w| w.get("all"))
            .and_then(|a| a.as_array())
            .and_then(|arr| arr.first())
            .and_then(|c| c.get("gate"))
            .and_then(|v| v.as_str())
            .expect("composed gate ref survives load_workflow_from_dir");
        assert_eq!(loaded_composed, "is_high_value");

        std::fs::remove_dir_all(dir).unwrap();
    }

    /// Bundle YAML pull/write must round-trip the top-level `gates` array and nested `gate:` refs.
    #[test]
    fn test_gates_bundle_yaml_round_trip() {
        let dir = setup_temp_dir();
        std::fs::write(dir.join("workflow.yaml"), "meta:\n  slug: placeholder\n").unwrap();

        let def = named_gates_sample_def();
        write_workflow_from_definition(&dir, &def).unwrap();

        assert!(dir.join("workflow.yaml").is_file());
        assert!(!dir.join("gates.yaml").is_file());

        let bundle_content = std::fs::read_to_string(dir.join("workflow.yaml")).unwrap();
        let bundle_value: Value = serde_yaml::from_str(&bundle_content).unwrap();
        let gates = bundle_value
            .get("gates")
            .and_then(|g| g.as_array())
            .expect("gates in bundle workflow.yaml");
        assert_eq!(gates.len(), 2);
        let nested_gate_ref = gates
            .iter()
            .find(|g| g.get("id").and_then(|v| v.as_str()) == Some("requires_manager_approval"))
            .and_then(|g| g.get("when"))
            .and_then(|w| w.get("all"))
            .and_then(|a| a.as_array())
            .and_then(|arr| arr.first())
            .and_then(|c| c.get("gate"))
            .and_then(|v| v.as_str())
            .expect("nested gate ref in bundle workflow.yaml");
        assert_eq!(nested_gate_ref, "is_high_value");

        let (bundle, _, format) = load_workflow_from_dir(&dir).unwrap();
        assert_eq!(format, AuthorFormat::BundleYaml);
        let loaded_gates = bundle
            .get("gates")
            .and_then(|g| g.as_array())
            .expect("gates survive bundle reload");
        assert_eq!(loaded_gates.len(), 2);
        let loaded_transition_gate = bundle
            .get("flow")
            .and_then(|f| f.get("stages"))
            .and_then(|s| s.as_array())
            .and_then(|arr| {
                arr.iter()
                    .find(|s| s.get("id").and_then(|v| v.as_str()) == Some("collect_request"))
            })
            .and_then(|s| s.get("transitions"))
            .and_then(|t| t.as_array())
            .and_then(|arr| arr.first())
            .and_then(|t| t.get("when"))
            .and_then(|w| w.get("gate"))
            .and_then(|v| v.as_str())
            .expect("transition gate ref survives bundle reload");
        assert_eq!(loaded_transition_gate, "requires_manager_approval");

        std::fs::remove_dir_all(dir).unwrap();
    }

    /// When the remote definition has no gates, pull must remove a stale local `gates.yaml`.
    #[test]
    fn test_pull_removes_stale_gates_yaml_when_absent_remotely() {
        let dir = setup_temp_dir();
        std::fs::write(
            dir.join("gates.yaml"),
            "- id: stale_gate\n  name: Stale\n  when:\n    present: inputs.totalAmount\n",
        )
        .unwrap();

        let def = json!({
            "id": "wf_no_gates",
            "slug": "test-wf-no-gates",
            "name": "Test Workflow Without Gates",
            "workflowType": "test_type",
            "version": 1,
            "initialStage": "start",
            "fields": [],
            "stages": [
                { "id": "start", "label": "Start", "kind": "terminal", "doneWhen": "terminal" }
            ],
            "gates": []
        });

        write_workflow_from_definition(&dir, &def).unwrap();

        assert!(!dir.join("gates.yaml").is_file());

        std::fs::remove_dir_all(dir).unwrap();
    }
}
