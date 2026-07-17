# AtheCode

AtheCode is a terminal-based AI coding assistant, forked from xAI's open-source
[Grok Build](https://github.com/xai-org/grok-build) (Apache-2.0) and extended
with a second inference backend — Amazon Bedrock — giving access to 11
Bedrock-hosted models (Qwen, DeepSeek, Kimi, Nova, GLM, MiniMax, Gemma,
GPT-OSS) alongside the original xAI model, selectable from a single terminal
UI via `/model`.

## What This Is

- A real fork of Grok Build — an 81-crate Rust workspace, Apache-2.0 licensed
  — with genuine engineering added on top, not a rebrand-only fork.
- A from-scratch Amazon Bedrock provider: a `SamplingBackend` trait,
  `BedrockClient`, and Converse/ConverseStream API integration. Upstream Grok
  Build has zero AWS integration; all of it was written for this fork.
- 11 Bedrock models plus the original xAI model, accessible through one
  terminal UI, switchable with `/model`.

## What I Built (not what Grok Build already had)

1. **`SamplingBackend` trait** — the provider abstraction that didn't exist in
   upstream Grok Build. `xai-grok-sampler`'s `SamplingClient` (xAI) and the
   new `xai-grok-bedrock`'s `BedrockClient` both implement it, so a caller can
   hold a `Box<dyn SamplingBackend>` without knowing which provider it is.
2. **`BedrockClient`** — full Converse API (non-streaming) and ConverseStream
   API (streaming) integration via `aws-sdk-bedrockruntime`, including AWS
   SigV4 signing (handled by the SDK), `ConversationRequest`→Converse message
   mapping, streaming event translation (`ContentBlockDelta`→`SamplingEvent`),
   and error handling that distinguishes credential, network, and
   model-access failures.
3. **Model catalog** — 11 Bedrock models wired into the existing catalog
   system with a `provider` field, correct per-model `max_completion_tokens`
   (8192) and `temperature`, and per-model `system_prompt_label` so each
   model states its real identity.
4. **Turn routing** — `run_turn_via_bedrock`, a parallel execution path that
   bypasses the xAI `SamplerActor` for Bedrock turns and forwards
   `SamplingEvent`s into the same event sink the actor uses, so both
   providers stream into the chat UI identically.
5. **Identity correction** — removed the system prompt's hardcoded "released
   by xAI" and the "Grok" default identity label, so each model reports its
   actual identity (or "AtheCode" as a neutral default) instead of claiming
   to be Grok regardless of which model is active.

## Known Limitations (honest, not hidden)

- **Bedrock models do not support tool calling.** `toolConfig` is never sent
  on Bedrock requests, and `ToolResultBlock` is never parsed from responses.
  Bedrock turns are single-shot text only — no bash, file read/write, or
  code execution. Generated code is not automatically tested before being
  shown to the user. See [ARCHITECTURE.md](ARCHITECTURE.md#6-known-gap-tool-calling)
  for what this caused and the planned fix.
- **No context compaction on the Bedrock path.** Long Bedrock conversations
  can exceed context without the trimming xAI turns get.
- **Text-only.** Tool calls, tool results, images, and reasoning blocks are
  stripped when converting a request to Bedrock's wire format; only system
  and user/assistant text round-trips.

## Available Models

| Model ID | Display Name | Provider |
|---|---|---|
| `grok-build` | Grok Build | xai |
| `moonshotai.kimi-k2.5` | Kimi K2.5 | bedrock |
| `moonshot.kimi-k2-thinking` | Kimi K2.5 Thinking | bedrock |
| `deepseek.v3.2` | DeepSeek V3.2 | bedrock |
| `google.gemma-3-12b-it` | Gemma 3 12B | bedrock |
| `amazon.nova-pro-v1:0` | Nova Pro | bedrock |
| `minimax.minimax-m2.5` | MiniMax M2.5 | bedrock |
| `openai.gpt-oss-safeguard-120b` | GPT-OSS Safeguard 120B | bedrock |
| `qwen.qwen3-coder-next` | Qwen3 Coder Next | bedrock |
| `qwen.qwen3-next-80b-a3b` | Qwen3 Next 80B | bedrock |
| `zai.glm-5` | GLM-5 | bedrock |
| `zai.glm-4.7` | GLM-4.7 | bedrock |

## Architecture

See [ARCHITECTURE.md](ARCHITECTURE.md) for the full design. In short: a
provider-agnostic `SamplingBackend` trait lives in `xai-grok-sampling-types`
(the shared, dependency-light crate). `xai-grok-sampler` implements it for
xAI; the new `xai-grok-bedrock` crate implements it for Bedrock. Both speak
the same `ConversationRequest`/`ConversationResponse`/`SamplingEvent` types,
so the chat UI never needs to know which backend produced a response.

## Setup — Local Installation

### Prerequisites

- **Rust** — pinned by `rust-toolchain.toml`; `rustup` (rustup.rs) installs it
  automatically on first build.
- **protoc** — either install [dotslash](https://dotslash-cli.com) (resolves
  the vendored `bin/protoc` launcher) or have a `protoc` binary on `PATH`
  (e.g. `brew install protobuf` on macOS).
- An AWS account with Bedrock access, if you want to use the Bedrock models.

### 1. Clone and build

```bash
git clone https://github.com/Athelesh-7G/AtheCode.git
cd AtheCode
cargo build -p xai-grok-pager-bin --release
```

### 2. Set up AWS credentials (for Bedrock models)

Create an IAM user/role with Bedrock invoke permissions, generate an access
key, and enable the models you want to use in the Bedrock console under
**Model access**. Not needed if you only use the xAI model.

### 3. Run AtheCode

```bash
export AWS_ACCESS_KEY_ID="your_access_key_id"
export AWS_SECRET_ACCESS_KEY="your_secret_access_key"
export AWS_REGION="us-east-1"
cargo run -p xai-grok-pager-bin
```

Or after a release build:

```bash
./target/release/xai-grok-pager
```

### 4. Select a model

Inside AtheCode, type `/model` and choose from the 11 Bedrock models or the
original xAI model.

## Editor Integration

Grok Build (and therefore this fork) ships an [Agent Client
Protocol](https://agentclientprotocol.com) stdio server, inherited from
upstream — this is existing Grok Build functionality, not something built
for this fork:

```bash
xai-grok-pager agent stdio
```

This is what editors like Zed use to embed the agent. Whether Bedrock model
selection works correctly through this path has not been separately
verified in this fork — testing so far has been through the interactive TUI.

## Credits

Forked from [xAI's Grok Build](https://github.com/xai-org/grok-build)
(Apache-2.0), Copyright 2023-2026 SpaceXAI. Bedrock integration, rebrand, and
bug fixes by Athelesh Balachandran. Not affiliated with xAI or SpaceXAI.
