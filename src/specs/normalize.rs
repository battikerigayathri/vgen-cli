use serde_json::{Map, Value};

/// Ensure an id-like value is stored as a string (YAML may parse unquoted ids as numbers).
fn ensure_id_string(v: Value) -> Value {
    match &v {
        Value::String(s) if !s.is_empty() => v,
        Value::Number(n) => Value::String(n.to_string()),
        _ => v,
    }
}

/// Recursively normalize MongoDB extended JSON: convert any `{"$oid": "<string>"}` to `"<string>"`,
/// and `_id` to `id` with the unwrapped string. Ensures `id` is always a string (so create vs update
/// works when YAML parses unquoted id as number). Keeps push/update and pull consistent.
pub fn normalize_mongo_oids(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            if map.len() == 1 {
                if let Some(oid) = map.get("$oid").and_then(|v| v.as_str()) {
                    return Value::String(oid.to_string());
                }
            }
            let mut out = Map::new();
            for (k, v) in map {
                let v = normalize_mongo_oids(v);
                let v = if k == "id" { ensure_id_string(v) } else { v };
                if k == "_id" {
                    out.insert("id".to_string(), ensure_id_string(v));
                } else {
                    out.insert(k, v);
                }
            }
            Value::Object(out)
        }
        Value::Array(arr) => Value::Array(arr.into_iter().map(normalize_mongo_oids).collect()),
        other => other,
    }
}
