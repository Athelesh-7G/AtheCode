<div align="center">

# AtheCode

**A terminal coding agent powered by 11 open and closed-source models on Amazon Bedrock.**

[![Rust](https://img.shields.io/badge/Rust-1.92%2B-orange?style=for-the-badge&logo=rust)](https://www.rust-lang.org)
[![Amazon Bedrock](https://img.shields.io/badge/Amazon%20Bedrock-11%20Models-232F3E?style=for-the-badge&logo=amazonaws)](#available-models)
[![License](https://img.shields.io/badge/License-Apache%202.0-blue?style=for-the-badge)](./LICENSE)

**Built by Athelesh Balachandran**

</div>

---

## What Is This

AtheCode is a fork of xAI's open-source [Grok Build](https://github.com/xai-org/grok-build), rebuilt with a custom Amazon Bedrock provider so it runs entirely on AWS credentials instead of xAI's API. One terminal agent, 11 models across 8 providers — Qwen, DeepSeek, Kimi, Nova, GLM, MiniMax, Gemma, GPT-OSS — selectable mid-session with `/model`, all streaming through a single unified interface.

Where upstream Grok Build talks to exactly one provider, AtheCode introduces a provider-abstraction layer that treats Bedrock as a first-class backend — same streaming UI, same session model, same terminal experience, entirely new inference layer underneath.

---

## Architecture

```
┌──────────────────────────────────────────────────────────────────────────┐
│                          ATHECODE TERMINAL UI                            │
│                    (xai-grok-pager — Rust TUI, 62+ crates)               │
└────────────────────────────────┬───────────────────────────────────────┘
                                  │
                                  ▼
                    ┌─────────────────────────────┐
                    │     SamplingBackend trait     │
                    │  (provider-agnostic interface) │
                    └──────────────┬──────────────┘
                                  │
                 ┌────────────────┴────────────────┐
                 ▼                                  ▼
      ┌─────────────────────┐          ┌─────────────────────────┐
      │   xAI SamplingClient  │          │      BedrockClient        │
      │   (upstream, xAI API) │          │   (new — this fork)       │
      └─────────────────────┘          └────────────┬────────────┘
                                                       │
                                                       ▼
                                        ┌───────────────────────────┐
                                        │  Amazon Bedrock Runtime    │
                                        │  Converse + ConverseStream │
                                        │  SigV4 · 11 models         │
                                        └───────────────────────────┘
```

---

## Quick Start

**Prerequisites:** Rust 1.92+, `protobuf` + `dotslash` (`brew install protobuf dotslash`), an AWS account with Bedrock model access.

```bash
git clone https://github.com/Athelesh-7G/AtheCode.git
cd AtheCode
cargo build --release
```

Create an IAM user with `AmazonBedrockFullAccess`, generate an access key under the **Local code** use case, then:

```bash
export AWS_ACCESS_KEY_ID="your_access_key_id"
export AWS_SECRET_ACCESS_KEY="your_secret_access_key"
export AWS_REGION="us-east-1"
```

Run it:

```bash
cargo run -p xai-grok-pager-bin
```

Inside AtheCode, type `/model` and pick any of the 11 Bedrock models.

**Editor integration (ACP):**

```bash
xai-grok-pager agent stdio
```

---

## Available Models

| Model | Provider | Model ID |
|---|---|---|
| Qwen3 Coder Next | Qwen | `qwen.qwen3-coder-next` |
| Qwen3 Next 80B | Qwen | `qwen.qwen3-next-80b-a3b` |
| DeepSeek V3.2 | DeepSeek | `deepseek.v3.2` |
| Kimi K2.5 | Moonshot AI | `moonshotai.kimi-k2.5` |
| Kimi K2.5 Thinking | Moonshot AI | `moonshot.kimi-k2-thinking` |
| GLM-5 | Zhipu AI | `zai.glm-5` |
| GLM-4.7 | Zhipu AI | `zai.glm-4.7` |
| Nova Pro | Amazon | `amazon.nova-pro-v1:0` |
| MiniMax M2.5 | MiniMax | `minimax.minimax-m2.5` |
| Gemma 3 12B | Google | `google.gemma-3-12b-it` |
| GPT-OSS Safeguard 120B | OpenAI | `openai.gpt-oss-safeguard-120b` |

---

## What's New in This Fork

Grok Build ships with a single hard-wired xAI client. Everything below is new engineering on top of the upstream codebase:

- **`SamplingBackend` trait** — a provider-agnostic interface (`xai-grok-sampling-types`) that both the xAI client and the new Bedrock client implement, so the rest of the codebase doesn't know or care which one is answering.
- **`BedrockClient`** — a new crate (`xai-grok-bedrock`) built on `aws-sdk-bedrockruntime`: Converse and ConverseStream API integration, SigV4 request signing, and Bedrock's binary event-stream framing mapped directly into the same streaming event types the xAI path already used.
- **Provider-aware model catalog** — 11 Bedrock models merged into the existing catalog system with correct per-model token limits and system-prompt identity, selectable through the same `/model` picker as the original xAI model.
- **Parallel turn routing** — Bedrock turns run through a dedicated execution path that writes into the same event sink as the xAI actor, so both providers stream identically in the chat UI with zero changes to existing xAI turn logic.

Full technical breakdown in [`ARCHITECTURE.md`](./ARCHITECTURE.md).

---

## Tech Stack

| Layer | Technology |
|---|---|
| Language | Rust 1.92 |
| TUI Framework | ratatui, custom pager (`xai-grok-pager`) |
| Inference | Amazon Bedrock Runtime (Converse API) |
| AWS SDK | `aws-sdk-bedrockruntime`, `aws-config` |
| Streaming | AWS event-stream framing → internal `SamplingEvent` |
| Build System | Cargo workspace, 62+ crates |
| Protocol | Agent Client Protocol (ACP) for editor integration |

---

## Docs

- [`ARCHITECTURE.md`](./ARCHITECTURE.md) — the `SamplingBackend` trait, Bedrock client design, streaming architecture, turn routing
- [`EVALUATION.md`](./EVALUATION.md) — testing methodology and results
- [`CONTRIBUTIONS.md`](./CONTRIBUTIONS.md) — full diff against upstream Grok Build

---

## Contributing

This fork is maintained independently and does not accept external pull requests. It's published for source transparency under the Apache License 2.0 — fork it, run it, adapt it for your own use.

The upstream project ([xai-org/grok-build](https://github.com/xai-org/grok-build)) has its own separate contribution policy; see that repository directly.

---

## License

Apache-2.0, inherited from [xai-org/grok-build](https://github.com/xai-org/grok-build). Not affiliated with xAI.

Fork, Bedrock integration, and engineering by [Athelesh Balachandran](https://github.com/Athelesh-7G).

</div>
