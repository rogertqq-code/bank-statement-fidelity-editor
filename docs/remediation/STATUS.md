# Remediation Program Status

**Repository head audited:** `41993a8daf73266eaae5d6d4abcc2cc13ac85662`
**Working branch:** `remediation/phase-03-unified-runtime`
**Current phase:** Phase 03 / unified runtime complete
**Current gate:** Gate 03 — `PASS`
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
| 01 | 01 | Complete | Windows, macOS, Linux development, Clippy, format, and optional-Pro jobs passed in CI run `30653780202`. |
| 02 | 02 | Complete | All five P0 failure classes, related exact-success defects, canonical geometry, and mandatory Windows/macOS/Linux P0 regressions passed in CI run `30673473276`. |
| 03 | 03 | Complete | Unified runtime protocol, authoritative state, routed results, cancellation/deadlines, isolated storage, atomic configuration, truthful capabilities, dispositions, and bounded diagnostics passed in CI run `30698338534`. |
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

Gate 00 established governance and the release freeze at `5c3678c`; Gate 01 established the executable cross-platform base state; Gate 02 closed the critical integrity failure classes at candidate `500167b`; and Gate 03 closed the unified runtime at candidate `c84c631` in CI run `30698338534`. All work remains on remediation branches and the default branch is untouched.

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
| Windows x64 CI | PASS — job `91233067547` |
| macOS Apple Silicon CI | PASS — job `91233067603` |
| Gate 01 evidence manifest | PASS — run `30653780202`, candidate `7cb54c2` |

## Gate 02 checklist

| Requirement | State |
|---|---|
| Strict hash-backed Rust/Python `ApplyReport` | PASS |
| No-overlap Python edit is non-destructive | PASS |
| Twenty-edit exact transaction stress | PASS |
| Preview and renderer edit-set identity | PASS |
| Unbalanced preview blocks before mutation | PASS |
| Zero-row extraction/balance rejects success | PASS |
| Independent content-addressed snapshots | PASS |
| Snapshot tamper and missing-object rejection | PASS |
| All-or-nothing output/evidence commit barrier | PASS |
| Exactly-once terminal result contract | PASS |
| Per-keystroke PDF writer removed | PASS |
| Canonical native/PyMuPDF top-left geometry | PASS |
| Windows P0 regression job | PASS — job `91296489736` |
| macOS P0 regression job | PASS — job `91296489716` |
| Linux P0 regression job | PASS — job `91296489755` |
| Gate 01 base-state, Clippy, and format regressions | PASS |
| Gate 02 evidence manifest | PASS — run `30673473276`, candidate `500167b` |

## Gate 03 checklist

| Requirement | State |
|---|---|
| One authoritative workflow state/event model | PASS |
| Typed job/document/correlation envelope | PASS |
| Job-scoped result routing with no cross-talk | PASS |
| Exactly-one terminal result with bounded cancellation/timeouts | PASS |
| Bounded graceful shutdown and explicit telemetry flush | PASS |
| Explicit interactive/headless fallback policy | PASS |
| Isolated platform-root document/run workspaces | PASS |
| Generation-tracked atomic configuration ownership | PASS |
| Truthful capability registry and disabled unavailable actions | PASS |
| Unsupported v1 remote-engine surface removed | PASS |
| Standardized operation dispositions and artifact postconditions | PASS |
| Structured bounded privacy-safe diagnostics | PASS |
| Windows base-state and P0 regressions | PASS — jobs `91364889578`, `91366292780` |
| macOS base-state and P0 regressions | PASS — jobs `91364889606`, `91366292775` |
| Linux base-state and P0 regressions | PASS — jobs `91364889613`, `91366292772` |
| Format, strict production Clippy, optional-Pro smoke | PASS |
| Gate 03 evidence manifest | PASS — run `30698338534`, candidate `c84c631` |

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

1. Commit and push the evidence-only Gate 03 closure without modifying the default branch.
2. Create the Phase 04 Python/PyMuPDF fortification branch from the verified Gate 03 checkpoint.
3. Define the versioned cross-language operation schema and exact Rust/Python golden fixtures.
4. Harden worker lifecycle, scratch publication, crash/restart, cancellation, timeouts, resource cleanup, compatibility probes, and standalone contract tests while keeping every prior gate mandatory.
