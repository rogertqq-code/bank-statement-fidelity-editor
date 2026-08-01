use dual_core_pdf_pipeline::engine::model::{FieldBboxes, Provenance, Transaction};
use dual_core_pdf_pipeline::engine::transfer::plan_transaction_transfer_deterministic;
use rust_decimal_macros::dec;

#[test]
fn provider_free_transfer_plans_exact_mapping() {
    let source = Transaction {
        page: 0,
        line_on_page: 0,
        date: "25/12/2023".into(),
        raw_text: "25/12/2023 COFFEE 10.00 90.00".into(),
        debit: None,
        credit: Some(dec!(10.00)),
        running_balance: Some(dec!(90.00)),
        bbox: Some([10.0, 20.0, 500.0, 35.0]),
        field_bboxes: FieldBboxes::default(),
        provenance: Provenance::Computed,
        category: None,
        canonical: Default::default(),
    };
    let target = Transaction {
        page: 1,
        line_on_page: 4,
        date: "12/25/2023".into(),
        raw_text: "TARGET ROW".into(),
        debit: None,
        credit: Some(dec!(1.00)),
        running_balance: Some(dec!(99.00)),
        bbox: Some([10.0, 40.0, 500.0, 55.0]),
        field_bboxes: FieldBboxes {
            date: Some([10.0, 40.0, 70.0, 55.0]),
            description: Some([80.0, 40.0, 250.0, 55.0]),
            debit: Some([260.0, 40.0, 320.0, 55.0]),
            credit: Some([330.0, 40.0, 390.0, 55.0]),
            running_balance: Some([400.0, 40.0, 490.0, 55.0]),
        },
        provenance: Provenance::Computed,
        category: None,
        canonical: Default::default(),
    };

    let plan = plan_transaction_transfer_deterministic(&[source], &[target], 2)
        .expect("provider-free exact-capacity mapping should succeed");
    assert_eq!(plan.strategy, "deterministic-local-exact-capacity");
    assert_eq!(plan.confidence, 1.0);
    assert_eq!(plan.mappings.len(), 1);
    assert_eq!(plan.mappings[0].target_page, 1);
    assert_eq!(plan.mappings[0].target_line, 4);
    assert_eq!(plan.mappings[0].converted_date, "12/25/2023");
    assert_eq!(plan.mappings[0].adapted_description, "COFFEE");
}
