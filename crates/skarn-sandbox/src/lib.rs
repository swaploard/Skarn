//! OS-native process sandboxing with a single, type-safe API.
//!
//! `skarn-sandbox` abstracts three very different kernel mechanisms behind one
//! [`Policy`]:
//!
//! | Platform | Mechanism | Backend |
//! |----------|-----------|---------|
//! | macOS    | Seatbelt (`sandbox_init`) | [`Backend::Seatbelt`] |
//! | Linux    | Landlock LSM + seccomp-bpf | [`Backend::Landlock`] |
//! | Windows  | AppContainer + Job Object  | [`Backend::AppContainer`] |
//!
//! # Execution model
//!
//! The most robust way to confine *arbitrary* programs (including a program we
//! do not control, like `cat`) is to run them through a **worker that is born
//! sandboxed**. On Unix the worker calls [`apply_to_current_process`] as its
