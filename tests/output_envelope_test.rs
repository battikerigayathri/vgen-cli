use vgen::output::{Envelope, ErrorBody, WarningBody};

#[test]
fn envelope_success_round_trip() {
    #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
    struct SampleData {
        value: u32,
    }

    let envelope = Envelope {
        ok: true,
        command: "test".to_string(),
        data: Some(SampleData { value: 42 }),
        error: None,
        warnings: vec![],
    };

    let json = serde_json::to_string(&envelope).expect("serialize");
    let parsed: Envelope<SampleData> = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(parsed.ok, true);
    assert_eq!(parsed.command, "test");
    assert_eq!(parsed.data, Some(SampleData { value: 42 }));
    assert!(parsed.error.is_none());
    assert!(parsed.warnings.is_empty());
}

#[test]
fn envelope_error_round_trip() {
    let envelope: Envelope<()> = Envelope {
        ok: false,
        command: "config validate".to_string(),
        data: None,
        error: Some(ErrorBody {
            code: "CONNECTIVITY_FAILED".to_string(),
            message: "Request failed".to_string(),
            details: Some(serde_json::json!({ "status": 503 })),
        }),
        warnings: vec![WarningBody {
            code: "JWT_SECRET_MISSING".to_string(),
            message: "VGEN_SECRET not set".to_string(),
        }],
    };

    let json = serde_json::to_string(&envelope).expect("serialize");
    let parsed: Envelope<serde_json::Value> = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(parsed.ok, false);
    assert_eq!(parsed.command, "config validate");
    assert!(parsed.data.is_none());
    let err = parsed.error.expect("error body");
    assert_eq!(err.code, "CONNECTIVITY_FAILED");
    assert_eq!(err.message, "Request failed");
    assert_eq!(err.details.unwrap()["status"], 503);
    assert_eq!(parsed.warnings.len(), 1);
}
