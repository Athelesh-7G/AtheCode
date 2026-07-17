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

use aws_sdk_bedrockruntime::Client;
use aws_sdk_bedrockruntime::error::{DisplayErrorContext, SdkError};
use aws_sdk_bedrockruntime::types::{ContentBlock, ConversationRole, ConverseOutput, Message};

use xai_grok_bedrock::{DEFAULT_BEDROCK_MODEL, PROVIDER_BEDROCK, PROVIDER_ENV_VAR};

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

    // 2. Resolve the region from AWS_REGION. The AWS credential chain
    //    picks up AWS_ACCESS_KEY_ID / AWS_SECRET_ACCESS_KEY from the
    //    environment automatically — no manual SigV4 handling.
    let region = std::env::var("AWS_REGION").map_err(|_| {
        "AWS_REGION is not set. Set it to your Bedrock region, e.g. AWS_REGION=us-east-1."
            .to_string()
    })?;

    let aws_config = aws_config::from_env()
        .region(aws_sdk_bedrockruntime::config::Region::new(region.clone()))
        .load()
        .await;
    let client = Client::new(&aws_config);

    // 2b. Resolve which model to test: ATHECODE_MODEL overrides the
    //     catalog default, so any BEDROCK_MODELS entry (or an
    //     arbitrary id) can be exercised without editing code.
    let model_id = std::env::var(MODEL_OVERRIDE_ENV_VAR)
        .unwrap_or_else(|_| DEFAULT_BEDROCK_MODEL.to_string());

    eprintln!("Using Bedrock model \"{model_id}\" in region \"{region}\".");
    eprintln!("Prompt: {PROMPT}");

    // 3. Build a single user message and send a non-streaming Converse
    //    request.
    let user_message = Message::builder()
        .role(ConversationRole::User)
        .content(ContentBlock::Text(PROMPT.to_string()))
        .build()
        .map_err(|e| format!("failed to build request message: {e}"))?;

    let response = client
        .converse()
        .model_id(&model_id)
        .messages(user_message)
        .send()
        .await
        .map_err(|e| describe_send_error(&model_id, e))?;

    // 4. Extract and print the reply text.
    let output = response
        .output()
        .ok_or_else(|| "Bedrock returned an empty response (no output).".to_string())?;

    let text = extract_text(output)?;
    if text.trim().is_empty() {
        return Err("Bedrock returned a response with no text content.".to_string());
    }

    println!("{text}");
    Ok(())
}

/// Pull the concatenated text from a Converse output message.
fn extract_text(output: &ConverseOutput) -> Result<String, String> {
    let message = output
        .as_message()
        .map_err(|_| "Bedrock output was not a message.".to_string())?;

    let mut text = String::new();
    for block in message.content() {
        if let Ok(chunk) = block.as_text() {
            text.push_str(chunk);
        }
    }
    Ok(text)
}

/// Turn an SdkError into a readable, actionable message instead of a
/// raw debug dump or panic.
fn describe_send_error<E, R>(model_id: &str, err: SdkError<E, R>) -> String
where
    E: std::error::Error + 'static,
    R: std::fmt::Debug,
{
    match &err {
        SdkError::ConstructionFailure(_) => format!(
            "Failed to build the request, most likely missing or invalid AWS credentials. \
             Check AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY. Details: {}",
            DisplayErrorContext(&err)
        ),
        SdkError::DispatchFailure(_) => format!(
            "Network/dispatch failure reaching Bedrock. Check connectivity and AWS_REGION. \
             Details: {}",
            DisplayErrorContext(&err)
        ),
        SdkError::TimeoutError(_) => format!(
            "Request to Bedrock timed out. Details: {}",
            DisplayErrorContext(&err)
        ),
        SdkError::ResponseError(_) => format!(
            "Bedrock returned an unparseable response. Details: {}",
            DisplayErrorContext(&err)
        ),
        SdkError::ServiceError(_) => format!(
            "Bedrock rejected the request. This usually means the credentials lack \
             bedrock:InvokeModel permission, the model \"{model_id}\" is not enabled/accessible \
             in this account or region, or the model id is wrong. Details: {}",
            DisplayErrorContext(&err)
        ),
        _ => format!(
            "Bedrock request failed. Details: {}",
            DisplayErrorContext(&err)
        ),
    }
}
