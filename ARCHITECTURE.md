# AtheCode Architecture

This document describes the Bedrock integration added on top of upstream
Grok Build: the provider abstraction, the Bedrock client, streaming, turn
routing, and the known tool-calling gap. It assumes familiarity with a
Rust workspace but not with Grok Build's internals.

## 1. System Overview

Before this fork, Grok Build's inference layer (`xai-grok-sampler`) was a
single concrete client (`SamplingClient`) hard-wired to xAI's three API
shapes (Chat Completions, Responses, Anthropic Messages — all xAI-hosted).
There was no trait, no second implementation, and no concept of "provider"
in the codebase at all.

This fork introduces a `SamplingBackend` trait as the seam between provider
implementations:

```
                    ConversationRequest
                            │
                            ▼
              ┌─────────────────────────┐
              │   SamplingBackend trait │   (xai-grok-sampling-types)
              └─────────────────────────┘
                     │              │
                     ▼              ▼
          ┌────────────────┐  ┌──────────────────┐
          │ SamplingClient │  │  BedrockClient    │
          │ (xai-grok-     │  │  (xai-grok-       │
          │  sampler)      │  │   bedrock)        │
          │                │  │                   │
          │ xAI Chat/      │  │ AWS Converse /    │
          │ Responses/     │  │ ConverseStream     │
          │ Messages APIs, │  │ (aws-sdk-          │
          │ SSE            │  │  bedrockruntime)   │
          └────────────────┘  └──────────────────┘
                     │              │
                     ▼              ▼
              stream of SamplingEvent
                            │
                            ▼
                  same UI event sink
```

Both implementations consume the same `ConversationRequest` and produce the
same `ConversationResponse` / `SamplingEvent` stream. The chat UI and the
turn loop never branch on which backend is active except at one dispatch
point (see §5).

## 2. The `SamplingBackend` Trait

Defined in `crates/codegen/xai-grok-sampling-types/src/backend.rs`:

```rust
#[async_trait]
pub trait SamplingBackend: Send + Sync {
    async fn chat_completion(
        &self,
        request: ConversationRequest,
    ) -> Result<ConversationResponse, SamplingError>;

    async fn chat_completion_stream(
        &self,
        request: ConversationRequest,
    ) -> Result<BoxStream<'static, SamplingEvent>, SamplingError>;
}
```

Two design decisions:

- **It lives in `xai-grok-sampling-types`, not `xai-grok-sampler`.** That
  crate is the shared, dependency-light types crate both `xai-grok-sampler`
  (xAI) and `xai-grok-bedrock` depend on. Putting the trait in
  `xai-grok-sampler` — which is xAI-specific by name and by its own crate
  description — would have created a dependency from Bedrock code back into
  xAI code for no reason.
- **It uses `async_trait`, not native async-fn-in-traits.** The trait needs
  to be `dyn`-compatible — callers hold a `Box<dyn SamplingBackend>` chosen
  at runtime based on which model a user selected — and native async fn in
  traits is not object-safe without manual boxing. `async-trait` was already
  a workspace dependency, so this added no new dependency.

Moving the trait's associated types (`SamplingEvent`, `RequestId`,
`InferenceLatencyStats`) out of `xai-grok-sampler` into
`xai-grok-sampling-types` was a prerequisite: `SamplingEvent` referenced
`RequestId` and `InferenceLatencyStats`, both of which previously lived in
the xAI-specific sampler crate. All three moved to
`xai-grok-sampling-types/src/{events,request_id,metrics}.rs`, with
re-export shims left in `xai-grok-sampler` at the old paths so no existing
xAI-side import site needed to change.

## 3. Bedrock Client Design

`crates/codegen/xai-grok-bedrock/src/client.rs` implements `BedrockClient`.

**Why a sibling crate, not a module inside `xai-grok-sampler`:** the AWS SDK
(`aws-sdk-bedrockruntime`, `aws-config`) is a heavy dependency tree that
nobody who only uses xAI models should have to compile. `xai-grok-sampler`'s
own Cargo.toml describes it as the xAI-specific layer — bolting Bedrock in
as a module would contradict that. `xai-grok-bedrock` depends only on
`xai-grok-sampling-types`, keeping the dependency graph a clean hub-and-spoke
(both provider crates depend on the shared types crate; neither depends on
the other).

**Request conversion** (`build_converse_input`): `ConversationRequest.items`
is walked once —

