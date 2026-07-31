# ADR-0003: Conditional Optional Local LLM

**Status:** Accepted for evaluation; implementation is conditional on evidence
**Date:** 2026-07-31
**Decision owner:** Repository owner

## Context

The application currently exposes natural-language editing, AI analysis, parser fallbacks, categorization, and cloud-provider integrations. A local LLM could improve offline usability, reduce provider dependence, and keep optional language tasks on-device. It could also increase package size, memory use, startup time, platform complexity, nondeterminism, and the risk that generated output is mistaken for verified financial truth.

## Decision

Evaluate an optional local LLM after deterministic extraction, editing, and verification contracts are repaired. Integrate it only if a benchmark proves material user benefit within agreed Windows/macOS resource budgets.

The local LLM may assist with:

| Candidate use | Allowed output |
|---|---|
| Natural-language edit intent | A typed, reviewable proposal that resolves to deterministic `UserEdit` objects. |
| Transaction categorization | Suggested category/confidence; never a document mutation without review. |
| Error and audit explanation | Plain-language explanation derived from typed evidence. |
| Parser ambiguity review | Candidate field/row interpretation with provenance and confidence. |
| Cloud fallback | Optional local proposal when cloud AI is unavailable, subject to the same deterministic gates. |

The local LLM may **not** determine whether a statement is balanced, declare visual fidelity, bypass completeness, approve a destructive edit, produce a terminal success, write directly to a PDF, or alter audit evidence.

## Evaluation gate

A candidate model/runtime must be assessed on representative supported machines for:

1. Task accuracy against a versioned synthetic/redacted benchmark.
2. JSON-schema adherence and invalid-output rate.
3. Latency, peak memory, disk/package size, startup cost, and thermal behavior.
4. Determinism at fixed configuration and safe handling of malformed/adversarial prompts.
5. Windows and macOS packaging/licensing feasibility.
6. Offline operation with all network access blocked.
7. Graceful absence: the complete verified core workflow remains functional with the local model disabled or uninstalled.

## Integration boundary

If approved, the model is an optional capability behind the same typed job protocol, deadline, cancellation, and result routing as other providers. Its output is parsed against a strict schema, validated against the canonical transaction/document model, displayed for review where material, and independently verified after any approved action.

## Rejection condition

Do not ship a local LLM if it fails the benchmark, requires unacceptable resources, has incompatible redistribution terms, reduces clean-machine reliability, or duplicates deterministic logic without measurable benefit. Record a rejection report rather than forcing an AI feature into the release.
