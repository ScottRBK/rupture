use anyhow::Result;
use rupture_server::app::App;

#[tokio::main]
async fn main() -> Result<()> {

    let app = App::new("0.0.0.0:3000".to_string());

    app.start().await?;

    Ok(())
}


