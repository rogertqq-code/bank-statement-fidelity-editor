//! GUI state management with integrated autosave functionality.
//!
//! This module manages the application lifecycle and ensures no data loss
//! through automatic state persistence.

use crate::app::workflow_state::WorkflowStateMachine;
use crate::engine::model::Transaction;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

/// Main GUI state that manages the application lifecycle.
/// All state transitions are atomic and integrated with autosave.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuiState {
    #[serde(skip)]
    pub workflow: WorkflowStateMachine,
    pub current_pdf_path: Option<PathBuf>,
    pub transactions: Vec<Transaction>,
    pub selected_transaction: Option<usize>,
    pub edit_mode: EditMode,
    pub preview_zoom: f32,
    pub show_tooltips: bool,
    pub autosave_interval: Duration,
    // Store as SystemTime for serialization compatibility
    #[serde(skip, default = "SystemTime::now")]
    pub last_autosave: SystemTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditMode {
    View,
    EditTransaction,
    EditText,
    EditBalance,
}

impl Default for GuiState {
    fn default() -> Self {
        Self {
            workflow: WorkflowStateMachine::new(),
            current_pdf_path: None,
            transactions: Vec::new(),
            selected_transaction: None,
            edit_mode: EditMode::View,
            preview_zoom: 1.0,
            show_tooltips: true,
            autosave_interval: Duration::from_secs(30),
            last_autosave: SystemTime::now(),
        }
    }
}

impl GuiState {
    /// Transition to the next workflow stage with autosave.
    pub fn advance_workflow(&mut self) -> Result<(), String> {
        self.workflow
            .advance()
            .map_err(|e| format!("Failed to advance workflow: {}", e))?;

        // Autosave after stage transition
        self.autosave();

        Ok(())
    }

    /// Update a transaction and track the change in history.
    pub fn update_transaction(&mut self, index: usize, new_tx: Transaction) -> Result<(), String> {
        if index >= self.transactions.len() {
            return Err(format!("Transaction index {} out of bounds", index));
        }

        self.transactions[index] = new_tx.clone();

        // Autosave after edit
        self.autosave();

        Ok(())
    }

    /// Autosave the current state to disk.
    fn autosave(&mut self) {
        // Check if enough time has elapsed since last autosave
        if let Ok(elapsed) = self.last_autosave.elapsed() {
            if elapsed >= self.autosave_interval {
                let state_json = serde_json::to_string_pretty(self).unwrap_or_else(|e| {
                    tracing::error!("Failed to serialize GUI state: {}", e);
                    String::new()
                });

                if !state_json.is_empty() {
                    let autosave_path = PathBuf::from("audit/autosave.json");
                    let tmp_path = autosave_path.with_extension("tmp");

                    if let Ok(mut file) = std::fs::File::create(&tmp_path) {
                        use std::io::Write;
                        if file.write_all(state_json.as_bytes()).is_ok() {
                            let _ = file.sync_all();
                            let _ = std::fs::rename(&tmp_path, &autosave_path);
                            tracing::debug!("Autosaved GUI state");
                        }
                    }
                }

                self.last_autosave = SystemTime::now();
            }
        }
    }

    /// Load autosaved state from disk.
    pub fn load_autosave() -> Option<Self> {
        let autosave_path = PathBuf::from("audit/autosave.json");
        if autosave_path.exists() {
            match std::fs::read_to_string(&autosave_path) {
                Ok(json) => {
                    match serde_json::from_str::<Self>(&json) {
                        Ok(mut state) => {
                            // Reset the autosave timer
                            state.last_autosave = SystemTime::now();
                            tracing::info!("Loaded autosaved state");
                            Some(state)
                        }
                        Err(e) => {
                            tracing::error!("Failed to deserialize autosaved state: {}", e);
                            None
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to read autosave file: {}", e);
                    None
                }
            }
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // use crate::engine::model::Provenance; // TODO: use when implementing provenance tracking

    #[test]
    fn test_gui_state_creation() {
        let state = GuiState::default();
        assert_eq!(state.preview_zoom, 1.0);
        assert!(state.transactions.is_empty());
    }

    #[test]
    fn test_workflow_advance() {
        let mut state = GuiState::default();

        // Set parse result to allow advancement
        state
            .workflow
            .set_parse_result(crate::app::workflow_state::ParseResult {
                transactions: vec![],
                opening_balance: rust_decimal::Decimal::ZERO,
                expected_closing: None,
                parser_used: "test".to_string(),
            });

        let result = state.advance_workflow();
        assert!(result.is_ok(), "Should advance to Edit stage");
    }
}
