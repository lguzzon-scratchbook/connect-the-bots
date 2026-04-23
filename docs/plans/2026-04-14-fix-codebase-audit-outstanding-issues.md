---
title: "fix: Codebase Audit — Outstanding Issues"
type: fix
status: active
date: 2026-04-14
---

# Codebase Audit — Outstanding Issues

## Context

A full codebase audit on 2026-04-14 explored all 82 source files across 8 crates. 13 bugs were fixed in the same session (UTF-8 panics, infinite loops, off-by-one errors, regex caching, etc.). This plan tracks the remaining issues that require larger refactoring or design decisions before they can be addressed.

## Fixed in initial pass (for reference)

- UTF-8 byte-slice panics in `truncation.rs`, `agent/lib.rs`, `prompt_builder.rs`, `codergen_handler.rs`, `parser.rs`
- Empty `old_string` infinite loop in `edit_file.rs`
- `LoopDetector` panic on `window_size=0`
- `normalize_label` regex compiled on every call
- Engine step counter off-by-one (`>` vs `>=`)
- `codergen_handler` non-zero exit with stdout treated as success
- Ripgrep pattern-as-flag injection (missing `--` separator)
- `prompt_builder` HashMap non-determinism in `with_context`

---

## Outstanding Issues

### P1 — Correctness bugs (wrong behavior in production paths)

#### 1. Engine: `loop_restart` clears outcomes before goal gate check

**File:** `crates/attractor-pipeline/src/engine.rs:284-287`

When an edge has `loop_restart = true`, `completed_nodes` and `node_outcomes` are both cleared immediately. If this edge eventually leads to an `Msquare` exit node, `enforce_goal_gates` receives an empty `node_outcomes` map, so every goal gate from the cleared loop is invisible and passes silently.

**Fix:** Clear `completed_nodes` and `node_outcomes` *after* the exit node check, or evaluate goal gates before clearing on a `loop_restart` edge.

**Risk:** Goal gates can be silently bypassed in pipelines that use `loop_restart` edges.

---

#### 2. Engine: Checkpoint doesn't store/restore `step_count` or `total_cost`

**File:** `crates/attractor-pipeline/src/engine.rs:134-168`

After checkpoint restore, `step_count` and `total_cost` both start at 0. A pipeline that ran 190 of 200 allowed steps, checkpointed, and resumes will get another 200 steps (390 total). Budget tracking is similarly reset.

**Fix:** Add `step_count: u64` and `total_cost: f64` fields to `PipelineCheckpoint`. Serialize/restore them alongside `completed_nodes` and `node_outcomes`.

**Risk:** Safety limits (step count, budget) are ineffective across checkpoint-resume cycles.

---

#### 3. Engine: `execute_with_retry` is never called (dead retry system)

**File:** `crates/attractor-pipeline/src/retry.rs` (entire module), `crates/attractor-pipeline/src/engine.rs:209,225`

`retry.rs` implements `execute_with_retry` and nodes have a `max_retries` field, but `engine.rs` calls `handler.execute(...)` directly without wrapping it. The entire retry system is dead code from the engine's perspective.

**Fix:** Wrap handler execution in `execute_with_retry` when `node.max_retries > 0`, or remove the retry module and `max_retries` field if retries are not intended.

**Risk:** Users who set `max_retries` on nodes get no retry behavior.

---

#### 4. Engine: Non-exit terminal with `Retry`/`Skipped` status silently succeeds

**File:** `crates/attractor-pipeline/src/engine.rs:304-313`

When a node completes with no outgoing edge (not an exit node), only `StageStatus::Fail` returns an error. `Retry` or `Skipped` status silently breaks the loop as if the pipeline succeeded.

**Fix:** At minimum, log a warning for `Retry`/`Skipped` at a terminal position. Consider returning an error for `Retry` since the node explicitly requested another attempt.

---

#### 5. Agent: `LoopDetector` not integrated into `AgentSession`

**File:** `crates/attractor-agent/src/lib.rs:39-41,53`

`SessionConfig` has `enable_loop_detection` and `loop_detection_window` fields, and `LoopDetector`/`SteeringInjector` are exported, but `AgentSession` has no `LoopDetector` field and `process_single_input` never calls `record_and_check`. The loop detection subsystem is entirely disconnected.

**Fix:** Add a `LoopDetector` field to `AgentSession`, instantiate it from config, call `record_and_check` in the tool execution loop, and inject `SteeringInjector::loop_detected_message` when triggered.

**Risk:** Agent tool-call loops are only stopped by `max_tool_rounds`, not by detection of repetitive patterns.

---

#### 6. Agent: `FidelityMode` subsystem disconnected from session

**File:** `crates/attractor-agent/src/fidelity.rs`, `crates/attractor-agent/src/lib.rs:293-373`

`apply_fidelity` is exported but never called in `build_request`. `SessionConfig` doesn't have a `fidelity_mode` field. Additionally, `Compact` and `Summary` modes are unimplemented stubs that silently return all messages unchanged.

**Fix:** Either integrate fidelity into `build_request` with a config field, or remove the module until it's needed to avoid a misleading public API.

---

### P2 — LLM adapter bugs (affect specific provider integrations)

#### 7. OpenAI: Wrong message format for Responses API tool history

**File:** `crates/attractor-llm/src/openai.rs:197-213,232-238`

