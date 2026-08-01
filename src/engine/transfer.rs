//! Transaction Transfer Pipeline.
//!
//! Transfers transactions from a "source" bank statement PDF to a "target"
//! bank statement PDF, intelligently adapting formats (dates, numbers,
//! descriptions, column layouts) to match the target's visual style. The
//! pipeline runs through 9 stages with live progress reporting and exhaustive
//! AI + engine verification.

use crate::engine::model::FieldBboxes;
use crate::engine::number_format::NumberFormat;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// Describes the visual and structural format of a parsed bank statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatementFormat {
    pub bank_name: String,
    /// e.g. "DD/MM/YYYY", "MM/DD/YYYY", "YYYY-MM-DD"
    pub date_format: String,
    /// Number rendering style (currency, separators, negative convention).
    pub number_format: NumberFormat,
    /// Ordered list of columns in the transaction table.
    pub column_order: Vec<ColumnType>,
    pub has_running_balance: bool,
    pub currency_symbol: String,
    /// Estimated transaction rows that fit on a single page.
    pub rows_per_page: usize,
    /// Page header area height in PDF points (logo, account info).
    pub header_height_pts: f32,
    /// Page footer area height in PDF points.
    pub footer_height_pts: f32,
    /// Bounding box of the transaction table area on a typical page.
    pub transaction_area_bbox: [f32; 4],
    /// Primary font used in the transaction table.
    pub font_name: String,
    /// Font size in points.
    pub font_size: f32,
    /// Vertical spacing between transaction rows in points.
    pub row_height_pts: f32,
    /// Which Document AI processor version works best for this format.
    pub parser_version: Option<String>,
}

/// Column types found in bank statement transaction tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColumnType {
    Date,
    Description,
    Debit,
    Credit,
    Amount,
    Balance,
    Reference,
    ValueDate,
}

/// A fully mapped transaction ready to be written into the target PDF.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MappedTransaction {
    /// Target page index (0-based).
    pub target_page: usize,
    /// Line index within the target page.
    pub target_line: usize,
    /// Date string already converted to the target's format.
    pub date: String,
    /// Description adapted to the target's style.
    pub description: String,
    /// Debit amount (money in).
    pub debit: Option<Decimal>,
    /// Credit amount (money out).
    pub credit: Option<Decimal>,
    /// Running balance recomputed from the target's opening balance.
    pub running_balance: Decimal,
    /// Where each field should be placed on the target page.
    pub field_bboxes: FieldBboxes,
}

/// Gemini's plan for how to execute the transfer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferPlan {
    /// Per-transaction mapping instructions.
    pub mappings: Vec<TransactionMapping>,
    /// How many pages the output will have.
    pub output_page_count: usize,
    /// Pages from the target to clone (for extra capacity).
    pub pages_to_clone: Vec<usize>,
    /// Pages from the target to remove (excess capacity).
    pub pages_to_remove: Vec<usize>,
    /// Overall strategy description.
    pub strategy: String,
    /// Confidence score (0..1).
    pub confidence: f32,
}

/// How a single source transaction maps to the target format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionMapping {
    /// Index into the source transaction list.
    pub source_index: usize,
    /// Target page the transaction lands on.
    pub target_page: usize,
    /// Target line within that page.
    pub target_line: usize,
    /// Date converted to the target's format.
    pub converted_date: String,
    /// Description adapted to the target's convention.
    pub adapted_description: String,
}

/// Result of the entire transfer pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferResult {
    pub output_path: PathBuf,
    pub source_tx_count: usize,
    pub target_tx_count: usize,
    pub pages_added: usize,
    pub pages_removed: usize,
    pub math_verified: bool,
    pub visual_verified: bool,
    pub visual_score: f64,
    pub math_imbalance: Decimal,
    pub stages_completed: u8,
    pub total_duration_secs: f64,
    pub corrections_applied: usize,
    pub retries_attempted: usize,
    pub synthesized_fonts_used: bool,
}

