# Contributions to Grok Build (Upstream Fork)

This document lists every substantive engineering change made on top of the
upstream Grok Build codebase ([xai-org/grok-build](https://github.com/xai-org/grok-build),
Apache-2.0). It is generated from the actual commit history of this fork
(`914103a..HEAD`), not a summary written after the fact.

## New Code (not present in upstream)

- **`crates/codegen/xai-grok-bedrock/`** — entire new crate. `BedrockClient`
  (the `SamplingBackend` implementation for Amazon Bedrock), Converse and
  ConverseStream API integration via `aws-sdk-bedrockruntime`, and the
  `bedrock_smoke_test` standalone verification binary.
- **`crates/codegen/xai-grok-sampling-types/src/backend.rs`** — the
  `SamplingBackend` trait itself: the provider abstraction that did not
  exist in upstream Grok Build.
- **`crates/codegen/xai-grok-sampling-types/src/events.rs`,
  `metrics.rs`, `request_id.rs`** — `SamplingEvent`, `InferenceLatencyStats`,
  and `RequestId` moved here from `xai-grok-sampler`, so the provider-agnostic
  trait and its associated types no longer depend on the xAI-specific
  sampler crate. Re-export shims were left at the old `xai-grok-sampler`
  paths so no existing xAI-side import needed to change.
- **`crates/codegen/xai-grok-sampling-types/src/error.rs`** — added
  `SamplingError::api_error(status: u16, message)`, a constructor that lets
  `BedrockClient` produce a first-class `Api` error without depending on
  `reqwest::StatusCode` directly.

## Modified Upstream Code

| File | Reason |
|---|---|
| `Cargo.toml` | Added `aws-config`, `aws-sdk-bedrockruntime` workspace dependencies; added `xai-grok-bedrock` as a workspace member. |
| `crates/codegen/xai-grok-agent/templates/prompt.md` | Removed the hardcoded "released by xAI" clause from the system prompt's identity line. |
| `crates/codegen/xai-grok-agent/src/prompt/context.rs` | Changed `DEFAULT_SYSTEM_PROMPT_LABEL` from `"Grok"` to `"AtheCode"`. |
| `crates/codegen/xai-grok-models/default_models.json` | Added a `provider` field to every entry; added 11 Bedrock model entries with `provider: "bedrock"`, per-model `max_completion_tokens`/`temperature`, and `system_prompt_label`. |
| `crates/codegen/xai-grok-models/src/lib.rs` | Added `provider` to `DefaultModelEntry` (serde default `"xai"` for backward compatibility). |
| `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs` | Added a persistent "AtheCode" status-bar item in the active chat view. |
| `crates/codegen/xai-grok-pager/src/app/app_view.rs` | Added `bedrock_notice_shown` flag, gating a one-time Bedrock tool-execution notice. |
| `crates/codegen/xai-grok-pager/src/app/dispatch/settings/setters.rs` | `set_default_model` now shows a one-time toast on first switch to a Bedrock model, warning that generated code is not automatically tested. |
| `crates/codegen/xai-grok-pager/src/slash/commands/model.rs` | `/model` dropdown groups xAI models before Bedrock models, labels each Bedrock row with its upstream vendor; Bedrock models now switch identically to xAI models. |
| `crates/codegen/xai-grok-sampler/Cargo.toml` | Added `async-trait` (for the `SamplingBackend` impl). |
| `crates/codegen/xai-grok-sampler/src/client.rs` | Added `impl SamplingBackend for SamplingClient`, delegating to the existing `conversation_collect`/`conversation_stream*` methods — no new xAI request/response conversion logic. |
| `crates/codegen/xai-grok-sampler/src/events.rs`, `metrics.rs`, `types.rs` | Reduced to re-export shims pointing at the moved types in `xai-grok-sampling-types`. |
| `crates/codegen/xai-grok-sampling-types/Cargo.toml` | Added `async-trait`, `futures-util`, `uuid` (needed by the moved-in types and the new trait). |
| `crates/codegen/xai-grok-sampling-types/src/lib.rs` | Exported the new `backend`, `events`, `metrics`, `request_id` modules. |
| `crates/codegen/xai-grok-shell/Cargo.toml` | Added `xai-grok-bedrock` dependency. |
| `crates/codegen/xai-grok-shell/src/agent/config.rs` | Added `provider`/`system_prompt_label` fields to `ModelInfo`/`ModelEntryConfig`/`DefaultModelJson`; added the `SamplingBackendConfig` enum, `resolve_sampling_backend_config`, `validate_bedrock_env`, and `merge_bedrock_defaults` (keeps Bedrock models selectable after a fetched xAI catalog replaces the built-in defaults). |
| `crates/codegen/xai-grok-shell/src/agent/models.rs` | Added `ModelsManager::model_provider` and `model_info` accessors. |
| `crates/codegen/xai-grok-shell/src/remote/client.rs` | Set `provider` on remotely-fetched model entries (defaults to `"xai"`). |
| `crates/codegen/xai-grok-shell/src/session/acp_session.rs` | Added `sampler_event_tx` field to `SessionActor` — a clone of the sink the sampler actor writes to, so the Bedrock turn path can write into the same UI event stream. |
| `crates/codegen/xai-grok-shell/src/session/acp_session_impl/sampler_turn.rs` | Added the provider branch in `run_turn_via_sampler`; added `run_turn_via_bedrock` and `report_bedrock_failure` — the parallel Bedrock execution path. xAI's code path in this file is otherwise unchanged. |
| `crates/codegen/xai-grok-shell/src/session/acp_session_impl/spawn.rs` | Clone `sampler_event_tx` before handing it to `SamplerActor::spawn`, so the session retains its own clone. |
| `crates/codegen/xai-grok-shell/src/session/compaction.rs` | Added `sampler_event_tx` to the test-fixture `SessionActor` literal. |

## Bugs Found and Fixed

Full detail in [EVALUATION.md](EVALUATION.md).

1. **No tool execution on Bedrock turns** — `BedrockClient` never sends
   `toolConfig` or parses `ToolUse` blocks, so Bedrock models cannot run
   code they generate. Surfaced by a real generated bug (a fibonacci
   implementation with a false docstring claim and a broken helper
   function) that went unverified because there was no tool to verify it
   with. Documented as a known limitation with a user-facing warning;
   the actual toolConfig/ToolResultBlock wiring was not built in this pass.
2. **Model identity confusion** — every model reported itself as "Grok
   4.5" regardless of which model was active, caused by a hardcoded
   "released by xAI" string plus a missing `system_prompt_label` on every
   Bedrock catalog entry. Fixed.
3. **Truncated output on longer tasks** — every Bedrock catalog entry was
   missing `max_completion_tokens`, so Bedrock silently applied its own low
   default token cap. Fixed by setting `8192` on all 11 entries.
4. **Weak branding during active use** — "AtheCode" only appeared on the
   welcome screen, not during an active chat session. Fixed by adding a
   persistent status-bar item.

## License Compliance

AtheCode is a fork of Grok Build, licensed Apache-2.0, Copyright 2023-2026
SpaceXAI. This fork retains the original license and copyright notices as
required by the Apache-2.0 license. This project is not affiliated with
xAI or SpaceXAI.