- `ConversationItem::System` → `SystemContentBlock::Text`
- `ConversationItem::User` → a `Message` with role `User`, content built
  from `content_parts_to_blocks` (text passes through; images are dropped —
  see §6)
- `ConversationItem::Assistant` → a `Message` with role `Assistant`
- everything else (tool calls, tool results, backend tool calls, reasoning)
  is skipped — see §6

Messages that would end up with zero content blocks are dropped rather than
sent empty, since Bedrock rejects empty-content messages.
`InferenceConfiguration` (`max_tokens`, `temperature`, `top_p`) is built
from the request's own fields, which are populated from the model catalog
entry (§7).

**Response conversion**: `chat_completion` (non-streaming) reads
`response.output().as_message()`, concatenates `ContentBlock::Text` chunks,
maps Bedrock's `StopReason` (`EndTurn`/`StopSequence`→`Stop`,
`MaxTokens`→`Length`, `ToolUse`→`ToolCalls`,
`ContentFiltered`/`GuardrailIntervened`→`ContentFilter`) and `TokenUsage`
back into the provider-agnostic `ConversationResponse`.

## 4. Streaming Architecture

Bedrock's `ConverseStream` API returns AWS's binary event-stream framing
(`application/vnd.amazon.eventstream`), handled entirely by
`aws-sdk-bedrockruntime` — no manual framing code was written. The
`chat_completion_stream` implementation drains `response.stream.recv()` in
an `async_stream::stream!` block and maps events:

| Bedrock event | AtheCode `SamplingEvent` |
|---|---|
| first `ContentBlockDelta::Text` | `FirstToken`, then `ChannelToken` |
| subsequent `ContentBlockDelta::Text` | `ChannelToken` |
| `MessageStop` | accumulated into the terminal `Completed` event's `stop_reason` |
| `Metadata` (usage) | accumulated into `Completed`'s `usage` |
| stream error | `Failed` |

`SamplingEvent` is the same enum the xAI SSE path (`xai-grok-sampler`'s
`stream/*.rs` transforms) produces. This is what makes the UI event sink
provider-agnostic: `handle_sampling_event` (the drainer in
`xai-grok-shell/src/session/acp_session_impl/tool_calls.rs`) renders
`ChannelToken`/`Completed`/`Failed` identically regardless of which backend
produced them, and correlates by `current_prompt_id`, not by the event's
`request_id` — so it doesn't matter that `BedrockClient` generates its own
`RequestId` independently of the xAI actor's request tracking.

## 5. Turn Routing

The interactive turn loop (`xai-grok-shell/src/session/acp_session_impl/
turn.rs`) calls `run_turn_via_sampler` once per model response. Before this
fork, that function always drove `SamplerHandle::submit_and_collect`, which
hands the request to the long-lived `SamplerActor` — an actor with a retry
loop, doom-loop recovery, and HTTP/1.1 fallback logic, all tightly coupled
to `SamplerConfig` (a struct passed by value into `SamplingClient::new` at
every retry).

**The alternative considered and rejected**: swap `SamplerConfig` for the
`SamplingBackendConfig` enum everywhere a session holds sampling config, so
the actor itself could drive either backend. This was rejected because
`SamplerConfig` is the pervasive currency of the whole session — stored in a
`RefCell`, cloned for subagents, reconstructed every turn
(`reconstruct_full_config`), and pushed into the actor via `update_config`.
A `Box<dyn SamplingBackend>` is not `Clone` and does not fit that shape
without rewriting the actor's retry/rebuild logic to be backend-generic —
a much larger, riskier change than this pass justified.

**What was built instead**: `run_turn_via_sampler` branches once, at the
top, on `models_manager.model_provider(current_model_id)`. xAI turns fall
through unchanged — the actor path below the branch is byte-for-byte what
it was before this fork. Bedrock turns call the new `run_turn_via_bedrock`,
which:

1. Resolves a `Box<dyn SamplingBackend>` via `resolve_sampling_backend_config`
   (validates `AWS_ACCESS_KEY_ID`/`AWS_SECRET_ACCESS_KEY`/`AWS_REGION`,
   builds a `BedrockClient`).
