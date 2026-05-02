#![forbid(unsafe_code)]

pub mod alerts;
pub mod args;
pub mod commands;
pub mod config;
pub mod events;
pub mod logging;
pub mod metrics;
pub mod pattern;
pub mod pipeline;
pub mod rules;
pub mod scanner;
pub mod server;
#[cfg(feature = "otel")]
pub mod telemetry;
pub mod validation;
pub mod watcher;
