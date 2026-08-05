use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

/// How the definition was authored before push.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceFormat {
    SplitYaml,
    BundleYaml,
    BundleJson,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowDefinition {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none", default)]
    pub id: Option<String>,
    pub slug: String,
    pub name: String,
    pub workflow_type: String,
    pub version: u32,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub description: Option<String>,
    pub fields: Vec<WorkflowFieldDef>,
    pub stages: Vec<WorkflowStageDef>,
    #[serde(default)]
    pub gates: Vec<WorkflowGateDef>,
    pub initial_stage: String,
    pub source_format: SourceFormat,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowFieldDef {
    pub key: String,
    pub label: String,
    #[serde(rename = "type")]
    pub field_type: WorkflowFieldType,
    pub bag: FieldBag,
    #[serde(default)]
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub required_from_stage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub validation: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub hitl_field_id: Option<String>,
    #[serde(default)]
    pub phi: bool,
    #[serde(default)]
    pub redact_in_prompt: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FieldBag {
    Inputs,
    Artifacts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkflowFieldType {
    String,
    Number,
    Boolean,
    Array,
    Object,
}

/// Represents a transition rule from one stage to another based on a condition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransitionRule {
    pub target: String,
    pub when: Condition,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reset: Option<Vec<String>>,
}

/// Rich, JSON-safe conditional logic evaluation tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Condition {
    #[serde(rename = "present")]
    Present(String),
    #[serde(rename = "absent")]
    Absent(String),
    #[serde(rename = "eq")]
    Eq { field: String, value: Value },
    #[serde(rename = "all")]
    All(Vec<Condition>),
    #[serde(rename = "any")]
    Any(Vec<Condition>),
    #[serde(rename = "not")]
    Not(Box<Condition>),
    #[serde(rename = "gate")]
    Gate(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowStageDef {
    pub id: String,
    pub label: String,
    pub kind: WorkflowStageKind,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub hitl_slug: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub agent_slug: Option<String>,
    pub done_when: StageDoneWhen,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub expected_outcome: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub next: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub back_to: Option<String>,
    #[serde(default)]
    pub transitions: Vec<TransitionRule>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub playbook: Option<Value>,
    #[serde(default)]
    pub is_terminal: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStageKind {
    Collect,
    AgentTask,
    Review,
    Terminal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StageDoneWhen {
    RequiredInputs(Vec<String>),
    ValidatorPass,
    Terminal,
}

impl StageDoneWhen {
    pub fn field_keys(&self) -> Option<&[String]> {
        match self {
            Self::RequiredInputs(keys) => Some(keys),
            _ => None,
        }
    }
}

impl<'de> Deserialize<'de> for StageDoneWhen {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        match value {
            Value::Array(items) => {
                let keys: Vec<String> = items
                    .into_iter()
                    .map(|v| match v {
                        Value::String(s) => Ok(s),
                        other => Err(serde::de::Error::custom(format!(
                            "doneWhen array items must be strings, got {other}"
                        ))),
                    })
                    .collect::<Result<_, D::Error>>()?;
                if keys.is_empty() {
                    return Err(serde::de::Error::custom("doneWhen array must not be empty"));
                }
                Ok(Self::RequiredInputs(keys))
            }
            Value::String(s) => match s.as_str() {
                "validator_pass" => Ok(Self::ValidatorPass),
                "terminal" => Ok(Self::Terminal),
                other => Err(serde::de::Error::custom(format!(
                    "unknown doneWhen string: {other}"
                ))),
            },
            other => Err(serde::de::Error::custom(format!(
                "doneWhen must be array or string, got {other}"
            ))),
        }
    }
}

impl Serialize for StageDoneWhen {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::RequiredInputs(keys) => keys.serialize(serializer),
            Self::ValidatorPass => serializer.serialize_str("validator_pass"),
            Self::Terminal => serializer.serialize_str("terminal"),
        }
    }
}

/// Reusable named predicate defined in `gates.yaml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowGateDef {
    pub id: String,
    pub name: String,
    pub when: Condition,
}

/// Author bundle shape (meta + schema + flow + gates) before flattening.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AuthorBundle {
    pub meta: AuthorMeta,
    pub schema: AuthorSchemaSection,
    pub flow: AuthorFlow,
    #[serde(default)]
    pub gates: Vec<WorkflowGateDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AuthorMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub slug: String,
    pub name: String,
    pub workflow_type: String,
    pub version: u32,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AuthorSchemaSection {
    pub fields: Vec<WorkflowFieldDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AuthorFlow {
    pub initial_stage: String,
    pub stages: Vec<AuthorStage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AuthorStage {
    pub id: String,
    pub label: String,
    pub kind: WorkflowStageKind,
    #[serde(default)]
    pub hitl_slug: Option<String>,
    #[serde(default)]
    pub agent_slug: Option<String>,
    pub done_when: StageDoneWhen,
    #[serde(default)]
    pub expected_outcome: Option<String>,
    #[serde(default)]
    pub next: Option<String>,
    #[serde(default)]
    pub back_to: Option<String>,
    #[serde(default)]
    pub transitions: Vec<TransitionRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub playbook: Option<Value>,
    #[serde(default)]
    pub terminal: bool,
}

impl AuthorStage {
    pub fn into_stage_def(self) -> WorkflowStageDef {
        let is_terminal = self.terminal || matches!(self.kind, WorkflowStageKind::Terminal);
        WorkflowStageDef {
            id: self.id,
            label: self.label,
            kind: self.kind,
            hitl_slug: self.hitl_slug,
            agent_slug: self.agent_slug,
            done_when: self.done_when,
            expected_outcome: self.expected_outcome,
            next: self.next,
            back_to: self.back_to,
            transitions: self.transitions,
            playbook: self.playbook,
            is_terminal,
        }
    }
}

impl AuthorBundle {
    pub fn into_definition(self, source_format: SourceFormat) -> WorkflowDefinition {
        WorkflowDefinition {
            id: self.meta.id,
            slug: self.meta.slug,
            name: self.meta.name,
            workflow_type: self.meta.workflow_type,
            version: self.meta.version,
            description: self.meta.description,
            fields: self.schema.fields,
            stages: self
                .flow
                .stages
                .into_iter()
                .map(AuthorStage::into_stage_def)
                .collect(),
            gates: self.gates,
            initial_stage: self.flow.initial_stage,
            source_format,
            created_at: None,
            updated_at: None,
        }
    }
}

impl WorkflowDefinition {
    pub fn from_mongo_value(value: Value) -> crate::error::Result<Self> {
        let value = crate::workflow::normalize::flatten_mongo_extended_json(value);
        serde_json::from_value(value).map_err(|e| {
            crate::error::SmritiError::InvalidResponse(format!(
                "failed to parse WorkflowDefinition from Mongo: {e}"
            ))
        })
    }

    pub fn stage_by_id(&self, id: &str) -> Option<&WorkflowStageDef> {
        self.stages.iter().find(|s| s.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_done_when_deserializes_array_and_string() {
        let array: StageDoneWhen =
            serde_json::from_value(serde_json::json!(["vendorId", "lineItems"])).unwrap();
        assert_eq!(
            array,
            StageDoneWhen::RequiredInputs(vec!["vendorId".into(), "lineItems".into()])
        );

        let pass: StageDoneWhen =
            serde_json::from_value(serde_json::json!("validator_pass")).unwrap();
        assert_eq!(pass, StageDoneWhen::ValidatorPass);
    }
}
