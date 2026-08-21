use vgen::sync::extract_hitl_slugs;
use serde_json::json;

#[test]
fn workflow_with_hitl_fixture_extracts_slugs() {
    let wf = json!({
        "authorBundle": {
            "meta": { "slug": "oracle-purchase-requisition-v1" },
            "flow": {
                "initialStage": "collect_requester",
                "stages": [
                    {
                        "id": "collect_requester",
                        "hitlSlug": "roc-select-requester-form",
                        "agentSlug": "oracle-pr-agent"
                    }
                ]
            }
        }
    });
    let slugs = extract_hitl_slugs(&wf);
    assert_eq!(slugs, vec!["roc-select-requester-form"]);
}
