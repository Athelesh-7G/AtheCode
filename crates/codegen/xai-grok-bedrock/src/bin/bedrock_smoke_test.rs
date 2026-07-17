//! Phase 0 smoke test: prove end-to-end Bedrock wiring.
//!
//! Sends a single non-streaming Converse request to Amazon Bedrock and
//! prints the model's reply. No streaming, no tool-calling, no
//! integration with the sampler actor or model catalog — this exists
//! only to confirm that credentials resolve, the network is reachable,
//! and the Converse request/response shape works end to end.
//!
//! Run it with:
//!
//! ```sh
//! ATHECODE_PROVIDER=bedrock \
//! AWS_REGION=us-east-1 \
//! AWS_ACCESS_KEY_ID=... \
//! AWS_SECRET_ACCESS_KEY=... \
//!   cargo run -p xai-grok-bedrock --bin bedrock_smoke_test
//! ```
//!
//! To test a specific model from [`xai_grok_bedrock::BEDROCK_MODELS`]
//! instead of the default, set `ATHECODE_MODEL`:
//!
//! ```sh
//! ATHECODE_PROVIDER=bedrock \
//! ATHECODE_MODEL=zai.glm-5 \
//! AWS_REGION=us-east-1 \
//! AWS_ACCESS_KEY_ID=... \
//! AWS_SECRET_ACCESS_KEY=... \
//!   cargo run -p xai-grok-bedrock --bin bedrock_smoke_test
//! ```

use xai_grok_bedrock::{BedrockClient, DEFAULT_BEDROCK_MODEL, PROVIDER_BEDROCK, PROVIDER_ENV_VAR};
use xai_grok_sampling_types::{ConversationItem, ConversationRequest, SamplingBackend};

/// Environment variable that overrides the model to test, so other
/// catalog entries can be exercised without editing code.
const MODEL_OVERRIDE_ENV_VAR: &str = "ATHECODE_MODEL";

/// The single test prompt.
const PROMPT: &str = "Say hello in exactly 5 words.";

#[tokio::main]
async fn main() {
    if let Err(msg) = run().await {
        eprintln!("bedrock_smoke_test: {msg}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    // 1. Gate on ATHECODE_PROVIDER=bedrock so this binary is a no-op
    //    unless Bedrock is explicitly selected.
    match std::env::var(PROVIDER_ENV_VAR) {
        Ok(v) if v == PROVIDER_BEDROCK => {}
        Ok(other) => {
            return Err(format!(
                "{PROVIDER_ENV_VAR}=\"{other}\" (expected \"{PROVIDER_BEDROCK}\"); \
                 nothing to do. Set {PROVIDER_ENV_VAR}={PROVIDER_BEDROCK} to run this test."
            ));
        }
        Err(_) => {
            return Err(format!(
                "{PROVIDER_ENV_VAR} is not set; nothing to do. \
                 Set {PROVIDER_ENV_VAR}={PROVIDER_BEDROCK} to run this test."
            ));
        }
    }

    // 2. Resolve which model to test: ATHECODE_MODEL overrides the
    //    catalog default, so any BEDROCK_MODELS entry (or an arbitrary
    //    id) can be exercised without editing code.
    let model_id = std::env::var(MODEL_OVERRIDE_ENV_VAR)
        .unwrap_or_else(|_| DEFAULT_BEDROCK_MODEL.to_string());

    // 3. Build the real BedrockClient (reads AWS_REGION + the AWS
    //    credential chain internally) and drive it through the shared
    //    SamplingBackend interface — the same path the rest of the
    //    system will use.
    let client = BedrockClient::new(&model_id)
        .await
        .map_err(|e| format!("failed to initialize Bedrock client: {e}"))?;

    eprintln!(
        "Using Bedrock model \"{model_id}\" in region \"{}\".",
        client.region()
    );
    eprintln!("Prompt: {PROMPT}");

    let request =
        ConversationRequest::from_items(vec![ConversationItem::user(PROMPT)]).with_model(model_id);

    // 4. Non-streaming completion via the trait.
    let response = client
        .chat_completion(request)
        .await
        .map_err(|e| format!("Bedrock request failed: {e}"))?;

    let text = response.assistant_text();
    if text.trim().is_empty() {
        return Err("Bedrock returned a response with no text content.".to_string());
    }

    println!("{text}");
    Ok(())
}
