//! Strict 6-stage linear workflow state machine with atomic transitions.
//!
//! This module prevents skipping stages or invalid transitions to ensure
//! the application follows the correct processing pipeline.

use crate::engine::model::Transaction;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Strict 6-stage linear workflow with atomic state transitions.
/// Prevents skipping stages or invalid transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorkflowStage {
    Parse,    // Stage 1: Parse PDF with AI
    Edit,     // Stage 2: User edits transactions
    Preview,  // Stage 3: Preview changes
    Render,   // Stage 4: Render edited PDF
    Validate, // Stage 5: Visual + math validation
    Final,    // Stage 6: Final math check and save
}

impl WorkflowStage {
    /// Get the next stage in the linear flow.
    pub fn next(&self) -> Option<Self> {
        match self {
            Self::Parse => Some(Self::Edit),
            Self::Edit => Some(Self::Preview),
            Self::Preview => Some(Self::Render),
            Self::Render => Some(Self::Validate),
            Self::Validate => Some(Self::Final),
            Self::Final => None,
        }
    }

    /// Get the previous stage (for back navigation).
    pub fn prev(&self) -> Option<Self> {
        match self {
            Self::Parse => None,
            Self::Edit => Some(Self::Parse),
            Self::Preview => Some(Self::Edit),
            Self::Render => Some(Self::Preview),
            Self::Validate => Some(Self::Render),
            Self::Final => Some(Self::Validate),
        }
    }

    /// Check if transition from self to target is valid.
    pub fn can_transition_to(&self, target: Self) -> bool {
        // Can always stay in same stage
        if *self == target {
            return true;
        }

        // Can only move to adjacent stages
        self.next() == Some(target) || self.prev() == Some(target)
    }

    /// Get the progress fraction (0.0 to 1.0) for this stage.
    pub fn progress_fraction(&self) -> f32 {
        match self {
            Self::Parse => 0.0,
            Self::Edit => 0.2,
            Self::Preview => 0.4,
            Self::Render => 0.6,
            Self::Validate => 0.8,
            Self::Final => 1.0,
        }
    }

    /// Get the human-readable label for this stage.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Parse => "Parse PDF",
            Self::Edit => "Edit Transactions",
            Self::Preview => "Preview Changes",
            Self::Render => "Render PDF",
            Self::Validate => "Validate Fidelity",
            Self::Final => "Finalize",
        }
    }
}

