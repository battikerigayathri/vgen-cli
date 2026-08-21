use vgen::errors_registry::{list_all, lookup};

#[test]
fn all_readme_codes_resolvable() {
    let codes = list_all();
    assert!(codes.len() >= 25);
    for doc in &codes {
        let found = lookup(&doc.code).expect("code should resolve");
        assert_eq!(found.code, doc.code);
        assert!(!found.description.is_empty());
    }
}

#[test]
fn broken_agent_ref_has_remediation() {
    let doc = lookup("BROKEN_AGENT_REF").expect("BROKEN_AGENT_REF");
    assert!(!doc.remediation.is_empty());
}

#[test]
fn workflow_schema_invalid_in_validation_domain() {
    let doc = lookup("WORKFLOW_SCHEMA_INVALID").expect("WORKFLOW_SCHEMA_INVALID");
    assert_eq!(doc.domain, "workflow");
}
