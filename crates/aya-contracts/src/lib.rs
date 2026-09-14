use std::{collections::HashSet, fmt::Write};

use serde_json::Value;
use sha2::{Digest, Sha256};

const SCORE_SCHEMA: &str = include_str!("../../../spec/v0/schemas/score.schema.json");
const LEASE_SCHEMA: &str = include_str!("../../../spec/v0/schemas/lease.schema.json");
const CAPABILITY_REPORT_SCHEMA: &str =
    include_str!("../../../spec/v0/schemas/capability-report.schema.json");
const RECEIPT_SCHEMA: &str = include_str!("../../../spec/v0/schemas/receipt.schema.json");

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

pub fn sha256_json(document: &Value) -> Result<String, String> {
    let canonical = serde_jcs::to_vec(document)
        .map_err(|error| format!("RFC 8785 canonicalization failed: {error}"))?;
    let digest = Sha256::digest(canonical);
    let mut encoded = String::with_capacity(64);
    for byte in digest {
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(encoded)
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
    fn matches_node_reference_digest() {
        let score = fixture(include_str!("../../../spec/v0/fixtures/valid/score.json"));
        assert_eq!(
            sha256_json(&score).expect("score must canonicalize"),
            "deaa447ebdf2f9091636f5c49632ef833a58305cbf61d104500c6054646ab326"
        );
    }
}