`Role::Tool` messages produce `role: "tool"` with `tool_call_id`, but the Responses API uses `"function_call_output"` items with `call_id`. Tool call `arguments` are also double-serialized as a string. Multi-turn tool-use conversations will produce malformed requests.

**Fix:** Implement proper `function_call` / `function_call_output` item types for the Responses API input format, or switch to the Chat Completions API (`/v1/chat/completions`) which uses the `role: "tool"` format.

---

#### 8. Gemini: `functionResponse.name` set to call ID instead of function name

**File:** `crates/attractor-llm/src/gemini.rs:220-228`

The `functionResponse.name` field is set to `tool_call_id` (a synthetic UUID from line 148), not the actual function name. Gemini requires this to match the `functionDeclaration.name`. The `ToolResult` `ContentPart` doesn't carry the function name.

**Fix:** Add a `tool_name: String` field to `ContentPart::ToolResult` (currently only has `tool_call_id` and `content`), and populate it from the tool call's name when constructing tool results.

---

#### 9. Anthropic/OpenAI: `retry_after` parsed from wrong location

**File:** `crates/attractor-llm/src/anthropic.rs:326-329`, `crates/attractor-llm/src/openai.rs:285-288`

Both adapters try to extract `retry_after` from `response_body["error"]["retry_after"]`. Neither API puts this in the JSON body — Anthropic uses `retry-after` HTTP header, OpenAI uses `Retry-After` header. The fallback `unwrap_or(1000)` (1 second) is always used.

**Fix:** Extract `retry-after` from the HTTP response headers. This requires threading the response headers through to the retry logic.

---

#### 10. Anthropic: `Thinking` block missing required `signature` field

**File:** `crates/attractor-llm/src/anthropic.rs:202-210`

When serializing `ContentPart::Thinking` back to the API for multi-turn conversations, the `signature` field is omitted when `None`. Anthropic requires this field to verify thinking block integrity. Omitting it causes API validation errors.

**Fix:** Always include `signature` in the serialized thinking block. If `None`, either skip the thinking block entirely or error early.

---

#### 11. All adapters: `stream()` returns empty stub but `supports_streaming()` returns true

**File:** `crates/attractor-llm/src/anthropic.rs:434`, all adapter `stream()` implementations

All three `stream()` implementations return `tokio_stream::empty()`. Anthropic's `supports_streaming()` returns `true`. Callers who check streaming support before calling `stream()` get no data with no error.

**Fix:** Either implement streaming or have `supports_streaming()` return `false` until streaming is implemented.

---

### P3 — Design issues (not bugs but limit correctness/safety)

#### 12. Parallel handler does not execute branches in parallel

**File:** `crates/attractor-pipeline/src/handlers/parallel.rs`

`ParallelHandler` only populates `suggested_next_ids` but does not actually fork execution. The engine follows one edge via `select_edge` (which uses `suggested_next_ids` as a tie-breaking fallback). Pipelines using `shape="component"` silently execute only one branch.

**Fix:** Implement actual parallel execution in the engine when `suggested_next_ids` contains multiple targets, or document this as a known limitation and consider renaming the handler to avoid confusion.

---

#### 13. `validate.rs` uses `std::process::exit(1)` instead of returning error

**File:** `crates/attractor-cli/src/commands/validate.rs:26`

`cmd_validate` calls `std::process::exit(1)` on validation errors, bypassing Drop impls and preventing callers from catching the error. `launch.rs` works around this with its own inline validation loop.

**Fix:** Replace `std::process::exit(1)` with `anyhow::bail!("Validation failed")` for consistency with other commands.

---

#### 14. `run.rs` `--fresh` flag silently ignored for per-pipeline checkpoints in directory mode

**File:** `crates/attractor-cli/src/commands/run.rs:150,198`

`cmd_run_dir` clears only the manifest file when `--fresh` is set. Per-pipeline checkpoints are not cleared because `cmd_run` is called with `fresh = false` hardcoded.

**Fix:** Pass the `fresh` flag through to `cmd_run` calls within `cmd_run_dir`.

---

#### 15. `DefaultHasher` used for checkpoint/logs directory names (not stable across Rust versions)

**File:** `crates/attractor-cli/src/commands/run.rs:11-12,26`

`DefaultHasher` can change its algorithm between Rust releases. Rebuilding the binary can orphan existing checkpoint directories, causing pipelines to restart from scratch with no warning.

**Fix:** Use a stable hash function (e.g., `fnv`, `xxhash`, or a simple `crc32`) for the path hash.

---

#### 16. Stale `preferred_label` context key persists across nodes

**File:** `crates/attractor-pipeline/src/engine.rs:256-260`

`preferred_label` is set in context only when `outcome.preferred_label.is_some()`. If a node produces no preferred label, the context key from the previous node remains. An edge condition `preferred_label=X` on a later node could match a stale value.

**Fix:** Always set `preferred_label` — use an empty string when `outcome.preferred_label` is `None` to clear the stale value.

---

## Suggested priority order

1. Issues 1-4 (engine correctness) — these affect pipeline execution semantics
2. Issues 5-6 (agent integration) — dead subsystems that users may rely on
3. Issues 7-10 (LLM adapters) — affect multi-turn tool-use with OpenAI/Gemini
4. Issues 11-16 (design/UX) — quality of life and robustness
