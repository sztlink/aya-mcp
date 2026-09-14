use std::{collections::HashSet, fmt::Write};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const SCORE_SCHEMA: &str = include_str!("../../../spec/v0/schemas/score.schema.json");
const LEASE_SCHEMA: &str = include_str!("../../../spec/v0/schemas/lease.schema.json");
const CAPABILITY_REPORT_SCHEMA: &str =
    include_str!("../../../spec/v0/schemas/capability-report.schema.json");
const RECEIPT_SCHEMA: &str = include_str!("../../../spec/v0/schemas/receipt.schema.json");

pub const ZERO_SHA256: &str = "0000000000000000000000000000000000000000000000000000000000000000";

pub const SUPPORTED_SCHEMAS: [&str; 4] = [
    "aya.score/v0",
    "aya.lease/v0",
    "aya.capability-report/v0",
    "aya.receipt/v0",
];

fn schema_source(name: &str) -> Option<&'static str> {
    match name {
        "aya.score/v0" => Some(SCORE_SCHEMA),
        "aya.lease/v0" => Some(LEASE_SCHEMA),
        "aya.capability-report/v0" => Some(CAPABILITY_REPORT_SCHEMA),
        "aya.receipt/v0" => Some(RECEIPT_SCHEMA),
        _ => None,
    }
}

