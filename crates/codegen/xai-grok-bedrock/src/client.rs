//! Bedrock implementation of the shared [`SamplingBackend`] trait.
//!
//! [`BedrockClient`] translates the provider-agnostic
//! [`ConversationRequest`] into an Amazon Bedrock **Converse** call
//! (unified message format across model families — Claude, Nova, Llama,
//! Mistral, …), then translates the result back into the shared
//! [`ConversationResponse`] / [`SamplingEvent`] types.
//!
//! Phase 1 Part A scope: text content only. Tool calls, tool results,
//! images, and reasoning blocks are intentionally not yet mapped
//! through the Bedrock wire format — they are skipped during
//! conversion so the core text round-trip is proven first.

use async_trait::async_trait;
use futures_util::stream::BoxStream;

use aws_sdk_bedrockruntime::Client;
use aws_sdk_bedrockruntime::error::DisplayErrorContext;
use aws_sdk_bedrockruntime::types::{
    ContentBlock, ContentBlockDelta, ConversationRole, ConverseStreamOutput as BedrockStreamEvent,
    InferenceConfiguration, Message, StopReason as BedrockStopReason, SystemContentBlock,
    TokenUsage as BedrockTokenUsage,
};

use xai_grok_sampling_types::{
    ContentPart, ConversationItem, ConversationRequest, ConversationResponse, RequestId,
    SamplingBackend, SamplingChannel, SamplingError, SamplingErrorInfo, SamplingEvent, StopReason,
    TokenUsage,
};

/// A Bedrock-backed sampling client.
///
/// Holds the AWS Bedrock runtime client, the resolved region, and the
/// default model id used when a [`ConversationRequest`] does not carry
/// its own `model`.
pub struct BedrockClient {
    client: Client,
    region: String,
    default_model_id: String,
}

impl BedrockClient {
    /// Build a client from the ambient AWS environment.
    ///
    /// Reads `AWS_REGION` for the region and relies on the standard AWS
    /// credential chain (`AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` /
    /// profile / IAM role) for authentication — the SDK performs SigV4
    /// signing, so no credentials are handled here directly.
    ///
    /// Async because building the AWS SDK config resolves credentials
    /// and region providers asynchronously.
    pub async fn new(model_id: &str) -> Result<Self, SamplingError> {
        let region = std::env::var("AWS_REGION").map_err(|_| {
            SamplingError::Auth(
                "AWS_REGION is not set. Set it to your Bedrock region, e.g. us-east-1.".to_string(),
            )
        })?;

        let aws_config = aws_config::from_env()
            .region(aws_sdk_bedrockruntime::config::Region::new(region.clone()))
            .load()
            .await;

        Ok(Self {
            client: Client::new(&aws_config),
            region,
            default_model_id: model_id.to_string(),
        })
    }

    /// The region this client is bound to.
    pub fn region(&self) -> &str {
        &self.region
    }

    /// The default model id used when a request omits its own.
    pub fn default_model_id(&self) -> &str {
        &self.default_model_id
    }

    /// Resolve the model id for a request: the request's own `model`, or
    /// this client's default.
    fn model_for(&self, request: &ConversationRequest) -> String {
        request
            .model
            .clone()
            .unwrap_or_else(|| self.default_model_id.clone())
    }
}

/// Converted Converse inputs: system prompt blocks, alternating
/// messages, and the inference-config knobs.
struct ConverseInput {
    system: Vec<SystemContentBlock>,
    messages: Vec<Message>,
    inference_config: InferenceConfiguration,
}

