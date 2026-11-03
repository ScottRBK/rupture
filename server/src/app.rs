use anyhow::Result;
use axum;
use crate::{
    routes::api,
    state::AppState,
};

pub struct App {
    addr: String,
}

impl App {
    pub fn new(addr: String) -> Self {

        Self { addr }
    }

    pub async fn start(self) -> Result<()> {

        let state = AppState::new()?;

        let router =  api::router(state);       

        let listener = tokio::net::TcpListener::bind(self.addr).await?;

        axum::serve(listener, router).await?;

        Ok(())

    }

}

