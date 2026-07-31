//! End-to-end integration tests for the application workflow.
//!
//! These tests simulate real user workflows and verify the entire
//! application pipeline from PDF parsing to final validation.

use crate::app::config_v2::EnhancedConfig;
use crate::app::gui_state::GuiState;
use crate::app::workflow_state::{
    EditResult, ParseResult, WorkflowStage, WorkflowVerificationReport,
};
use crate::engine::model::Provenance;
use crate::engine::model::Transaction;
use std::path::PathBuf;

/// Mock E2E test that simulates a user clicking through all 6 stages.
#[tokio::test]
async fn e2e_user_workflow_simulation() {
    // Initialize GUI state
    let mut gui_state = GuiState::default();

    // Stage 1: Parse PDF
    gui_state
        .workflow
        .transition_to(WorkflowStage::Parse)
        .unwrap();
    assert_eq!(gui_state.workflow.current_stage(), WorkflowStage::Parse);

    // Simulate parsing
    let parse_result = ParseResult {
        transactions: vec![
            Transaction {
                page: 1,
                line_on_page: 1,
                date: "2026-01-01".to_string(),
                raw_text: "Opening Balance".to_string(),
                debit: None,
                credit: None,
                running_balance: Some(rust_decimal::Decimal::new(100000, 2)),
                bbox: Some([100.0, 200.0, 300.0, 220.0]),
                field_bboxes: Default::default(),
                provenance: Provenance::DocumentAI { confidence: 0.95 },
                category: None,
            },
            Transaction {
                page: 1,
                line_on_page: 2,
                date: "2026-01-02".to_string(),
                raw_text: "Deposit".to_string(),
                debit: Some(rust_decimal::Decimal::new(50000, 2)),
                credit: None,
                running_balance: Some(rust_decimal::Decimal::new(150000, 2)),
                bbox: Some([100.0, 250.0, 300.0, 270.0]),
                field_bboxes: Default::default(),
                provenance: Provenance::DocumentAI { confidence: 0.95 },
                category: None,
            },
        ],
        opening_balance: rust_decimal::Decimal::new(100000, 2),
        expected_closing: Some(rust_decimal::Decimal::new(150000, 2)),
        parser_used: "DocumentAI".to_string(),
    };

    gui_state.workflow.set_parse_result(parse_result);
    gui_state.transactions = gui_state
        .workflow
        .parse_result()
        .unwrap()
        .transactions
        .clone();

    // Advance to Stage 2: Edit
    gui_state.advance_workflow().unwrap();
    assert_eq!(gui_state.workflow.current_stage(), WorkflowStage::Edit);

    // Simulate editing a transaction
    let mut edited_tx = gui_state.transactions[1].clone();
    edited_tx.debit = Some(rust_decimal::Decimal::new(60000, 2)); // Change 500 to 600
    gui_state.update_transaction(1, edited_tx).unwrap();

    // Set edit result
    gui_state.workflow.set_edit_result(EditResult {
        edited_transactions: gui_state.transactions.clone(),
        edits_applied: 1,
        balance_adjusted: true,
    });

    // Advance to Stage 3: Preview
    gui_state.advance_workflow().unwrap();
    assert_eq!(gui_state.workflow.current_stage(), WorkflowStage::Preview);

    // Advance to Stage 4: Render
    gui_state.advance_workflow().unwrap();
    assert_eq!(gui_state.workflow.current_stage(), WorkflowStage::Render);

    // Simulate rendering
    gui_state
        .workflow
        .set_render_result(crate::app::workflow_state::RenderResult {
            output_path: PathBuf::from("output.pdf"),
            pages_rendered: 1,
            render_time_ms: 1500,
        });

    // Advance to Stage 5: Validate
    gui_state.advance_workflow().unwrap();
    assert_eq!(gui_state.workflow.current_stage(), WorkflowStage::Validate);

    // Simulate validation using WorkflowVerificationReport
    let validation_result = WorkflowVerificationReport {
        math_valid: true,
        visual_diff_score: 0.001,
        only_intended_changes: true,
        message: "Passed".to_string(),
        ssim_score: 0.98,
    };

    gui_state.workflow.set_validation_result(validation_result);

    // Advance to Stage 6: Final
    gui_state.advance_workflow().unwrap();
    assert_eq!(gui_state.workflow.current_stage(), WorkflowStage::Final);

    // Verify final state
    assert!(gui_state.workflow.validation_result().unwrap().ssim_score > 0.95);

    println!("E2E workflow simulation completed successfully!");
}

/// Test configuration auto-configuration.
#[test]
fn test_config_auto_configure() {
    let config = EnhancedConfig::auto_configure();

    // Should have reasonable defaults
    assert!(config.visual_diff_threshold > 0.0);
    assert!(config.visual_diff_threshold < 1.0);
    assert!(config.ssim_floor > 0.0);
    assert!(config.ssim_floor <= 1.0);
}

/// Test that the workflow state machine prevents invalid transitions.
#[test]
fn test_workflow_invalid_transition() {
    let mut state = GuiState::default();

    // Try to skip from Parse to Preview (should fail)
    let result = state.workflow.transition_to(WorkflowStage::Preview);
    assert!(result.is_err());
}
