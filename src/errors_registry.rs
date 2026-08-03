use serde::Serialize;
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ErrorCodeDoc {
    pub code: String,
    pub domain: String,
    pub severity: Option<String>,
    pub exit_code: Option<u8>,
    pub description: String,
    pub remediation: Vec<String>,
}

static REGISTRY: OnceLock<HashMap<String, ErrorCodeDoc>> = OnceLock::new();

const EMBEDDED_DOCS: &[(&str, &str)] = &[
    ("config", include_str!("../docs/errors/config.md")),
    ("validation", include_str!("../docs/errors/validation.md")),
    ("workflow", include_str!("../docs/errors/workflow.md")),
    ("push", include_str!("../docs/errors/push.md")),
];

fn build_registry() -> HashMap<String, ErrorCodeDoc> {
    let mut map = HashMap::new();
    for (domain, content) in EMBEDDED_DOCS {
        for doc in parse_markdown_tables(domain, content) {
            map.insert(doc.code.to_ascii_uppercase(), doc);
        }
    }
    map
}

fn registry() -> &'static HashMap<String, ErrorCodeDoc> {
    REGISTRY.get_or_init(build_registry)
}

pub fn lookup(code: &str) -> Option<ErrorCodeDoc> {
    registry().get(&code.to_ascii_uppercase()).cloned()
}

pub fn list_all() -> Vec<ErrorCodeDoc> {
    let mut docs: Vec<ErrorCodeDoc> = registry().values().cloned().collect();
    docs.sort_by(|a, b| a.code.cmp(&b.code));
    docs
}

pub fn list_by_domain(domain: &str) -> Vec<ErrorCodeDoc> {
    let domain = domain.to_ascii_lowercase();
    let mut docs: Vec<ErrorCodeDoc> = registry()
        .values()
        .filter(|d| d.domain == domain)
        .cloned()
        .collect();
    docs.sort_by(|a, b| a.code.cmp(&b.code));
    docs
}

fn parse_markdown_tables(domain: &str, content: &str) -> Vec<ErrorCodeDoc> {
    let mut docs = Vec::new();
    let mut in_table = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            in_table = false;
            continue;
        }
        if trimmed.contains("---") {
            in_table = true;
            continue;
        }
        if !in_table {
            continue;
        }

        let cells: Vec<&str> = trimmed
            .trim_matches('|')
            .split('|')
            .map(|c| c.trim())
            .collect();
        if cells.len() < 4 {
            continue;
        }

        let code = extract_backtick_code(cells[0]);
        let Some(code) = code else {
            continue;
        };
        if code.eq_ignore_ascii_case("code") {
            continue;
        }

        let exit_code = parse_exit_code(cells.get(1).copied().unwrap_or("—"));
        let severity = parse_optional_cell(cells.get(2).copied().unwrap_or(""));
        let description = cells.get(3).copied().unwrap_or("").to_string();
        let remediation = parse_remediation(cells.get(4).copied().unwrap_or(""));

        docs.push(ErrorCodeDoc {
            code: code.to_ascii_uppercase(),
            domain: domain.to_string(),
            severity,
            exit_code,
            description,
            remediation,
        });
    }

    docs
}

fn extract_backtick_code(cell: &str) -> Option<String> {
    let start = cell.find('`')?;
    let rest = &cell[start + 1..];
    let end = rest.find('`')?;
    let code = rest[..end].trim();
    if code.is_empty() {
        None
    } else {
        Some(code.to_string())
    }
}

fn parse_exit_code(cell: &str) -> Option<u8> {
    let cell = cell.trim();
    if cell.is_empty() || cell == "—" || cell == "-" {
        return None;
    }
    cell.parse().ok()
}

fn parse_optional_cell(cell: &str) -> Option<String> {
    let cell = cell.trim();
    if cell.is_empty() || cell == "—" || cell == "-" {
        None
    } else {
        Some(cell.to_string())
    }
}

fn parse_remediation(cell: &str) -> Vec<String> {
    let cell = cell.trim();
    if cell.is_empty() || cell == "—" || cell == "-" {
        return Vec::new();
    }
    cell.split(';')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_embedded_codes_parse() {
        let docs = list_all();
        assert!(docs.len() >= 20, "expected many codes, got {}", docs.len());
    }

    #[test]
    fn lookup_case_insensitive() {
        let doc = lookup("broken_agent_ref").expect("BROKEN_AGENT_REF");
        assert_eq!(doc.code, "BROKEN_AGENT_REF");
        assert!(!doc.remediation.is_empty());
    }

    #[test]
    fn list_by_domain_validation() {
        let docs = list_by_domain("validation");
        assert!(docs.iter().any(|d| d.code == "BROKEN_TOOL_REF"));
    }
}