/// Tracks which stage the pipeline is currently executing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferStage {
    AnalyzeSource,
    AnalyzeTarget,
    AiFormatMapping,
    ComputeBalances,
    PdfSurgery,
    VisualFidelityCheck,
    MathVerificationEngine,
    MathVerificationGemini,
    FinalAudit,
}

impl TransferStage {
    pub fn label(&self) -> &'static str {
        match self {
            Self::AnalyzeSource => "Analyzing source statement...",
            Self::AnalyzeTarget => "Analyzing target statement...",
            Self::AiFormatMapping => "AI mapping transaction formats...",
            Self::ComputeBalances => "Computing balances...",
            Self::PdfSurgery => "Applying PDF changes...",
            Self::VisualFidelityCheck => "Verifying visual fidelity...",
            Self::MathVerificationEngine => "Verifying math (engine)...",
            Self::MathVerificationGemini => "Verifying math (AI)...",
            Self::FinalAudit => "Writing audit report...",
        }
    }

    /// Progress fraction range [start, end) for this stage.
    pub fn fraction_range(&self) -> (f32, f32) {
        match self {
            Self::AnalyzeSource => (0.00, 0.10),
            Self::AnalyzeTarget => (0.10, 0.20),
            Self::AiFormatMapping => (0.20, 0.30),
            Self::ComputeBalances => (0.30, 0.35),
            Self::PdfSurgery => (0.35, 0.55),
            Self::VisualFidelityCheck => (0.55, 0.75),
            Self::MathVerificationEngine => (0.75, 0.85),
            Self::MathVerificationGemini => (0.85, 0.95),
            Self::FinalAudit => (0.95, 1.00),
        }
    }
}

/// Build a deterministic transfer plan without an AI provider.
///
/// The local planner is deliberately exact rather than speculative: it requires
/// one editable target row for every source row, preserves source and target
/// document order, converts only unambiguous dates, and refuses to invent page
/// capacity or description conventions. Optional AI providers may handle more
/// complex layouts, but provider absence must not block this supported case.
pub fn plan_transaction_transfer_deterministic(
    source_transactions: &[crate::engine::model::Transaction],
    target_transactions: &[crate::engine::model::Transaction],
    target_page_count: usize,
) -> Result<TransferPlan, String> {
    if source_transactions.is_empty() {
        return Err("deterministic transfer requires at least one source row".into());
    }
    if target_transactions.is_empty() {
        return Err("deterministic transfer requires at least one target row".into());
    }
    if source_transactions.len() != target_transactions.len() {
        return Err(format!(
            "deterministic transfer requires equal row capacity: source has {}, target has {}",
            source_transactions.len(),
            target_transactions.len()
        ));
    }

    let source_format = infer_statement_date_format(source_transactions)?;
    let target_format = infer_statement_date_format(target_transactions)?;

    let mut source_order: Vec<usize> = (0..source_transactions.len()).collect();
    source_order.sort_by_key(|index| {
        let tx = &source_transactions[*index];
        (tx.page, tx.line_on_page)
    });
    let mut target_order: Vec<&crate::engine::model::Transaction> =
        target_transactions.iter().collect();
    target_order.sort_by_key(|tx| (tx.page, tx.line_on_page));

    let mut target_rows = HashSet::new();
    let mut mappings = Vec::with_capacity(source_transactions.len());
    for (source_index, target) in source_order.into_iter().zip(target_order) {
        if !target_rows.insert((target.page, target.line_on_page)) {
            return Err(format!(
                "target row identity is duplicated at page {} line {}",
                target.page, target.line_on_page
            ));
        }
        if target.bbox.is_none() && target.field_bboxes.is_empty() {
            return Err(format!(
                "target row at page {} line {} has no editable geometry",
                target.page, target.line_on_page
            ));
        }
        let source = &source_transactions[source_index];
        if source.debit.is_none() && source.credit.is_none() {
            return Err(format!(
                "source row at page {} line {} has no monetary amount",
                source.page, source.line_on_page
            ));
        }
        mappings.push(TransactionMapping {
            source_index,
            target_page: target.page,
            target_line: target.line_on_page,
            converted_date: convert_date(&source.date, source_format, target_format)?,
            adapted_description: transfer_description(source)?,
        });
    }

    let observed_pages = target_transactions
        .iter()
        .map(|transaction| transaction.page + 1)
        .max()
        .unwrap_or(0);
    Ok(TransferPlan {
        mappings,
        output_page_count: target_page_count.max(observed_pages),
        pages_to_clone: Vec::new(),
        pages_to_remove: Vec::new(),
        strategy: "deterministic-local-exact-capacity".into(),
        confidence: 1.0,
    })
}