2. Arms the same `turn_stream_drained` oneshot barrier the xAI path uses.
3. Calls `chat_completion_stream` and forwards every `SamplingEvent` into
   `self.sampler_event_tx` — a clone of the same
   `mpsc::UnboundedSender<SamplingEvent>` the `SamplerActor` writes to —
   so `handle_sampling_event` renders it exactly like an xAI response.
4. Captures the final `ConversationResponse`/metrics from the terminal
   `Completed` event and returns `SamplerTurnOutcome::Response`, letting the
   outer turn loop finish identically to an xAI turn.

**Cancellation**: turns are cancelled by aborting the turn's tokio task
(`tasks_cancel.rs`'s `handle.abort()`), which drops the `run_turn_via_bedrock`
future — including the `stream.next()` loop — tearing the Bedrock connection
down cleanly. No explicit cancellation token was needed.

**Explicit known limitations of this path**: no auth-refresh (AWS
credentials are static per session, unlike xAI's rotating session tokens)
and no compaction (context trimming) — a Bedrock turn that fails is
surfaced as a terminal error rather than retried through xAI's recovery
machinery.

## 6. Known Gap: Tool Calling

Bedrock's Converse API natively supports tool calling via a `toolConfig`
field on the request and `ContentBlock::ToolUse` blocks in the response —
this is not a missing AWS feature, it's simply not wired up yet in
`BedrockClient`.

`build_converse_input` walks `ConversationRequest.items` and explicitly
skips `ConversationItem::ToolCall`/`ToolResult` (and backend tool calls,
and reasoning items) with a `_ => {}` match arm. `ConversationRequest.tools`
— the actual tool list the turn loop builds via `build_request(effective_tools,
...)`, which includes bash, file read/write, grep, etc. — is never read by
`BedrockClient` at all. `chat_completion_stream` correspondingly only parses
`ContentBlockDelta::Text`; it has no handling for `ContentBlock::ToolUse`.

**What this caused, concretely**: see
[EVALUATION.md](EVALUATION.md#2-real-bug-found-and-what-it-revealed) — a
Bedrock model (Qwen3 Coder Next) was asked for a fibonacci implementation
and produced a self-contradicting docstring plus a broken matrix-multiplication
helper. The bug's significance isn't the wrong code — it's that a Bedrock
turn has no way to run `cargo test` or `python3 -c "..."` on its own output
before showing it to the user, because it was never given the tool to do
so.

**Planned fix** (not built): thread `ConversationRequest.tools` through
`ToolConfiguration` on the Converse/ConverseStream request; parse
`ContentBlock::ToolUse` from both the non-streaming response and the
streaming `ContentBlockStart`/`ContentBlockDelta` events into
`SamplingEvent::ToolCallDelta`; and map `ConversationItem::ToolResult` back
into a Bedrock `ToolResultBlock` on the next turn's request so the ReAct
loop in `turn.rs` (`execute_tool_calls` → `continue` until `tool_calls.
is_empty()`) works unchanged for Bedrock turns.

## 7. Model Catalog Design

`crates/codegen/xai-grok-models/default_models.json` gained a `provider`
field on every entry (defaulting to `"xai"` for backward compatibility) and
11 new entries with `"provider": "bedrock"`. Each Bedrock entry sets:

- `max_completion_tokens: 8192` and `temperature: 0.7` — both were
  previously entirely absent from every Bedrock entry, which meant
  `InferenceConfiguration.max_tokens` was never set on the Converse request
  and Bedrock silently applied a low provider default (see
  [EVALUATION.md](EVALUATION.md#4-token-limit-bug)).
- `system_prompt_label` set to the model's own display name (e.g. "Qwen3
  Coder Next"), so the system prompt's `You are ${{ system_prompt_label }}`
  line resolves to the real model identity instead of the generic default.

`ApiBackend` (`chat_completions`/`responses`/`messages`) was deliberately
**not** reused as a provider selector for Bedrock — it's a wire-shape enum
consumed deep inside `SamplingClient` for xAI-hosted endpoint routing, and
repurposing it for a different vendor entirely would have been a category
error. `provider` is a new, separate field.

`resolve_model_list` (in `xai-grok-shell/src/agent/config.rs`) merges a
fetched/prefetched xAI catalog over the built-in defaults, which would
normally drop the bundled Bedrock entries entirely once a real xAI catalog
arrives. `merge_bedrock_defaults` re-inserts any `provider == "bedrock"`
default entry whose key isn't already present, so the 11 Bedrock models
stay selectable regardless of xAI catalog source.
