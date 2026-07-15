//! The contract lock (RS001): the utoipa-rendered OpenAPI document must cover the oracle spec —
//! every oracle path/method exists here, and the shared schemas agree on property names and
//! required fields. `api/openapi.oracle.yaml` is a committed copy of Synapse's
//! `api/openapi.yaml`; it grows in lock-step as endpoints are ported, so drift from the Scala
//! contract is a red test, not a production surprise.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeSet;

use serde_json::Value;
use utoipa::OpenApi;

fn keys(v: &Value) -> BTreeSet<String> {
    v.as_object()
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default()
}

fn required(v: &Value) -> BTreeSet<String> {
    v.get("required")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(str::to_owned).collect())
        .unwrap_or_default()
}

#[test]
fn every_oracle_path_and_method_is_served() {
    let oracle: Value = serde_yaml::from_str(include_str!("../../api/openapi.oracle.yaml")).unwrap();
    let ours = serde_json::to_value(synapse_server::ApiDoc::openapi()).unwrap();

    for (path, item) in oracle["paths"].as_object().unwrap() {
        let our_item = ours["paths"]
            .get(path)
            .unwrap_or_else(|| panic!("oracle path missing from our spec: {path}"));
        for method in keys(item) {
            assert!(
                our_item.get(&method).is_some(),
                "oracle operation missing from our spec: {method} {path}"
            );
        }
    }
}

#[test]
fn shared_schemas_match_the_oracle_shape() {
    let oracle: Value = serde_yaml::from_str(include_str!("../../api/openapi.oracle.yaml")).unwrap();
    let ours = serde_json::to_value(synapse_server::ApiDoc::openapi()).unwrap();

    for (name, oracle_schema) in oracle["components"]["schemas"].as_object().unwrap() {
        let our_schema = &ours["components"]["schemas"][name];
        assert!(
            !our_schema.is_null(),
            "oracle schema missing from our components: {name}"
        );
        assert_eq!(
            keys(&our_schema["properties"]),
            keys(&oracle_schema["properties"]),
            "property names diverge on {name}"
        );
        assert_eq!(
            required(our_schema),
            required(oracle_schema),
            "required fields diverge on {name}"
        );
    }
}