/// Translate a [`ConversationRequest`] into Bedrock Converse inputs.
///
/// Text-only for Part A: `System` items become system content blocks,
/// `User`/`Assistant` text becomes messages, and every other item kind
/// (tool calls/results, backend tool calls, reasoning, images) is
/// skipped. Messages that would carry no content block are dropped so
/// Bedrock never receives an empty-content message.
fn build_converse_input(request: &ConversationRequest) -> Result<ConverseInput, SamplingError> {
    let mut system = Vec::new();
    let mut messages = Vec::new();

    for item in &request.items {
        match item {
            ConversationItem::System(sys) => {
                system.push(SystemContentBlock::Text(sys.content.to_string()));
            }
            ConversationItem::User(user) => {
                let blocks = content_parts_to_blocks(&user.content);
                if !blocks.is_empty() {
                    messages.push(
                        Message::builder()
                            .role(ConversationRole::User)
                            .set_content(Some(blocks))
                            .build()
                            .map_err(|e| {
                                SamplingError::serialization_message(format!("failed to build Bedrock user message: {e}"))
                            })?,
                    );
                }
            }
            ConversationItem::Assistant(assistant) => {
                let text = assistant.content.to_string();
                if !text.is_empty() {
                    messages.push(
                        Message::builder()
                            .role(ConversationRole::Assistant)
                            .content(ContentBlock::Text(text))
                            .build()
                            .map_err(|e| {
                                SamplingError::serialization_message(format!("failed to build Bedrock assistant message: {e}"))
                            })?,
                    );
                }
            }
            // KNOWN LIMITATION: Bedrock tool calling (toolConfig) is not yet
            // implemented. This means Bedrock models cannot execute bash, read
            // files, or verify generated code — they only produce single-shot
            // text responses. Generated code is NOT executed or tested before
            // being shown to the user. This is a real correctness gap: models
            // may produce code with bugs that would be caught by execution.
            // TODO: Wire ConversationItem::ToolCall/ToolResult through
            // ToolConfiguration and parse ContentBlock::ToolUse from streaming
            // responses to close this gap.
            _ => {}
        }
    }

    let mut inference_config = InferenceConfiguration::builder();
    if let Some(max) = request.max_output_tokens {
        inference_config = inference_config.max_tokens(i32::try_from(max).unwrap_or(i32::MAX));
    }
    if let Some(temp) = request.temperature {
        inference_config = inference_config.temperature(temp);
    }
    if let Some(top_p) = request.top_p {
        inference_config = inference_config.top_p(top_p);
    }

    Ok(ConverseInput {
        system,
        messages,
        inference_config: inference_config.build(),
    })
}

/// Map provider-agnostic content parts to Bedrock content blocks.
/// Text passes through; images are skipped in Part A.
fn content_parts_to_blocks(parts: &[ContentPart]) -> Vec<ContentBlock> {
    parts
        .iter()
        .filter_map(|part| match part {
            ContentPart::Text { text } => Some(ContentBlock::Text(text.to_string())),
            ContentPart::Image { .. } => None,
        })
        .collect()
}

/// Map a Bedrock stop reason to the provider-agnostic [`StopReason`].
fn map_stop_reason(reason: &BedrockStopReason) -> StopReason {
    match reason {
        BedrockStopReason::EndTurn | BedrockStopReason::StopSequence => StopReason::Stop,
        BedrockStopReason::MaxTokens => StopReason::Length,
        BedrockStopReason::ToolUse => StopReason::ToolCalls,
        BedrockStopReason::ContentFiltered | BedrockStopReason::GuardrailIntervened => {
            StopReason::ContentFilter
        }
        // Unknown / future variants: treat as a natural stop.
        _ => StopReason::Stop,
    }
}

/// Map Bedrock token usage to the provider-agnostic [`TokenUsage`].
fn map_usage(usage: &BedrockTokenUsage) -> TokenUsage {
    TokenUsage {
        prompt_tokens: u32::try_from(usage.input_tokens()).unwrap_or(0),
        completion_tokens: u32::try_from(usage.output_tokens()).unwrap_or(0),
        total_tokens: u32::try_from(usage.total_tokens()).unwrap_or(0),
        reasoning_tokens: 0,
        cached_prompt_tokens: 0,
    }
}

/// Build a [`ConversationResponse`] from the assistant text plus
/// optional stop reason / usage.
fn build_response(
    text: String,
    model_id: &str,
    stop_reason: Option<StopReason>,
    usage: Option<TokenUsage>,
) -> ConversationResponse {
    ConversationResponse {
        items: vec![ConversationItem::assistant_with_model(text, model_id)],
        stop_reason,
        usage,
        cost_usd_ticks: None,
        message_chunks_emitted: 0,
        doom_loop_signals: Vec::new(),
        stop_message: None,
    }
}

/// Convenience: an `Api`-flavored error carrying a human message.
fn api_error(message: String) -> SamplingError {
    SamplingError::api_error(500, message)
}

