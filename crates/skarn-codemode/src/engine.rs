//! The hermetic Code Mode runtime, built on an async QuickJS isolate.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

