mod fixtures;

use dual_core_pdf_pipeline::app::runtime::{Job, JobResult};
use dual_core_pdf_pipeline::engine::model::ProposedChange;
use dual_core_pdf_pipeline::pdf::engine::PdfEngine;
use dual_core_pdf_pipeline::pdf::native_engine::OxidizePdfEngine;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[test]
fn short_document_batch_commits_every_edit_before_success() {
    let workspace = tempfile::tempdir().unwrap();
    let input = workspace.path().join("input.pdf");
    let output = workspace.path().join("output.pdf");
    fixtures::generate_test_pdf(2, &input);

    let engine = OxidizePdfEngine::new();
    let first_before = engine.get_text_blocks(&input, 0).unwrap();
    let second_before = engine.get_text_blocks(&input, 1).unwrap();
    assert_eq!(first_before.len(), 1);
    assert_eq!(second_before.len(), 1);

    let changes = vec![
        ProposedChange {
            page: 0,
            bbox: Some(first_before[0].bbox),
            old_text: first_before[0].text.clone(),
            new_text: "FIRST EDIT".into(),
            reason: "integrity regression".into(),
            confidence: 1.0,
            affects_subsequent_balances: false,
        },
        ProposedChange {
            page: 1,
            bbox: Some(second_before[0].bbox),
            old_text: second_before[0].text.clone(),
            new_text: "SECOND EDIT".into(),
            reason: "integrity regression".into(),
            confidence: 1.0,
            affects_subsequent_balances: false,
        },
    ];

    std::env::remove_var("TEST_CRASH_PYTHON_ACTOR");
    let config = Arc::new(dual_core_pdf_pipeline::app::config::AppConfig::default());
    let audit_log = dual_core_pdf_pipeline::app::audit::AuditLog::open(workspace.path()).unwrap();
    let (_runtime, job_tx, result_rx) =
        dual_core_pdf_pipeline::app::runtime::Runtime::start(audit_log, config);

    job_tx
        .send(Job::ApplyProposedChanges {
            input: input.clone(),
            output: output.clone(),
            changes,
        })
        .unwrap();

    let deadline = Instant::now() + Duration::from_secs(90);
    let mut terminal = None;
    while Instant::now() < deadline {
        match result_rx.recv_timeout(Duration::from_millis(500)) {
            Ok(JobResult::ProposedChangesApplied {
                changes_applied,
                failures,
            }) => {
                terminal = Some((changes_applied, failures));
                break;
            }
            Ok(JobResult::Error { message, .. }) => panic!("batch failed: {message}"),
            Ok(JobResult::WorkflowFailed(failure)) => panic!("workflow failed: {failure:?}"),
            Ok(_) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(error) => panic!("result channel failed: {error}"),
        }
    }

    let (changes_applied, failures) = terminal.expect("missing terminal batch result");
    assert_eq!(changes_applied, 2);
    assert!(failures.is_empty());
    assert!(
        output.is_file(),
        "success must follow durable output creation"
    );

    let first_after = engine.get_text_blocks(&output, 0).unwrap();
    let second_after = engine.get_text_blocks(&output, 1).unwrap();
    assert_eq!(first_after.len(), 1);
    assert_eq!(second_after.len(), 1);
    assert_eq!(first_after[0].text, "FIRST EDIT");
    assert_eq!(second_after[0].text, "SECOND EDIT");
}
