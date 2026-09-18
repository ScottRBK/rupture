use uuid::Uuid;
use futures_core::Stream;
use futures::stream::StreamExt;
use async_stream::try_stream;
use ferox_ai::{Gateway, models::{
    CompletionChunk,
    CompletionRequest,
    Message,
    Model,
    ReasoningEffort,
    Tool,
}};
use crate::errors::AgentError;

#[non_exhaustive]
pub struct AgentConfig {
    // Name of the agent
    pub name: String, 
    // System prompt for the agent that will be injected ahead of any call to them 
    pub system_prompt: String,
    // Optional list of tools available to the agent 
    pub tools: Option<Vec<Tool>>,
}

pub struct LlmConfig { 
    // Large Language Model that is to be used for the Agent
    model: Model,
    // The reasoning effor that will be used by the large language model for agent requests
    reasoning_effort: ReasoningEffort,
}

#[non_exhaustive]
pub struct AgentSession<P>
{
    // Unique identifier for the session 
    pub id: Uuid,
    // The agent configuration that will be used for the session 
    pub agent_config: AgentConfig,
    // The Ferox large language model configuration that will be used for the session 
    pub llm_config: LlmConfig,
    // The ferox gateway that will be used to pass data to the Large Language Model
    gateway: ferox_ai::Gateway<P>,
    // List of messages that are in the session
    messages: Vec<Message>,
}


#[non_exhaustive]
pub struct AgentChunkResponse {
    pub chunk_text: String,
}

impl<P> AgentSession<P> 
where 
    P: ferox_ai::LlmProvider
{
    // Creates a new agent session
    pub fn new(
        agent_config: AgentConfig,
        llm_config: LlmConfig,
        gateway: Gateway<P>,
        messages: Vec<Message>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            agent_config,
            llm_config,
            gateway,
            messages,
        }
    }

    pub fn text_prompt(&mut self, prompt: &str) -> 
    impl Stream<Item = Result<AgentChunkResponse, AgentError>> {
        try_stream! {

            self.messages.push(Message::User{ content: prompt.into() });

            let mut request = CompletionRequest::new(
                self.llm_config.model.id.clone(),
                &self.messages,
            );

            request.tools = self.agent_config.tools.clone();
            request.reasoning_effort = Some(self.llm_config.reasoning_effort);

            let stream = self.gateway.stream(request).await?;
            futures::pin_mut!(stream); 
            while let Some(chunk) = stream.next().await {
                if let Some(text) = chunk?.text {
                    yield AgentChunkResponse { chunk_text: text }
                }
            }
        }
    } 
}


