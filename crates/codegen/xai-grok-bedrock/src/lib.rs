//! Amazon Bedrock backend for AtheCode.
//!
//! This crate adds Bedrock as an additive LLM backend alongside the
//! existing xAI backend. It is selected at runtime via the
//! `ATHECODE_PROVIDER=bedrock` environment variable and authenticates
//! through the standard AWS credential chain (`AWS_ACCESS_KEY_ID`,
//! `AWS_SECRET_ACCESS_KEY`, `AWS_REGION`).
//!
//! Phase 0 is intentionally minimal: it ships only a standalone smoke
//! test binary (`bedrock_smoke_test`) that proves the end-to-end
//! wiring — credential resolution, network reachability, and a single
//! non-streaming Converse call. The provider trait, model-catalog
//! integration, and actor wiring are deliberately out of scope here
//! and land in a later phase.

/// Environment variable that selects the LLM backend (`"xai"` or `"bedrock"`).
pub const PROVIDER_ENV_VAR: &str = "ATHECODE_PROVIDER";

/// The provider value that activates the Bedrock backend.
pub const PROVIDER_BEDROCK: &str = "bedrock";

/// A single selectable Bedrock model: the ID passed to `converse()`, a
/// human-readable display name, and the model's upstream provider.
pub struct BedrockModel {
    pub id: &'static str,
    pub name: &'static str,
    pub provider: &'static str,
}

pub const BEDROCK_MODELS: &[BedrockModel] = &[
    BedrockModel {
        id: "moonshotai.kimi-k2.5",
        name: "Kimi K2.5",
        provider: "Moonshot AI",
    },
    BedrockModel {
        id: "moonshot.kimi-k2-thinking",
        name: "Kimi K2.5 Thinking",
        provider: "Moonshot AI",
    },
    BedrockModel {
        id: "deepseek.v3.2",
        name: "DeepSeek V3.2",
        provider: "DeepSeek",
    },
    BedrockModel {
        id: "google.gemma-3-12b-it",
        name: "Gemma 3 12B",
        provider: "Google",
    },
    BedrockModel {
        id: "amazon.nova-pro-v1:0",
        name: "Nova Pro",
        provider: "Amazon",
    },
    BedrockModel {
        id: "minimax.minimax-m2.5",
        name: "MiniMax M2.5",
        provider: "MiniMax",
    },
    BedrockModel {
        id: "openai.gpt-oss-safeguard-120b",
        name: "GPT-OSS Safeguard 120B",
        provider: "OpenAI",
    },
    BedrockModel {
        id: "qwen.qwen3-coder-next",
        name: "Qwen3 Coder Next",
        provider: "Qwen",
    },
    BedrockModel {
        id: "qwen.qwen3-next-80b-a3b",
        name: "Qwen3 Next 80B",
        provider: "Qwen",
    },
    BedrockModel {
        id: "zai.glm-5",
        name: "GLM-5",
        provider: "Zhipu AI",
    },
    BedrockModel {
        id: "zai.glm-4.7",
        name: "GLM-4.7",
        provider: "Zhipu AI",
    },
];

pub const DEFAULT_BEDROCK_MODEL: &str = "qwen.qwen3-coder-next";
