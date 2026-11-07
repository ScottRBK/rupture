use crate::state::AppState;
use anyhow::Result;
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
    routing::{get, post},
};
use ferox_ai::{
    models::{Message, Model},
};
use futures_util::stream::{Stream};
use rupture_agents::{AgentConfig, AgentSession, LlmConfig};
use rupture_contracts::stream_events::TextStreamEvent;
use std::{sync::Arc};
use tokio_stream::StreamExt;

pub fn router(state: AppState) -> Router<()> {
    Router::new()
        .route("/", get(root))
        .route("/ping", get(pong))
        .route("/prompt", post(prompt))
        .with_state(state)
}

async fn root() -> String {
    "Welcome to the rupture server".into()
}

async fn pong() -> String {
    "Pong".into()
}

// pass the following in order to retrieve a response
// - prompt
// - model
// - reasoning effort (optional)
// - session id (optional as might be new)
async fn prompt(
    State(state): State<AppState>,
    Json(request): Json<PromptRequest>,
) -> std::result::Result<Sse<impl Stream<Item = Result<Event>>>, (StatusCode, &'static str)> {
    let gateway = match state.providers.get("openai_compatible") {
        Some(crate::state::Provider::OpenAiCompatible(gateway)) => Arc::clone(gateway),
        None => {
            return Err((StatusCode::BAD_REQUEST, "unrecognised provider"));
        }
    };

    let prompt = request.prompt;

    let model = Model::new("glm-5.3-flash", None, None);

    let llm_config = LlmConfig {
        model,
        reasoning_effort: ferox_ai::models::ReasoningEffort::None,
    };

    let messages: Vec<Message> = Vec::new();

    let agent_config = AgentConfig::new("API Agent".to_string(), None, None);

    let session = AgentSession::new(agent_config, llm_config, gateway, messages);

    let stream = session.text_prompt(prompt).map(|result| match result {
        Ok(chunk) => Ok(text_to_sse(&TextStreamEvent::Chunk {
            text: chunk.chunk_text,
            token_count: chunk.token_count,
            is_reasoning: chunk.is_reasoning,
        })),
        Err(error) => {
            eprintln!("agent stream failed: {error:#}");
            Ok(text_to_sse(&TextStreamEvent::Error {
                message: "The agent failed to generate a response".into(),
            }))
        }
    });

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

fn text_to_sse(event: &TextStreamEvent) -> Event {
    let payload = serde_json::to_string(event)
        .unwrap_or_else(|_| r#"{"type":"error","message":"internal serialization failure"}"#.into());

    Event::default().data(payload)
}

#[derive(serde::Deserialize)]
struct PromptRequest {
    prompt: String,
}
