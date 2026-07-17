# AtheCode

A terminal coding agent, rewired to run on Amazon Bedrock.

[![Rust](https://img.shields.io/badge/Rust-1.92%2B-orange?style=flat-square&logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/License-Apache_2.0-blue?style=flat-square)](./LICENSE)
[![Models](https://img.shields.io/badge/Bedrock_Models-11-232F3E?style=flat-square&logo=amazonaws)](#available-models)

---

AtheCode is a fork of xAI's open-source [Grok Build](https://github.com/xai-org/grok-build), re-architected with a custom Amazon Bedrock provider so it can run entirely on AWS credentials instead of xAI's API. One terminal agent, 11 models — Qwen, DeepSeek, Kimi, Nova, GLM, MiniMax, Gemma, GPT-OSS — selectable mid-session with `/model`.

Built for anyone who wants a Grok-Build-grade terminal coding agent without an xAI account, using AWS Bedrock access they already have.

---

## Quick Start

**Prerequisites:** Rust 1.92+, `protobuf` + `dotslash` (`brew install protobuf dotslash`), an AWS account with Bedrock model access.

```bash
git clone https://github.com/Athelesh-7G/AtheCode.git
cd AtheCode
cargo build --release
```

Set your AWS credentials (create an IAM user with `AmazonBedrockFullAccess`, generate an access key under **Local code** use case):

```bash
export AWS_ACCESS_KEY_ID="your_access_key_id"
export AWS_SECRET_ACCESS_KEY="your_secret_access_key"
export AWS_REGION="us-east-1"
```

Run it:

```bash
cargo run -p xai-grok-pager-bin
```

Inside AtheCode, type `/model` and pick any of the 11 Bedrock models. Start chatting.

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

## What's Actually New Here

Grok Build has zero AWS integration natively. Everything below was built on top of the upstream fork, not inherited from it:

- **`SamplingBackend` trait** — the provider abstraction Grok Build didn't have. Both the original xAI client and the new Bedrock client implement the same interface, so the rest of the codebase doesn't know or care which one is answering.
- **`BedrockClient`** — a new crate (`xai-grok-bedrock`) wrapping `aws-sdk-bedrockruntime`: Converse API calls, streaming via `ConverseStream`, SigV4 auth, and Bedrock's binary event-stream framing mapped into the same event types the xAI SSE path already used.
- **Provider-aware model catalog** — 11 Bedrock models merged into the existing catalog with per-model token limits and correct system-prompt identity, so each model reports what it actually is instead of claiming to be Grok.
- **Parallel turn routing** — Bedrock turns run through a dedicated path (`run_turn_via_bedrock`) that writes into the same event sink the xAI actor uses, so both providers render identically in the chat UI without touching the existing xAI turn logic at all.
- **Identity fix** — the upstream system prompt hardcoded `"released by xAI"` regardless of which model was actually running. Removed; every model now identifies correctly.

## Known Limitations

**No tool execution on Bedrock models yet.** Bedrock's Converse API supports tool calling natively (`toolConfig`), but this integration doesn't wire it up yet — Bedrock models currently produce single-shot text responses with no bash, file read, or code execution access. This was caught directly: a generated Fibonacci implementation shipped with a broken matrix-multiplication function and a self-contradicting docstring, because the model never got to run its own code to check it. Full writeup in [`EVALUATION.md`](./EVALUATION.md).

Two direct consequences:
- Bedrock models can't inspect a repo or read files — they can only work with what's pasted into the chat
- Generated code isn't automatically verified before being shown to you

Also not yet done: context compaction on the Bedrock path, and tool/image/reasoning content blocks through the Bedrock wire format (text-only for now).

These are documented, not hidden — see [`ARCHITECTURE.md`](./ARCHITECTURE.md) §6 for the exact fix path.

---

## Docs

- [`ARCHITECTURE.md`](./ARCHITECTURE.md) — the `SamplingBackend` trait, Bedrock client design, streaming architecture, turn routing
- [`EVALUATION.md`](./EVALUATION.md) — what was tested, the bugs found and fixed, what's still unverified
- [`CONTRIBUTIONS.md`](./CONTRIBUTIONS.md) — full diff against upstream Grok Build

---

## License

Apache-2.0, inherited from [xai-org/grok-build](https://github.com/xai-org/grok-build). This project is not affiliated with xAI.

Fork, Bedrock integration, and fixes by [Athelesh Balachandran](https://github.com/Athelesh-7G).
