//! The authoring domain: what a proposed change IS (`edit`), where it lands (`branch`), and what
//! it must satisfy to be proposed at all (`validation`). Pure — no forge, no store, no HTTP.

pub mod branch;
pub mod edit;
pub mod validation;

pub use edit::{EditRequest, EditRequestId, EditRequestState, ProposalLocation, PullRequestRef};
