use anyhow::{Context, Result};
use ferox_ai::{
    Gateway,
    openai_compatible::{OpenAiCompatibleClient, OpenAiCompatibleClientBuilder},
};
use std::{collections::HashMap, sync::Arc};

pub enum Provider {
    OpenAiCompatible(Arc<Gateway<OpenAiCompatibleClient>>),
}

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) providers: Arc<ProviderRegistry>,
}

impl AppState {
    pub(crate) fn new() -> Result<Self> {
        Ok(Self {
            providers: Self::builder_providers()?,
        })
    }

    fn builder_providers() -> Result<Arc<ProviderRegistry>> {
        let mut providers = HashMap::new();

        let client = OpenAiCompatibleClient::builder()
            .base_url("http://192.168.1.201:8018/v1")
            .build()
            .context("failed to build OpenAI-compatible client")?;

        let gateway = Arc::new(Gateway::new(client));

        providers.insert(
            "openai_compatible".to_string(),
            Provider::OpenAiCompatible(gateway),
        );

        Ok(Arc::new(ProviderRegistry { providers }))
    }
}

pub(crate) struct ProviderRegistry {
    providers: HashMap<String, Provider>,
}

impl ProviderRegistry {
    pub(crate) fn get(&self, provider_id: &str) -> Option<&Provider> {
        self.providers.get(provider_id)
    }
}
