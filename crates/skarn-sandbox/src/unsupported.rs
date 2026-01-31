//! Fallback for platforms without a supported sandbox backend.

use skarn_common::Result;

use crate::{Backend, Policy, RestrictionReport, RestrictionStatus};

pub fn apply(_policy: &Policy) -> Result<RestrictionReport> {
    Ok(
