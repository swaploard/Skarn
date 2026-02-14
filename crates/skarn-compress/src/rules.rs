//! The declarative rule model and the embedded built-in defaults.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The full configuration: a `default` rule set plus per-tool patches that
/// extend / override it.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
