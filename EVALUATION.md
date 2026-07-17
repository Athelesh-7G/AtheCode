# Evaluation

This document records what was actually tested during the Bedrock
integration, what bugs live testing surfaced, and what remains unverified.
It is written as an honest evaluation record, not a changelog — the most
useful finding here is a real correctness gap in the assistant, not a list
of things that worked.

## 1. What Was Tested

**Compile-time verification (every change in this fork):** `cargo check
--workspace` and `cargo build --workspace` were run after every change set
and confirmed clean before commit. This verifies the code compiles and
type-checks; it does not verify runtime behavior against a real Bedrock
endpoint, since the development environment used to write this code had no
AWS credentials configured.

**`bedrock_smoke_test` binary** (`crates/codegen/xai-grok-bedrock/src/bin/
bedrock_smoke_test.rs`): a standalone binary built to prove the wiring —
`ATHECODE_PROVIDER=bedrock` gate, AWS credential resolution via
`aws_config::from_env()`, a single non-streaming `converse()` call, printed
response. This compiled cleanly but was not run against live AWS
credentials during development; it exists for the operator to run manually
(`cargo run -p xai-grok-bedrock --bin bedrock_smoke_test`) as the first
checkpoint before trusting the full TUI integration.

**Live TUI testing**: performed manually by the project owner, with real
AWS credentials, against the full interactive chat path. This is where the
four bugs below were found — none of them were caught by compilation or the
smoke test, because none of them are type errors; they're behavioral gaps
that only show up when a real model actually responds.

## 2. Real Bug Found and What It Revealed

**What happened**: with Qwen3 Coder Next selected as the active model, the
user asked for a fibonacci implementation. The model returned code that
included a matrix-multiplication-based fibonacci function
(`fibonacci_matrix`) with a broken matrix-multiplication helper, alongside a
docstring asserting `fibonacci_matrix(100) == 354224848179261915075` — which
is false; that is not the 100th Fibonacci number, and the buggy
multiplication function couldn't have produced it correctly regardless.

**Why this is the most valuable finding, not an embarrassment**: the model
being wrong isn't surprising — any model can generate buggy code. What this
proved is a structural gap in the harness: **the Bedrock turn path has no
way to execute or verify the code it generates before showing it to the
user.** Root-cause diagnosis (see
[ARCHITECTURE.md §6](ARCHITECTURE.md#6-known-gap-tool-calling)) confirmed
`BedrockClient` never sends `toolConfig` on the Converse request and never
parses `ContentBlock::ToolUse` from the response — so a Bedrock turn is
always single-shot text, with no bash tool to run `cargo test` or a Python
snippet against its own claims. The xAI path, by contrast, runs a full
ReAct tool-calling loop (`turn.rs`: `execute_tool_calls` → `continue` until
`tool_calls.is_empty()`) — the same class of bug would very likely have
been caught before being shown to the user, because the model could have
run the code itself.

This was documented as a known limitation (not fixed in this pass — it's a
genuinely large feature: `ToolConfiguration` on requests, `ToolResultBlock`
parsing on streaming and non-streaming responses, and threading both
through the existing ReAct loop) and surfaced to the user directly: a
one-time toast on first Bedrock model selection now states that generated
code is not automatically tested.

## 3. Model Identity Bug

**What happened**: asking any active model "who are you" returned "Grok
4.5" — regardless of whether a Bedrock model was actually selected.

**Root cause**: `crates/codegen/xai-grok-agent/templates/prompt.md` line 1
hardcoded `You are ${{ system_prompt_label }} released by xAI.` — the
"released by xAI" clause was unconditional text, not templated. Separately,
`DEFAULT_SYSTEM_PROMPT_LABEL` in `xai-grok-agent/src/prompt/context.rs`
defaulted to `"Grok"`, and — critically — **every one of the 11 Bedrock
catalog entries had no `system_prompt_label` set**, so every Bedrock model
resolved to that same "Grok" default. Combined, the system prompt for a
Bedrock model literally read "You are Grok released by xAI" regardless of
which of the 11 models was active.

One caveat noted during diagnosis but not something this fix controls: some
open-weight models are trained on data that includes strong self-identity
priors (e.g. having been fine-tuned on Grok-branded conversations
elsewhere); a system prompt correction cannot force a model to stop
self-identifying against its own training if that happens. This fix removes
the harness-side cause; it does not guarantee every model's own trained
identity is neutral.

**Fix**: removed "released by xAI" entirely (not replaced with any other
attribution — the actual origin varies per model and a blanket claim would
be inaccurate either way); changed the default label to `"AtheCode"`; and
set each Bedrock catalog entry's `system_prompt_label` to its real display
name (e.g. `"Qwen3 Coder Next"`), so the resolution tier chain
(env var → user-per-model → user-global → **catalog per-model** → remote →
default) now resolves each Bedrock model to its own name.

## 4. Token Limit Bug

**What happened**: asking a model to analyze multiple files in a directory
produced output that appeared to cut off mid-analysis.

**Root cause**: `BedrockClient::build_converse_input` only sets
`InferenceConfiguration.max_tokens` `if request.max_output_tokens.is_some()`.
That value is sourced from the model's catalog entry
(`max_completion_tokens`) — and every one of the 11 Bedrock entries in
`default_models.json` had that field entirely absent. With no value ever
set, Bedrock silently applied each model's own low default cap, truncating
any response — like a multi-file analysis — that needed more than that
default. This was not a streaming bug: the chunk-accumulation logic in
`chat_completion_stream` was independently verified correct (it appends
every `ContentBlockDelta::Text` across the whole stream with no early
break).

**Fix**: added `"max_completion_tokens": 8192` and `"temperature": 0.7` to
all 11 Bedrock catalog entries (temperature was also confirmed absent).

## 5. Verified Working

Confirmed via live manual testing by the project owner:

- Model selection via `/model`, including switching between Bedrock models
  and back to the xAI model.
- Text streaming from a Bedrock model into the chat UI, rendering through
  the same event path as xAI responses.
- Response rendering (the model's actual reply text reaching the terminal).
- AWS credential validation surfacing a clear error when
  `AWS_ACCESS_KEY_ID`/`AWS_SECRET_ACCESS_KEY`/`AWS_REGION` are missing,
  rather than a panic.

## 6. What's Not Yet Verified

- **Tool execution** — not implemented (§2); not testable until built.
- **Context compaction on the Bedrock path** — `run_turn_via_bedrock`
  explicitly does not perform compaction; behavior once a long Bedrock
  conversation exceeds context has not been exercised live.
- **Multi-turn conversation state under Bedrock** — whether conversation
  history round-trips correctly across several turns (system/user/assistant
  item ordering, especially once a turn included content types Bedrock
  drops, like an image) has not been separately confirmed beyond the single
  reported testing session.
- **ACP/editor integration with Bedrock models** — the `agent stdio` path
  (inherited from upstream Grok Build) has not been tested with a Bedrock
  model selected; all live testing so far went through the interactive TUI.