/// Workflow state machine with atomic transitions.
#[derive(Debug, Clone)]
pub struct WorkflowStateMachine {
    current_stage: WorkflowStage,
    parse_result: Option<ParseResult>,
    edit_result: Option<EditResult>,
    render_result: Option<RenderResult>,
    validation_result: Option<WorkflowVerificationReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseResult {
    pub transactions: Vec<Transaction>,
    pub opening_balance: rust_decimal::Decimal,
    pub expected_closing: Option<rust_decimal::Decimal>,
    pub parser_used: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditResult {
    pub edited_transactions: Vec<Transaction>,
    pub edits_applied: usize,
    pub balance_adjusted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderResult {
    pub output_path: PathBuf,
    pub pages_rendered: usize,
    pub render_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowVerificationReport {
    pub math_valid: bool,
    pub visual_diff_score: f64,
    pub only_intended_changes: bool,
    pub message: String,
    pub ssim_score: f64,
}

impl WorkflowStateMachine {
    pub fn new() -> Self {
        Self {
            current_stage: WorkflowStage::Parse,
            parse_result: None,
            edit_result: None,
            render_result: None,
            validation_result: None,
        }
    }

    /// Attempt to transition to the next stage.
    pub fn transition_to(&mut self, target: WorkflowStage) -> Result<(), WorkflowError> {
        if !self.current_stage.can_transition_to(target) {
            return Err(WorkflowError::InvalidTransition {
                from: self.current_stage,
                to: target,
            });
        }

        // Validate prerequisites for the target stage
        self.validate_prerequisites(target)?;

        self.current_stage = target;
        Ok(())
    }

    /// Transition to the next stage in the linear flow.
    pub fn advance(&mut self) -> Result<(), WorkflowError> {
        if let Some(next) = self.current_stage.next() {
            self.transition_to(next)
        } else {
            Err(WorkflowError::AlreadyAtFinalStage)
        }
    }

    fn validate_prerequisites(&self, target: WorkflowStage) -> Result<(), WorkflowError> {
        match target {
            WorkflowStage::Edit => {
                if self.parse_result.is_none() {
                    return Err(WorkflowError::MissingPrerequisite {
                        stage: target,
                        required: WorkflowStage::Parse,
                    });
                }
            }
            WorkflowStage::Preview | WorkflowStage::Render => {
                if self.edit_result.is_none() {
                    return Err(WorkflowError::MissingPrerequisite {
                        stage: target,
                        required: WorkflowStage::Edit,
                    });
                }
            }
            WorkflowStage::Validate => {
                if self.render_result.is_none() {
                    return Err(WorkflowError::MissingPrerequisite {
                        stage: target,
                        required: WorkflowStage::Render,
                    });
                }
            }
            WorkflowStage::Final => {
                if self.validation_result.is_none() {
                    return Err(WorkflowError::MissingPrerequisite {
                        stage: target,
                        required: WorkflowStage::Validate,
                    });
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Store the result of the Parse stage.
    pub fn set_parse_result(&mut self, result: ParseResult) {
        self.parse_result = Some(result);
    }

    /// Store the result of the Edit stage.
    pub fn set_edit_result(&mut self, result: EditResult) {
        self.edit_result = Some(result);
    }

    /// Store the result of the Render stage.
    pub fn set_render_result(&mut self, result: RenderResult) {
        self.render_result = Some(result);
    }

    /// Store the result of the Validate stage.
    pub fn set_validation_result(&mut self, result: WorkflowVerificationReport) {
        self.validation_result = Some(result);
    }

    pub fn current_stage(&self) -> WorkflowStage {
        self.current_stage
    }

    pub fn parse_result(&self) -> Option<&ParseResult> {
        self.parse_result.as_ref()
    }

    pub fn edit_result(&self) -> Option<&EditResult> {
        self.edit_result.as_ref()
    }

    pub fn render_result(&self) -> Option<&RenderResult> {
        self.render_result.as_ref()
    }

    pub fn validation_result(&self) -> Option<&WorkflowVerificationReport> {
        self.validation_result.as_ref()
    }
}

impl Default for WorkflowStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkflowError {
    InvalidTransition {
        from: WorkflowStage,
        to: WorkflowStage,
    },
    MissingPrerequisite {
        stage: WorkflowStage,
        required: WorkflowStage,
    },
    AlreadyAtFinalStage,
}

impl std::fmt::Display for WorkflowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidTransition { from, to } => {
                write!(f, "Invalid transition from {:?} to {:?}", from, to)
            }
            Self::MissingPrerequisite { stage, required } => {
                write!(
                    f,
                    "Stage {:?} requires {:?} to be completed first",
                    stage, required
                )
            }
            Self::AlreadyAtFinalStage => {
                write!(f, "Already at Final stage, cannot advance further")
            }
        }
    }
}

impl std::error::Error for WorkflowError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_transitions() {
        let mut machine = WorkflowStateMachine::new();

        assert_eq!(machine.current_stage(), WorkflowStage::Parse);

        // Set parse result before advancing
        machine.set_parse_result(ParseResult {
            transactions: vec![],
            opening_balance: rust_decimal::Decimal::ZERO,
            expected_closing: None,
            parser_used: "test".to_string(),
        });

        machine.advance().unwrap();
        assert_eq!(machine.current_stage(), WorkflowStage::Edit);

        // Set edit result before advancing
        machine.set_edit_result(EditResult {
            edited_transactions: vec![],
            edits_applied: 0,
            balance_adjusted: false,
        });

        machine.advance().unwrap();
        assert_eq!(machine.current_stage(), WorkflowStage::Preview);
    }

    #[test]
    fn test_invalid_transition() {
        let mut machine = WorkflowStateMachine::new();

        // Try to skip from Parse to Preview
        let result = machine.transition_to(WorkflowStage::Preview);
        assert!(result.is_err(), "Should not allow skipping stages");
    }

    #[test]
    fn test_missing_prerequisite() {
        let mut machine = WorkflowStateMachine::new();

        // Try to advance without parse result
        let result = machine.transition_to(WorkflowStage::Edit);
        assert!(result.is_err(), "Should require parse result");
    }
}
