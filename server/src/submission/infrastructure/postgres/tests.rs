//! The JSONB shape a submission is stored in. The column outlives any one deployment, so its
//! encoding is pinned here rather than left to whatever serde happens to do today.

#![allow(clippy::unwrap_used)]

use super::*;

#[test]
fn the_jsonb_shape_is_circe_parity() {
    let outcome = SuiteOutcome::Rejected {
        passed: 8,
        total: 118,
        first_failure: FailedCase {
            index: 8,
            args: BTreeMap::from([("n".to_owned(), "5".to_owned())]),
            expected: Some("120".to_owned()),
            stdout: "119\n".to_owned(),
            stderr: String::new(),
            status: RunStatus::Accepted,
        },
    };
    let json = serde_json::to_value(OutcomeJson::from(&outcome)).unwrap();
    // The externally-tagged wrapper object + camelCase + case-name status — byte-compatible
    // with rows written by an earlier deployment of this service.
    assert_eq!(json["Rejected"]["passed"], 8);
    assert_eq!(json["Rejected"]["firstFailure"]["status"], "Accepted");
    assert_eq!(json["Rejected"]["firstFailure"]["expected"], "120");
    let back: SuiteOutcome = serde_json::from_value::<OutcomeJson>(json).unwrap().into();
    assert_eq!(back, outcome);
}