fn infer_statement_date_format(
    transactions: &[crate::engine::model::Transaction],
) -> Result<&'static str, String> {
    let first = transactions
        .first()
        .ok_or_else(|| "cannot infer date format from an empty ledger".to_string())?;
    let separator = if first.date.contains('/') {
        '/'
    } else if first.date.contains('-') {
        '-'
    } else if first.date.contains('.') {
        '.'
    } else {
        return Err(format!("unrecognized date separator in '{}'", first.date));
    };

    let mut first_is_day = false;
    let mut first_is_month = false;
    let mut year_first = false;
    for transaction in transactions {
        if !transaction.date.contains(separator) {
            return Err("statement contains inconsistent date separators".into());
        }
        let parts: Vec<&str> = transaction.date.split(separator).collect();
        if parts.len() != 3 {
            return Err(format!(
                "expected three date parts in '{}'",
                transaction.date
            ));
        }
        let values: Vec<u32> = parts
            .iter()
            .map(|part| {
                part.parse::<u32>()
                    .map_err(|_| format!("non-numeric date part in '{}'", transaction.date))
            })
            .collect::<Result<_, _>>()?;
        if parts[0].len() == 4 {
            year_first = true;
        } else if values[0] > 12 {
            first_is_day = true;
        } else if values[1] > 12 {
            first_is_month = true;
        }
    }
    if year_first && (first_is_day || first_is_month) {
        return Err("statement mixes year-first and day/month-first dates".into());
    }
    if first_is_day && first_is_month {
        return Err("statement contains contradictory day/month ordering".into());
    }

    let format = match (year_first, first_is_day, first_is_month, separator) {
        (true, false, false, '-') => "YYYY-MM-DD",
        (true, false, false, '/') => "YYYY/MM/DD",
        (true, false, false, '.') => "YYYY.MM.DD",
        (false, true, false, '-') => "DD-MM-YYYY",
        (false, true, false, '/') => "DD/MM/YYYY",
        (false, true, false, '.') => "DD.MM.YYYY",
        (false, false, true, '-') => "MM-DD-YYYY",
        (false, false, true, '/') => "MM/DD/YYYY",
        (false, false, true, '.') => "MM.DD.YYYY",
        _ => {
            return Err(
                "date ordering is ambiguous; human review or a configured mapper is required"
                    .into(),
            )
        }
    };
    for transaction in transactions {
        convert_date(&transaction.date, format, format)?;
    }
    Ok(format)
}

