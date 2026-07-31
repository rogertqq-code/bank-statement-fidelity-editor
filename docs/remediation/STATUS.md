# Remediation Program Status

**Repository head audited:** `41993a8daf73266eaae5d6d4abcc2cc13ac85662`
**Working branch:** `remediation/phase-01-base-state`
**Current phase:** Phase 01 / executable base state
**Current gate:** Gate 01 — `IN PROGRESS`
**Release publication:** Frozen

## Accepted owner decisions

| Decision | Status | Record |
|---|---|---|
| Python and PyMuPDF remain permanent first-class production components and must be fortified. | Accepted | `adr/ADR-0001-python-first-production-pipeline.md` |
| Windows x64 and macOS Apple Silicon are mandatory customer platforms. | Accepted | `adr/ADR-0002-windows-macos-production-support.md` |
| A local LLM may be added only after benchmark, resource, schema, packaging, and deterministic-safety gates pass. | Accepted for evaluation | `adr/ADR-0003-conditional-local-llm.md` |
| Functional work precedes non-blocking privacy hardening, but critical data-integrity and active secret/data-exposure defects remain immediate blockers. | Accepted with release safeguards | `adr/ADR-0004-functional-first-privacy-final.md` |
| Repository access uses a write-enabled repository-scoped deploy key. | Verified | Read and write dry-run against audited head succeeded. |
| Remote changes use phase/ticket branches; `master` is not modified directly. | Active default | `MASTER_PLAN.md` |

## Phase ledger

| Phase | Gate | State | Blocking outcome |
|---:|---:|---|---|
| 00 | 00 | Complete | Backlog, ADRs, release freeze, evidence governance, and baseline inventory passed and were pushed at `5c3678c`. |
| 01 | 01 | In progress | Linux development checks pass; Windows/macOS branch CI pending. |
| 02 | 02 | Planned | Five P0 integrity defects and active blockers closed. |
| 03 | 03 | Planned | Unified runtime protocol, state, storage, cancellation, and recovery. |
| 04 | 04 | Planned | Permanent Python/PyMuPDF pipeline bulletproofed. |
| 05 | 05 | Planned | Extraction and financial invariants verified. |
| 06 | 06 | Planned | Exact PDF editing, fonts, geometry, segmentation, and atomic output. |
| 07 | 07 | Planned | Independent fail-closed verification. |
| 08 | 08 | Planned | Local LLM go/no-go and conditional integration. |
| 09 | 09 | Planned | Coherent accessible GUI, batch, audit, and recovery UX. |
| 10 | 10 | Planned | Self-contained Windows/macOS packages. |
| 11 | 11 | Planned | Functional corpus, fault, performance, provider, and package qualification. |
| 12 | 12 | Planned | Final non-blocking privacy, secrets, dependency, and supply-chain hardening. |
| 13 | 13 | Planned | Complete post-hardening rerun and signed release. |

## Completed checkpoint

Gate 00 passed locally, was committed at `5c3678c`, and was pushed to `remediation/phase-00-governance-baseline`. The default branch remains untouched.

## Gate 01 checklist

| Requirement | State |
|---|---|
| Host-neutral Rust 1.89.0 toolchain and Cargo configuration | Local PASS |
| Windows-only development dependencies target-scoped | Local PASS |
| Pinned Python/PyMuPDF base and optional-Pro package manifests | Local PASS |
| Production Python bridge smoke | Local PASS |
| `cargo fmt --all -- --check` | Local PASS |
| Strict production Clippy | Local PASS |
| `cargo check --locked --all-targets` | Linux development PASS |
| Library tests | 228 passed, 0 failed, 0 ignored |
| Runtime actor smoke | 2 passed, 0 failed |
| Configuration-free CLI startup regressions | 2 passed, 0 failed |
| Production binary build and direct help/version startup | Linux development PASS; stderr empty without configuration |
| Generated logs, scratch scripts, outputs, and machine Pdfium DLLs removed | Local PASS |
| CI and release workflow YAML validation | Local PASS |
| Phase 01 validator | Local PASS |
| Windows x64 CI | Pending branch push |
| macOS Apple Silicon CI | Pending branch push |
| Gate 01 evidence manifest | Pending cross-platform results |

## Open decisions with later blocking phases

| Decision | Default until owner responds | Blocks |
|---|---|---|
| macOS Intel/universal support | Apple Silicon only | Phase 10 packaging |
| PyMuPDF Pro redistribution/license model | User-provided key; core capability must be truthful without it | Phase 04/10 |
| Required cloud providers for v1 | Optional providers remain quarantined until contract-qualified | Phase 05/11 |
| Local OCR distribution | Optional component with explicit model capability | Phase 05/10 |
| Signing identities/certificates | Build unsigned internal candidates only; no GA | Phase 10/13 |
| Remote processing service | Excluded from v1 unless separately approved and designed | Phase 09/10 |
| Typst reconstruction | Separate explicitly non-fidelity export or remove | Phase 06 |

## Next executable work

1. Commit and push the validated Phase 01 branch to trigger Windows, macOS, and Linux development CI.
2. Diagnose every cross-platform failure; do not waive or soften a gate.
3. Capture the final CI run IDs, exact outcomes, and artifact hashes in the Gate 01 manifest.
4. Close Gate 01 only after all mandatory platform jobs pass from the remote branch.
