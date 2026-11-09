//! Shared bounded, no-follow explicit-file inspection primitives.
pub(crate) mod parse;
pub(crate) mod reader;
pub(crate) mod types;
use types::{InspectionIssue, LIMITS};
