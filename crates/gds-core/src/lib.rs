//! Google Drive Sync — core library.
//!
//! Pure sync logic, domain model, and abstractions. No OS-specific I/O;
//! everything injectable via traits for testing.

pub mod api;
pub mod auth;
pub mod db;
pub mod model;
