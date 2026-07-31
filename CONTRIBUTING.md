# Contributing

We welcome contributions to the Bank Statement Fidelity Editor!

## Development Setup

Follow [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) from a clean checkout. The executable base state does not require cloud credentials. Install `requirements-ci.txt`, then run the platform verification command:

```bash
./scripts/verify-base-state.sh
```

On Windows PowerShell, run:

```powershell
./scripts/verify-base-state.ps1
```

Copy `.env.example` to `.env` only when testing an optional provider or licensed capability that explicitly requires it.

## Code Quality

### Linting & Formatting

All code must pass the platform base-state command before merge. For a focused Rust edit, the minimum local checks are:

```bash
cargo fmt --all -- --check
cargo clippy --locked --lib --bins -- -D warnings
```

### Mutation Testing

To ensure high-quality code and robust logic, we highly recommend using mutation testing locally before submitting a PR:

```bash
cargo install cargo-mutants
cargo mutants
```

This verifies that the test suite actually catches bugs in the business logic (especially in `src/engine/balance.rs`, `src/engine/verification.rs`, and `src/engine/offline_parser.rs`).

### Full Validation

Run the complete platform command in [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) before submitting. During the remediation program, also complete the ticket evidence fields, run all prior P0 regressions, and update the owning phase manifest under `docs/remediation/evidence/`.

## Architecture Guidelines

- **Fallback chains:** Every new cloud integration must have an offline fallback. Never leave a pipeline stage with a single point of failure.
- **API availability:** New API keys must be added to `ApiAvailability` in `src/app/config.rs` and checked at boot time.
- **Backend preferences:** New backends should be added to the appropriate enum (`AiProviderMode`, `DocumentParserMode`, `VerificationMode`, `PdfEngineMode`) and surfaced in the Backend Preferences UI in `src/app/modals.rs`.
- **Error handling:** Prefer typed errors with context. No silent failures or unchecked unwraps in production paths.
- **Secrets:** Never log, print, or commit API key values. Use `.env.example` for templates.

## Documentation Parity

Before merging, verify documentation matches code:

- [ ] Version strings in `README.md`, `docs/TECH_STACK.md`, `AGENTS.md`, `CHANGELOG.md`, and `Cargo.toml` are consistent.
- [ ] Default backend/parser mentions match the `#[default]` attributes in `src/app/config.rs`.
- [ ] Dependency version numbers in `docs/TECH_STACK.md` match `Cargo.toml`.
- [ ] Engine descriptions match the actual implementations in `src/pdf/`.
- [ ] OCR / Typst / Feature-gated capabilities documented with correct prerequisites.
- [ ] Comments in `src/pdf/selector.rs` accurately describe engine priority and fallback behavior.

## Files

- **`.env.example`** — Template with all configurable keys (safe to commit)
- **`.env`** — Your local secrets (gitignored, never commit)
- **`AGENTS.md`** — Agent development rules and autonomy boundaries
- **`QUICKSTART.md`** — Setup guide for new developers
- **`CHANGELOG.md`** — Release history
- **`docs/DEVELOPMENT.md`** — Reproducible Windows/macOS/Linux-development bootstrap and verification
- **`docs/remediation/MASTER_PLAN.md`** — Sequenced implementation tickets and mandatory gates
- **`docs/remediation/EVIDENCE_POLICY.md`** — Ticket and phase proof requirements
