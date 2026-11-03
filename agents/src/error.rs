use thiserror::Error;
use ferox_ai::error::GatewayError;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AgentError {
    #[error("llm provider error: {0}")]
    ProviderError(#[from] GatewayError)
}

