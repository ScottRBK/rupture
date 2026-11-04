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

    let model = Model::new("qwen3.8-flash-next-stock", None, None);

    let llm_config = LlmConfig {
        model,
        reasoning_effort: ferox_ai::models::ReasoningEffort::None,
    };

    let mut messages: Vec<Message> = Vec::new();

    let agent_config = AgentConfig::new("API Agent".to_string(), None, None);

    let session = AgentSession::new(agent_config, llm_config, gateway, messages);

    let stream = session.text_prompt(prompt).map(|result| match result {
        Ok(chunk) => Ok(Event::default().data(chunk.chunk_text)),
        Err(error) => {
            eprintln!("agent stream failed: {error:#}");

            Ok(Event::default()
                .event("error")
                .data("The agent failed to generate a response"))
        }
    });

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

fn handle_sse_line(line: &str) -> Option<Result<String>> {
    let line = line.trim();

    if line.is_empty() {
        return None;
    }

    let data = line.strip_prefix("data:")?.trim_start();
    Some(Ok(data.to_string()))

}

#[derive(serde::Deserialize)]
struct PromptRequest {
    prompt: String,
}
