use ferox_ai::{
    Gateway,
    error::LlmError,
    models::{
        CompletionChunk,
        CompletionRequest,
        CompletionResponse,
        Model,
        ReasoningEffort,
    },
};
use futures::{StreamExt, stream};
use rupture_agents::{AgentConfig, AgentSession, LlmConfig};
use std::sync::Arc;

/// One scripted provider update: either a chunk with reasoning and/or answer
/// text, or a failure.
enum Script {
    Ok {
        reasoning: Option<&'static str>,
        text: Option<&'static str>,
    },
    Err(&'static str),
}

/// A provider that replays a scripted sequence of updates for every stream
/// request.
struct FakeProvider {
    script: Arc<Vec<Script>>,
}

impl FakeProvider {
    fn chunk(reasoning: Option<&'static str>, text: Option<&'static str>) -> CompletionChunk {
        let mut chunk = CompletionChunk::new(vec![], None);
        chunk.reasoning = reasoning.map(Into::into);
        chunk.text = text.map(Into::into);
        chunk
    }
}

impl ferox_ai::LlmProvider for FakeProvider {
    type CompletionStream = stream::Iter<std::vec::IntoIter<Result<CompletionChunk, LlmError>>>;

    async fn complete(
        &self,
        _request: CompletionRequest<'_>,
    ) -> Result<CompletionResponse, LlmError> {
        Err(LlmError::ProviderFailure {
            message: "not expected in these tests".into(),
        })
    }

    async fn stream(
        &self,
        _request: CompletionRequest<'_>,
    ) -> Result<Self::CompletionStream, LlmError> {
        let items = self
            .script
            .iter()
            .map(|step| match step {
                Script::Ok { reasoning, text } => Ok(Self::chunk(*reasoning, *text)),
                Script::Err(message) => Err(LlmError::ProviderFailure {
                    message: (*message).into(),
                }),
            })
            .collect::<Vec<_>>();
        Ok(stream::iter(items))
    }

    async fn list_models(&self) -> Result<Vec<Model>, LlmError> {
        Err(LlmError::ProviderFailure {
            message: "not expected in these tests".into(),
        })
    }
}

fn session_with(script: Vec<Script>) -> AgentSession<FakeProvider> {
    let agent_config = AgentConfig::new("test agent".into(), None, None);
    let llm_config = LlmConfig {
        model: Model::new("test-model", None, None),
        reasoning_effort: ReasoningEffort::Medium,
    };
    let gateway = Arc::new(Gateway::new(FakeProvider {
        script: Arc::new(script),
    }));
    AgentSession::new(agent_config, llm_config, gateway, vec![])
}

async fn collect<S: futures_core::Stream>(stream: S) -> Vec<S::Item> {
    let mut stream = Box::pin(stream);
    let mut results = Vec::new();
    while let Some(item) = stream.as_mut().next().await {
        results.push(item);
    }
    results
}

// A reasoning chunk from the provider surfaces to the caller flagged as
// thinking, so the UI can distinguish it from the answer.
#[tokio::test]
async fn reasoning_chunk_is_surfaced_with_is_reasoning_true() {
    let stream = session_with(vec![Script::Ok {
        reasoning: Some("let me think"),
        text: None,
    }])
    .text_prompt("hello".into());

    let results = collect(stream).await;

    assert_eq!(results.len(), 1, "expected exactly one chunk");
    let chunk = match &results[0] {
        Ok(chunk) => chunk,
        Err(error) => panic!("chunk should not be an error: {error:#}"),
    };
    assert_eq!(chunk.chunk_text, "let me think");
    assert!(chunk.is_reasoning);
}

// Providers send thinking before the answer. Both must surface, in order,
// with the flag set so a caller can tell them apart.
#[tokio::test]
async fn thinking_then_answer_are_surfaced_in_order() {
    let stream = session_with(vec![
        Script::Ok {
            reasoning: Some("pondering"),
            text: None,
        },
        Script::Ok {
            reasoning: None,
            text: Some("the answer"),
        },
    ])
    .text_prompt("hello".into());

    let results = collect(stream).await;

    assert_eq!(results.len(), 2, "expected two chunks");
    let thinking = match &results[0] {
        Ok(chunk) => chunk,
        Err(error) => panic!("first chunk should not error: {error:#}"),
    };
    assert_eq!(thinking.chunk_text, "pondering");
    assert!(thinking.is_reasoning);
    let answer = match &results[1] {
        Ok(chunk) => chunk,
        Err(error) => panic!("second chunk should not error: {error:#}"),
    };
    assert_eq!(answer.chunk_text, "the answer");
    assert!(!answer.is_reasoning);
}

// A provider failure mid-stream is reported as an error item rather than
// being silently swallowed or panicking.
#[tokio::test]
async fn provider_failure_is_surfaced_as_an_error() {
    let stream = session_with(vec![
        Script::Ok {
            reasoning: None,
            text: Some("partial"),
        },
        Script::Err("boom"),
    ])
    .text_prompt("hello".into());

    let results = collect(stream).await;

    assert_eq!(results.len(), 2, "expected one chunk then one error");
    assert!(results[0].is_ok(), "first item should be a chunk");
    let error = match &results[1] {
        Ok(_) => panic!("second item should be an error"),
        Err(error) => error,
    };
    assert!(
        error.to_string().contains("llm provider error"),
        "unexpected error: {error:#}"
    );
}
