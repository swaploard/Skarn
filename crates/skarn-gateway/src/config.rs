//! Gateway configuration, parsed from `skarn.toml`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use skarn_common::{Error, Result};