#[async_trait]
impl SamplingBackend for BedrockClient {
    async fn chat_completion(
        &self,
        request: ConversationRequest,
    ) -> Result<ConversationResponse, SamplingError> {
        let model_id = self.model_for(&request);
        let input = build_converse_input(&request)?;

        let response = self
            .client
            .converse()
            .model_id(&model_id)
            .set_system(Some(input.system))
            .set_messages(Some(input.messages))
            .inference_config(input.inference_config)
            .send()
            .await
            .map_err(|e| {
                api_error(format!(
                    "Bedrock Converse request failed: {}",
                    DisplayErrorContext(&e)
                ))
            })?;

        let output = response
            .output()
            .ok_or_else(|| api_error("Bedrock returned an empty response (no output).".into()))?;
        let message = output
            .as_message()
            .map_err(|_| api_error("Bedrock output was not a message.".into()))?;

        let mut text = String::new();
        for block in message.content() {
            if let Ok(chunk) = block.as_text() {
                text.push_str(chunk);
            }
        }

        let stop_reason = Some(map_stop_reason(response.stop_reason()));
        let usage = response.usage().map(map_usage);

        Ok(build_response(text, &model_id, stop_reason, usage))
    }

    async fn chat_completion_stream(
        &self,
        request: ConversationRequest,
    ) -> Result<BoxStream<'static, SamplingEvent>, SamplingError> {
        let model_id = self.model_for(&request);
        let input = build_converse_input(&request)?;

        let mut response = self
            .client
            .converse_stream()
            .model_id(&model_id)
            .set_system(Some(input.system))
            .set_messages(Some(input.messages))
            .inference_config(input.inference_config)
            .send()
            .await
            .map_err(|e| {
                api_error(format!(
                    "Bedrock ConverseStream request failed: {}",
                    DisplayErrorContext(&e)
                ))
            })?;

        let request_id = RequestId::random();

        let stream = async_stream::stream! {
            let now_ms = || chrono_millis();
            yield SamplingEvent::StreamStarted {
                request_id: request_id.clone(),
                timestamp_ms: now_ms(),
            };

            let mut accumulated = String::new();
            let mut chunk_index: u64 = 0;
            let mut first_token_sent = false;
            let mut stop_reason: Option<StopReason> = None;
            let mut usage: Option<TokenUsage> = None;

            loop {
                match response.stream.recv().await {
                    Ok(Some(event)) => match event {
                        BedrockStreamEvent::ContentBlockDelta(delta_event) => {
                            if let Some(ContentBlockDelta::Text(text)) = delta_event.delta {
                                if !first_token_sent {
                                    first_token_sent = true;
                                    yield SamplingEvent::FirstToken {
                                        request_id: request_id.clone(),
                                    };
                                }
                                accumulated.push_str(&text);
                                yield SamplingEvent::ChannelToken {
                                    request_id: request_id.clone(),
                                    channel: SamplingChannel::Text,
                                    text,
                                    chunk_index,
                                };
                                chunk_index += 1;
                            }
                        }
                        BedrockStreamEvent::MessageStop(stop) => {
                            stop_reason = Some(map_stop_reason(&stop.stop_reason));
                        }
                        BedrockStreamEvent::Metadata(meta) => {
                            if let Some(u) = meta.usage() {
                                usage = Some(map_usage(u));
                            }
                        }
                        // MessageStart / ContentBlockStart / ContentBlockStop
                        // and any future variants carry no text for Part A.
                        _ => {}
                    },
                    Ok(None) => break,
                    Err(e) => {
                        let err = SamplingError::StreamError {
                            error_type: "bedrock".into(),
                            message: format!("{}", DisplayErrorContext(&e)),
                        };
                        yield SamplingEvent::Failed {
                            request_id: request_id.clone(),
                            error: SamplingErrorInfo::from(&err),
                        };
                        return;
                    }
                }
            }

            let response = build_response(accumulated, &model_id, stop_reason, usage);
            yield SamplingEvent::Completed {
                request_id: request_id.clone(),
                response: Box::new(response),
                metrics: Default::default(),
            };
        };

        Ok(Box::pin(stream))
    }
}

/// Current wall-clock time in epoch milliseconds (best-effort; `0` if the
/// clock is before the Unix epoch).
fn chrono_millis() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