pub fn validate_document(document: &Value) -> Result<(), Vec<String>> {
    let name = document
        .get("schema")
        .and_then(Value::as_str)
        .ok_or_else(|| vec!["document schema is required".to_owned()])?;
    let source = schema_source(name).ok_or_else(|| vec![format!("unsupported schema: {name}")])?;
    let schema: Value = serde_json::from_str(source)
        .map_err(|error| vec![format!("embedded schema {name} is invalid: {error}")])?;
    let validator = jsonschema::options()
        .should_validate_formats(true)
        .build(&schema)
        .map_err(|error| vec![format!("could not compile schema {name}: {error}")])?;
    let errors: Vec<String> = validator
        .iter_errors(document)
        .map(|error| format!("{} {error}", error.instance_path()))
        .collect();

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn validate_score(document: &Value) -> Result<(), Vec<String>> {
    if document.get("schema").and_then(Value::as_str) != Some("aya.score/v0") {
        return Err(vec!["aya.score/v0 document required".to_owned()]);
    }
    validate_document(document)?;

    let mut paths = HashSet::new();
    let duplicate = document["inputs"]
        .as_array()
        .expect("Score schema requires inputs array")
        .iter()
        .filter_map(|input| input["path"].as_str())
        .find(|path| !paths.insert(*path));

    if let Some(path) = duplicate {
        Err(vec![format!("score.inputs contains duplicate path {path}")])
    } else {
        Ok(())
    }
}

fn encode_digest(digest: impl IntoIterator<Item = u8>) -> String {
    let mut encoded = String::with_capacity(64);
    for byte in digest {
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    encoded
}

pub fn sha256_bytes(bytes: impl AsRef<[u8]>) -> String {
    encode_digest(Sha256::digest(bytes.as_ref()))
}

pub fn sha256_json(document: &Value) -> Result<String, String> {
    let canonical = serde_jcs::to_vec(document)
        .map_err(|error| format!("RFC 8785 canonicalization failed: {error}"))?;
    Ok(sha256_bytes(canonical))
}

pub fn append_receipt_event(
    events: &mut Vec<Value>,
    at: &str,
    kind: &str,
    summary: &str,
) -> Result<(), String> {
    let previous_sha256 = events
        .last()
        .and_then(|event| event.get("eventSha256"))
        .and_then(Value::as_str)
        .unwrap_or(ZERO_SHA256);
    let mut event = json!({
        "index": events.len(),
        "at": at,
        "kind": kind,
        "summary": summary,
        "previousSha256": previous_sha256
    });
    let digest = sha256_json(&event)?;
    event["eventSha256"] = Value::String(digest);
    events.push(event);
    Ok(())
}

pub fn verify_receipt_chain(events: &[Value]) -> Vec<String> {
    let mut errors = Vec::new();
    for (index, event) in events.iter().enumerate() {
        if event.get("index").and_then(Value::as_u64) != Some(index as u64) {
            errors.push(format!("events[{index}].index must equal {index}"));
        }
        let expected_previous = if index == 0 {
            ZERO_SHA256
        } else {
            events[index - 1]
                .get("eventSha256")
                .and_then(Value::as_str)
                .unwrap_or("")
        };
        if event.get("previousSha256").and_then(Value::as_str) != Some(expected_previous) {
            errors.push(format!(
                "events[{index}].previousSha256 does not match the previous event"
            ));
        }
        let mut unsigned = event.clone();
        if let Some(object) = unsigned.as_object_mut() {
            object.remove("eventSha256");
        }
        let expected_digest = sha256_json(&unsigned).unwrap_or_default();
        if event.get("eventSha256").and_then(Value::as_str) != Some(expected_digest.as_str()) {
            errors.push(format!("events[{index}].eventSha256 is invalid"));
        }
    }
    errors
}

pub fn hash_receipt(receipt: &Value) -> Result<String, String> {
    let mut unsigned = receipt.clone();
    let object = unsigned
        .as_object_mut()
        .ok_or_else(|| "receipt must be a JSON object".to_owned())?;
    object.remove("receiptSha256");
    sha256_json(&unsigned)
}

pub fn seal_receipt(receipt: &mut Value) -> Result<(), String> {
    let digest = hash_receipt(receipt)?;
    let object = receipt
        .as_object_mut()
        .ok_or_else(|| "receipt must be a JSON object".to_owned())?;
    object.insert("receiptSha256".to_owned(), Value::String(digest));
    Ok(())
}

pub fn verify_receipt_digest(receipt: &Value) -> Result<bool, String> {
    Ok(receipt.get("receiptSha256").and_then(Value::as_str)
        == Some(hash_receipt(receipt)?.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(source: &str) -> Value {
        serde_json::from_str(source).expect("fixture must be JSON")
    }

    #[test]
    fn validates_all_golden_documents() {
        let documents = [
            include_str!("../../../spec/v0/fixtures/valid/score.json"),
            include_str!("../../../spec/v0/fixtures/valid/lease.json"),
            include_str!("../../../spec/v0/fixtures/valid/capability-report.json"),
            include_str!("../../../spec/v0/fixtures/valid/receipt.json"),
        ];

        for source in documents {
            validate_document(&fixture(source)).expect("golden fixture must validate");
        }
    }

    #[test]
    fn rejects_path_traversal_fixture() {
        let document = fixture(include_str!(
            "../../../spec/v0/fixtures/invalid/score-path-traversal.json"
        ));
        assert!(validate_document(&document).is_err());
    }

    #[test]
    fn rejects_invalid_date_time_format() {
        let mut lease = fixture(include_str!("../../../spec/v0/fixtures/valid/lease.json"));
        lease["createdAt"] = Value::String("not-a-date".to_owned());
        assert!(validate_document(&lease).is_err());
    }

    #[test]
    fn rejects_duplicate_score_inputs() {
        let mut score = fixture(include_str!("../../../spec/v0/fixtures/valid/score.json"));
        let duplicate = score["inputs"][0].clone();
        score["inputs"]
            .as_array_mut()
            .expect("Score fixture has inputs")
            .push(duplicate);
        assert!(validate_score(&score).is_err());
    }

    #[test]
    fn verifies_node_receipt_chain_and_digest() {
        let receipt = fixture(include_str!("../../../spec/v0/fixtures/valid/receipt.json"));
        assert!(verify_receipt_chain(receipt["events"].as_array().unwrap()).is_empty());
        assert!(verify_receipt_digest(&receipt).unwrap());
    }

    #[test]
    fn appends_and_seals_receipt() {
        let mut events = Vec::new();
        append_receipt_event(
            &mut events,
            "2026-09-14T12:00:00Z",
            "inspect",
            "Synthetic inspection",
        )
        .unwrap();
        assert!(verify_receipt_chain(&events).is_empty());

        let mut receipt = json!({"events": events, "receiptSha256": ZERO_SHA256});
        seal_receipt(&mut receipt).unwrap();
        assert!(verify_receipt_digest(&receipt).unwrap());
    }

    #[test]
    fn matches_node_reference_digest() {
        let score = fixture(include_str!("../../../spec/v0/fixtures/valid/score.json"));
        assert_eq!(
            sha256_json(&score).expect("score must canonicalize"),
            "deaa447ebdf2f9091636f5c49632ef833a58305cbf61d104500c6054646ab326"
        );
    }
}
