# Remediation Program Status

**Repository head audited:** `41993a8daf73266eaae5d6d4abcc2cc13ac85662`
**Working branch:** `remediation/phase-00-governance-baseline`
**Current phase:** Phase 00 / backlog and governance
**Current gate:** Gate 00 — `IN PROGRESS`
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
| 00 | 00 | In progress | Backlog, ADRs, release freeze, evidence governance, and baseline inventory. |
| 01 | 01 | Planned | Reproducible Windows/macOS and Linux-development CI baseline. |
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

## Gate 00 checklist

| Requirement | State |
|---|---|
| Deploy-key read access verified | Complete |
| Deploy-key write dry-run verified | Complete |
| Fresh isolated remediation clone and branch | Complete |
| Accepted architecture decisions recorded | Complete |
| 57 findings mapped to unique tickets | Complete — validator reports 57/57 mapped and 129 unique tickets |
| Release publication frozen | Complete in branch; remote merge pending |
| Ticket template and evidence policy | Complete |
| Baseline source/fixture/toolchain hash manifest | Pending |
| Gate 00 manifest | Pending |
| Branch validation and clean diff review | Pending |
| Commit/push for owner review | Pending explicit repository-instruction confirmation at the branch checkpoint |

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

1. Create the immutable baseline and fixture hash inventory.
2. Create and validate the Gate 00 evidence manifest.
3. Run documentation/YAML/plan validators and secret/diff hygiene checks.
4. Present the branch checkpoint for commit and push authorization required by `AGENTS.md`.
