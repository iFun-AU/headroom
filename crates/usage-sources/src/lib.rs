//! Asynchronous data-source infrastructure for How Is It.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod event;

/// Centralized filesystem path derivation.
pub mod paths;
/// Activity-aware polling scheduler.
pub mod scheduler;
/// Single-owner usage and history store actor.
pub mod store;
/// Bounded incremental file tailing.
pub mod tail;
/// Recursive, debounced filesystem event delivery.
pub mod watch;

pub use event::SourceEvent;
