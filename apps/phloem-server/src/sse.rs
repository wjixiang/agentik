//! SSE helper: converts tokio broadcast stream into axum SSE response.

use std::convert::Infallible;

use axum::response::sse::{Event, KeepAlive, Sse};
use futures::StreamExt;
use tokio::sync::broadcast;
use uuid::Uuid;

use agentik_sdk::types::AgentEvent;

/// Create an SSE body stream that yields events for a specific agent.
///
/// The returned stream filters the global event bus by `agent_id` and
/// converts each `AgentEvent` into an SSE `Event` frame.
pub fn agent_event_stream(
    agent_id: Uuid,
    rx: broadcast::Receiver<(Uuid, AgentEvent)>,
) -> Sse<impl futures::Stream<Item = Result<Event, Infallible>>> {
    let stream = tokio_stream::wrappers::BroadcastStream::new(rx)
        .filter_map(move |result| async move {
            let Ok((id, event)) = result else {
                return None;
            };
            if id != agent_id {
                return None;
            }
            let (event_type, data) = match &event {
                AgentEvent::TextDelta(text) => ("text_delta", text.clone()),
                AgentEvent::ThinkingDelta(text) => ("thinking_delta", text.clone()),
                AgentEvent::UsageUpdate {
                    input_tokens,
                    output_tokens,
                } => (
                    "usage_update",
                    serde_json::json!({ "input_tokens": input_tokens, "output_tokens": output_tokens }).to_string(),
                ),
                AgentEvent::StreamStart { message } => ("stream_start", serde_json::to_string(message).unwrap_or_default()),
                AgentEvent::ContentBlockStart { index, content_block_kind } => (
                    "content_block_start",
                    serde_json::json!({ "index": index, "kind": content_block_kind }).to_string(),
                ),
                AgentEvent::ContentBlockStop { index } => (
                    "content_block_stop",
                    serde_json::json!({ "index": index }).to_string(),
                ),
                AgentEvent::StreamDelta { stop_reason } => (
                    "stream_delta",
                    serde_json::json!({ "stop_reason": stop_reason }).to_string(),
                ),
                AgentEvent::LlmResponse(text) => ("llm_response", text.clone()),
                AgentEvent::Thinking(text) => ("thinking", text.clone()),
                AgentEvent::Requesting => ("requesting", "".to_string()),
                AgentEvent::ToolCall { name, input } => (
                    "tool_call",
                    serde_json::json!({ "name": name, "input": input }).to_string(),
                ),
                AgentEvent::ToolResult { ok, content } => (
                    "tool_result",
                    serde_json::json!({ "ok": ok, "content": content }).to_string(),
                ),
                AgentEvent::Done => ("done", "".to_string()),
                AgentEvent::Error(msg) => ("error", msg.clone()),
            };

            Some(Ok(Event::default()
                .event(event_type)
                .data(data)))
        });

    Sse::new(stream).keep_alive(KeepAlive::default())
}
