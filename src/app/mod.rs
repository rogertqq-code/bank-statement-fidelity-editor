//! UI application state and workflow management

pub mod api_verification;
pub mod audit;
pub mod cli;
pub mod config;
pub mod config_v2;
pub mod env_spec;
pub mod error;
pub mod fontcache;
pub mod gui;
pub mod gui_state;
pub mod modals;
pub mod notify;
pub mod paths;
pub mod preflight;
pub mod runtime;
pub mod server;
pub mod telemetry;
pub mod theme;
pub mod watchdog;
pub mod workflow_state;

#[cfg(test)]
pub mod e2e_tests;