fn transfer_description(transaction: &crate::engine::model::Transaction) -> Result<String, String> {
    let expected_amounts = usize::from(transaction.debit.is_some())
        + usize::from(transaction.credit.is_some())
        + usize::from(transaction.running_balance.is_some());
    let mut tokens: Vec<String> = transaction
        .raw_text
        .split_whitespace()
        .filter(|token| *token != transaction.date)
        .map(str::to_string)
        .collect();
    let mut removed = 0usize;
    let mut index = tokens.len();
    while index > 0 && removed < expected_amounts {
        index -= 1;
        let upper = tokens[index].to_ascii_uppercase();
        if matches!(upper.as_str(), "CR" | "DR" | "AUD" | "USD" | "EUR" | "GBP") {
            tokens.remove(index);
            continue;
        }
        let cleaned = upper
            .trim_matches(|character: char| {
                matches!(character, '$' | '€' | '£' | '(' | ')' | '+' | '-')
            })
            .trim_end_matches("CR")
            .trim_end_matches("DR")
            .replace(',', "");
        if Decimal::from_str_exact(&cleaned).is_ok() {
            tokens.remove(index);
            removed += 1;
        }
    }
    while tokens.last().is_some_and(|token| {
        matches!(
            token.to_ascii_uppercase().as_str(),
            "CR" | "DR" | "AUD" | "USD" | "EUR" | "GBP"
        )
    }) {
        tokens.pop();
    }
    let description = tokens.join(" ").trim().to_string();
    if description.is_empty() {
        return Err(format!(
            "source row at page {} line {} has no deterministic description",
            transaction.page, transaction.line_on_page
        ));
    }
    Ok(description)
}

/// Recompute running balances from an opening balance and a set of
/// transactions (using the codebase's sign convention: debit = money in,
/// credit = money out).
///
/// # Errors
///
/// Returns `BalanceError` if the running balance overflows or underflows
/// beyond the valid monetary range.
pub fn recompute_running_balances(
    opening: Decimal,
    txns: &mut [MappedTransaction],
) -> Result<(), String> {
    let mut balance = opening;
    for (idx, tx) in txns.iter_mut().enumerate() {
        let delta_in = tx.debit.unwrap_or(Decimal::ZERO);
        let delta_out = tx.credit.unwrap_or(Decimal::ZERO);

        // Check for overflow before arithmetic
        let new_balance = balance + delta_in - delta_out;
        if new_balance < Decimal::ZERO && balance >= Decimal::ZERO {
            // Allow negative balances (overdrafts) but log them
            tracing::warn!(
                "Negative running balance at transaction {}: {}",
                idx,
                new_balance
            );
        }

        // Check for unreasonable values (more than 1 trillion)
        if new_balance.abs() > Decimal::new(1000000000000, 0) {
            return Err(format!(
                "Balance overflow at transaction {}: balance = {}",
                idx, new_balance
            ));
        }

        balance = new_balance.round_dp(2);
        tx.running_balance = balance;
    }
    Ok(())
}

