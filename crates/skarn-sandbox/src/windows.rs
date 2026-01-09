//! Windows backend — AppContainer isolation plus a Job Object for resource
//! limits and tree-kill.
//!
//! Unlike Unix, a Windows process cannot move *itself* into an AppContainer, so
//! the parent must launch the worker into one. We:
//!
//! 1. Create (or reuse) an AppContainer profile and derive its SID.
//! 2. Grant that SID read/execute on the program and read/write on the
//!    workspace via ACLs (AppContainer processes are denied by default).
