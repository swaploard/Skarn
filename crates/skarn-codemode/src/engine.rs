//! The hermetic Code Mode runtime, built on an async QuickJS isolate.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use rquickjs::prelude::{Async, Func};
use rquickjs::{AsyncContext, AsyncRuntime, CatchResultExt, async_with};
use serde::{Deserialize, Serialize};
use skarn_common::{Error, Result};

use crate::bridge::ToolBridge;

/// Resource limits for a single Code Mode execution.
