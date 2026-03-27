//! End-to-end Code Mode execution tests (real QuickJS isolate + in-process bridge).
//!
//! rquickjs' async runtime is `!Send`, so these run on a current-thread runtime.

use std::sync::Arc;
use std::time::Duration;

use skarn_codemode::{Engine, ExecLimits, InProcessBridge, ToolBridge};

fn math_bridge() -> Arc<dyn ToolBridge> {