/// Convert a date string from one format to another.
/// Supports DD/MM/YYYY, MM/DD/YYYY, YYYY-MM-DD and variants with '-' or '.'.
///
/// # Errors
///
/// Returns the original string if parsing fails or the format is unrecognized.
pub fn convert_date(date_str: &str, from_format: &str, to_format: &str) -> Result<String, String> {
    if from_format == to_format {
        return Ok(date_str.to_string());
    }

    // Normalize the date string and detect separator
    let date_str = date_str.trim();
    if date_str.is_empty() {
        return Err("Empty date string".to_string());
    }

    let sep_char = if date_str.contains('/') {
        '/'
    } else if date_str.contains('-') {
        '-'
    } else if date_str.contains('.') {
        '.'
    } else {
        return Err(format!("Unrecognized date separator in '{}'", date_str));
    };

    let parts: Vec<&str> = date_str.split(sep_char).collect();
    if parts.len() != 3 {
        return Err(format!(
            "Expected 3 date parts in '{}', got {}",
            date_str,
            parts.len()
        ));
    }

    let p1 = parts[0].trim();
    let p2 = parts[1].trim();
    let p3 = parts[2].trim();

    // Validate that parts are numeric
    for (i, p) in [p1, p2, p3].iter().enumerate() {
        if p.parse::<u32>().is_err() {
            return Err(format!(
                "Non-numeric date part '{}' at position {} in '{}'",
                p,
                i + 1,
                date_str
            ));
        }
    }

    let (day, month, year) = match from_format {
        "DD/MM/YYYY" | "DD-MM-YYYY" | "DD.MM.YYYY" => (p1, p2, p3),
        "MM/DD/YYYY" | "MM-DD-YYYY" | "MM.DD.YYYY" => (p2, p1, p3),
        "YYYY-MM-DD" | "YYYY/MM/DD" | "YYYY.MM.DD" => (p3, p2, p1),
        _ => return Err(format!("Unrecognized source format '{}'", from_format)),
    };

    // Validate day and month ranges
    let day_num: u32 = day.parse().unwrap_or(0);
    let month_num: u32 = month.parse().unwrap_or(0);
    if day_num == 0 || day_num > 31 {
        return Err(format!("Invalid day value: {}", day));
    }
    if month_num == 0 || month_num > 12 {
        return Err(format!("Invalid month value: {}", month));
    }

    let sep = if to_format.contains('/') {
        "/"
    } else if to_format.contains('-') {
        "-"
    } else if to_format.contains('.') {
        "."
    } else {
        return Err(format!("Unrecognized target format '{}'", to_format));
    };

    let result = match to_format {
        "DD/MM/YYYY" | "DD-MM-YYYY" | "DD.MM.YYYY" => {
            format!("{day}{sep}{month}{sep}{year}")
        }
        "MM/DD/YYYY" | "MM-DD-YYYY" | "MM.DD.YYYY" => {
            format!("{month}{sep}{day}{sep}{year}")
        }
        "YYYY-MM-DD" | "YYYY/MM/DD" | "YYYY.MM.DD" => {
            format!("{year}{sep}{month}{sep}{day}")
        }
        _ => return Err(format!("Unrecognized target format '{}'", to_format)),
    };

    Ok(result)
}

