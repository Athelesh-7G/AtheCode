# Evaluation

This document records what was actually tested during the Bedrock
integration, the behavioral issues uncovered through live validation, the
fixes applied, and the areas that remain unverified. It serves as an
engineering evaluation record rather than a changelog, documenting both the
validated capabilities of this fork and the limitations that still require
future work.

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

**Live TUI testing:** performed manually by the project owner using real AWS
credentials against the full interactive chat path. This validated model
selection, provider routing, response streaming, multi-turn conversations,
and overall runtime behavior. It also exposed the behavioral issues
documented below—none of which could have been detected through compilation
or the standalone smoke test alone.

## 2. Tool-Calling Validation Gap

**What happened:** with Qwen3 Coder Next selected as the active model, the
user asked for a fibonacci implementation. The model returned code that
included a matrix-multiplication-based fibonacci function
(`fibonacci_matrix`) with a broken matrix-multiplication helper, alongside a
docstring asserting `fibonacci_matrix(100) == 354224848179261915075` — which
is false; that is not the 100th Fibonacci number, and the buggy
multiplication function couldn't have produced it correctly regardless.

**Why this mattered:** the incorrect code itself was not the primary issue.
Any LLM can generate buggy code. The important finding was that the Bedrock
execution path had no mechanism to verify its own output before presenting
it to the user.

Root-cause analysis (see
[ARCHITECTURE.md §6](ARCHITECTURE.md#6-known-gap-tool-calling)) confirmed
that `BedrockClient` never sends `toolConfig` on Converse requests and never
parses `ContentBlock::ToolUse` responses. Consequently, every Bedrock turn
is currently limited to single-shot text generation with no ability to
execute generated code or validate its own claims.

By contrast, the original xAI execution path runs a complete ReAct
tool-calling loop (`turn.rs`: `execute_tool_calls` → `continue` until
`tool_calls.is_empty()`), allowing models to invoke tools such as bash
before producing a final answer. The same class of error would likely have
been detected automatically through tool execution.

This limitation was documented rather than hidden. A one-time notification
now informs users when selecting a Bedrock model that generated code is not
automatically verified until tool calling is implemented.

## 3. Model Identity Bug

**What happened:** asking any active model "who are you" returned "Grok
4.5" — regardless of whether a Bedrock model was actually selected.

**Root cause:** `crates/codegen/xai-grok-agent/templates/prompt.md` line 1
hardcoded `You are ${{ system_prompt_label }} released by xAI.` — the
"released by xAI" clause was unconditional text, not templated. Separately,
`DEFAULT_SYSTEM_PROMPT_LABEL` in `xai-grok-agent/src/prompt/context.rs`
defaulted to `"Grok"`, and — critically — every one of the 11 Bedrock
catalog entries omitted `system_prompt_label`, causing every Bedrock model
to resolve to the same default identity.

Combined, every Bedrock model's system prompt effectively became:

> You are Grok released by xAI.

One caveat noted during diagnosis—but outside the control of this
integration—is that some open-weight models may still self-identify based on
their training data. This fix removes the harness-side cause without making
claims about model-specific training behavior.

**Fix:** removed the hardcoded "released by xAI" attribution, changed the
default label to `"AtheCode"`, and assigned every Bedrock model its own
`system_prompt_label` so identity now resolves correctly through the normal
configuration hierarchy.

## 4. Token Limit Bug

**What happened:** asking a model to analyze multiple files in a directory
produced output that appeared to stop mid-analysis.

**Root cause:** `BedrockClient::build_converse_input()` only sets
`InferenceConfiguration.max_tokens` when
`request.max_output_tokens.is_some()`. That value comes directly from each
model's catalog entry (`max_completion_tokens`), and all 11 Bedrock entries
initially omitted it. Bedrock therefore silently applied provider defaults,
resulting in truncated responses for longer generations.

This was independently confirmed not to be a streaming issue—the streaming
implementation correctly accumulated every `ContentBlockDelta::Text` until
completion.

**Fix:** added `"max_completion_tokens": 8192` and `"temperature": 0.7` to
every Bedrock model entry in the default catalog.

## 5. Verified Functionality

Confirmed through live manual testing using real AWS credentials:

- Model selection via `/model`, including switching between Bedrock models
  and the original xAI model.
- Successful routing of inference requests to the selected provider.
- Text streaming through the shared `SamplingEvent` pipeline.
- Response rendering through the existing terminal UI.
- Multi-turn conversations under normal usage.
- AWS credential validation with clear runtime errors when
  `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, or `AWS_REGION` are missing.

## 6. What's Not Yet Verified

The following capabilities have not yet been exercised or validated beyond
the testing described above:

- **Tool execution** — not implemented (see §2). Bedrock requests currently
  do not send `ToolConfiguration` or process `ToolUse` / `ToolResult`
  blocks, so tool execution cannot yet be validated.

- **Context compaction under long-running Bedrock sessions** — normal
  multi-turn conversations were exercised during live testing, but behavior
  once a Bedrock conversation grows large enough to require context
  compaction has not yet been validated.

- **Stress and resilience testing** — the Bedrock integration has not yet
  been evaluated under prolonged workloads, concurrent sessions, AWS
  throttling, or network interruption scenarios.

- **ACP / editor integration** — the inherited `agent stdio` pathway has
  not yet been validated with Bedrock models. Live testing for this project
  has focused on the interactive terminal UI.