/// Build a JSON audit report for the transfer operation.
/// Uses atomic write with checksum to prevent corruption.
pub fn write_transfer_audit(
    result: &TransferResult,
    source_path: &std::path::Path,
    target_path: &std::path::Path,
) -> std::io::Result<PathBuf> {
    let audit_dir = PathBuf::from("audit/transfers");
    std::fs::create_dir_all(&audit_dir)?;

    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
    let audit_path = audit_dir.join(format!("transfer_{timestamp}.json"));
    let tmp_path = audit_path.with_extension("tmp");

    let report = serde_json::json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "source_pdf": source_path.to_string_lossy(),
        "target_pdf": target_path.to_string_lossy(),
        "output_pdf": result.output_path.to_string_lossy(),
        "source_tx_count": result.source_tx_count,
        "target_tx_count": result.target_tx_count,
        "pages_added": result.pages_added,
        "pages_removed": result.pages_removed,
        "math_verified": result.math_verified,
        "visual_verified": result.visual_verified,
        "visual_score": result.visual_score,
        "math_imbalance": result.math_imbalance.to_string(),
        "stages_completed": result.stages_completed,
        "total_duration_secs": result.total_duration_secs,
        "corrections_applied": result.corrections_applied,
        "retries_attempted": result.retries_attempted,
        "synthesized_fonts_used": result.synthesized_fonts_used,
    });

    let pretty = serde_json::to_string_pretty(&report)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    // Atomic write: compute checksum, write to temp file, fsync, then rename
    let checksum = crc32fast::hash(pretty.as_bytes());
    let mut payload = pretty.into_bytes();
    payload.extend_from_slice(&checksum.to_le_bytes());

    let mut file = std::fs::File::create(&tmp_path)?;
    use std::io::Write;
    file.write_all(&payload)?;
    file.sync_all()?; // Ensure data is on disk

    std::fs::rename(&tmp_path, &audit_path)?;

    // Verify the write
    let verify_data = std::fs::read(&audit_path)?;
    if verify_data.len() >= 4 {
        let verify_checksum = crc32fast::hash(&verify_data[..verify_data.len() - 4]);
        let stored_checksum =
            u32::from_le_bytes(verify_data[verify_data.len() - 4..].try_into().unwrap());
        if verify_checksum != stored_checksum {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Checksum mismatch after write",
            ));
        }
    }

    tracing::info!(
        "Transfer audit written to {:?} (checksum {:08x})",
        audit_path,
        checksum
    );
    Ok(audit_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn recompute_balances_from_opening() {
        let mut txns = vec![
            MappedTransaction {
                target_page: 0,
                target_line: 0,
                date: "01/01/2026".into(),
                description: "Deposit".into(),
                debit: Some(dec!(500)),
                credit: None,
                running_balance: Decimal::ZERO,
                field_bboxes: FieldBboxes::default(),
            },
            MappedTransaction {
                target_page: 0,
                target_line: 1,
                date: "02/01/2026".into(),
                description: "Withdrawal".into(),
                debit: None,
                credit: Some(dec!(200)),
                running_balance: Decimal::ZERO,
                field_bboxes: FieldBboxes::default(),
            },
        ];

        let result = recompute_running_balances(dec!(1000), &mut txns);
        assert!(result.is_ok(), "Balance recomputation should succeed");

        assert_eq!(txns[0].running_balance, dec!(1500.00));
        assert_eq!(txns[1].running_balance, dec!(1300.00));
    }

    #[test]
    fn convert_date_dd_mm_to_mm_dd() {
        let result = convert_date("25/12/2026", "DD/MM/YYYY", "MM/DD/YYYY");
        assert_eq!(result, Ok("12/25/2026".to_string()));
    }

    #[test]
    fn convert_date_mm_dd_to_yyyy_mm_dd() {
        let result = convert_date("12/25/2026", "MM/DD/YYYY", "YYYY-MM-DD");
        assert_eq!(result, Ok("2026-12-25".to_string()));
    }

    #[test]
    fn convert_date_same_format_is_identity() {
        let result = convert_date("25/12/2026", "DD/MM/YYYY", "DD/MM/YYYY");
        assert_eq!(result, Ok("25/12/2026".to_string()));
    }

    #[test]
    fn convert_date_invalid_format_returns_error() {
        let result = convert_date("25-12-2026", "INVALID", "MM/DD/YYYY");
        assert!(result.is_err(), "Should return error for invalid format");
    }

    #[test]
    fn convert_date_non_numeric_parts_return_error() {
        let result = convert_date("AB/12/2026", "DD/MM/YYYY", "MM/DD/YYYY");
        assert!(result.is_err(), "Should return error for non-numeric parts");
    }

    #[test]
    fn convert_date_empty_string_returns_error() {
        let result = convert_date("", "DD/MM/YYYY", "MM/DD/YYYY");
        assert!(result.is_err(), "Should return error for empty string");
    }

    fn transfer_tx(
        page: usize,
        line_on_page: usize,
        date: &str,
        raw_text: &str,
        amount: Decimal,
        balance: Decimal,
        bbox: Option<[f32; 4]>,
    ) -> crate::engine::model::Transaction {
        crate::engine::model::Transaction {
            page,
            line_on_page,
            date: date.into(),
            raw_text: raw_text.into(),
            debit: Some(amount),
            credit: None,
            running_balance: Some(balance),
            bbox,
            field_bboxes: FieldBboxes::default(),
            provenance: crate::engine::model::Provenance::Computed,
            category: None,
            canonical: Default::default(),
        }
    }

    #[test]
    fn deterministic_plan_maps_exact_capacity_without_provider() {
        let source = vec![
            transfer_tx(
                0,
                1,
                "26/12/2026",
                "26/12/2026 SECOND SHOP 20.00 970.00",
                dec!(20.00),
                dec!(970.00),
                Some([0.0; 4]),
            ),
            transfer_tx(
                0,
                0,
                "25/12/2026",
                "25/12/2026 FIRST SHOP 10.00 990.00",
                dec!(10.00),
                dec!(990.00),
                Some([0.0; 4]),
            ),
        ];
        let target = vec![
            transfer_tx(
                1,
                0,
                "12/26/2025",
                "target two",
                dec!(1.00),
                dec!(99.00),
                Some([10.0, 20.0, 30.0, 40.0]),
            ),
            transfer_tx(
                0,
                0,
                "12/25/2025",
                "target one",
                dec!(1.00),
                dec!(100.00),
                Some([10.0, 20.0, 30.0, 40.0]),
            ),
        ];

        let plan = plan_transaction_transfer_deterministic(&source, &target, 2).unwrap();
        assert_eq!(plan.strategy, "deterministic-local-exact-capacity");
        assert_eq!(plan.confidence, 1.0);
        assert_eq!(plan.output_page_count, 2);
        assert!(plan.pages_to_clone.is_empty());
        assert!(plan.pages_to_remove.is_empty());
        assert_eq!(plan.mappings.len(), 2);
        assert_eq!(plan.mappings[0].source_index, 1);
        assert_eq!(plan.mappings[0].converted_date, "12/25/2026");
        assert_eq!(plan.mappings[0].adapted_description, "FIRST SHOP");
        assert_eq!(plan.mappings[1].source_index, 0);
        assert_eq!(plan.mappings[1].target_page, 1);
    }

    #[test]
    fn deterministic_plan_rejects_ambiguous_date_ordering() {
        let source = vec![transfer_tx(
            0,
            0,
            "01/02/2026",
            "01/02/2026 SHOP 10.00 90.00",
            dec!(10.00),
            dec!(90.00),
            Some([0.0; 4]),
        )];
        let target = vec![transfer_tx(
            0,
            0,
            "02/03/2026",
            "target",
            dec!(1.00),
            dec!(99.00),
            Some([0.0; 4]),
        )];
        let error = plan_transaction_transfer_deterministic(&source, &target, 1).unwrap_err();
        assert!(error.contains("ambiguous"));
    }

    #[test]
    fn deterministic_plan_rejects_capacity_or_geometry_mismatch() {
        let source = vec![transfer_tx(
            0,
            0,
            "25/12/2026",
            "25/12/2026 SHOP 10.00 90.00",
            dec!(10.00),
            dec!(90.00),
            Some([0.0; 4]),
        )];
        let target = vec![
            transfer_tx(
                0,
                0,
                "12/25/2026",
                "target one",
                dec!(1.00),
                dec!(99.00),
                Some([0.0; 4]),
            ),
            transfer_tx(
                0,
                1,
                "12/26/2026",
                "target two",
                dec!(1.00),
                dec!(98.00),
                Some([0.0; 4]),
            ),
        ];
        assert!(plan_transaction_transfer_deterministic(&source, &target, 1)
            .unwrap_err()
            .contains("equal row capacity"));

        let no_geometry = vec![transfer_tx(
            0,
            0,
            "12/25/2026",
            "target",
            dec!(1.00),
            dec!(99.00),
            None,
        )];
        assert!(
            plan_transaction_transfer_deterministic(&source, &no_geometry, 1)
                .unwrap_err()
                .contains("no editable geometry")
        );
    }

    #[test]
    fn transfer_stage_labels_all_defined() {
        let stages = [
            TransferStage::AnalyzeSource,
            TransferStage::AnalyzeTarget,
            TransferStage::AiFormatMapping,
            TransferStage::ComputeBalances,
            TransferStage::PdfSurgery,
            TransferStage::VisualFidelityCheck,
            TransferStage::MathVerificationEngine,
            TransferStage::MathVerificationGemini,
            TransferStage::FinalAudit,
        ];
        for s in stages {
            assert!(!s.label().is_empty());
            let (lo, hi) = s.fraction_range();
            assert!(lo < hi);
        }
    }
}
