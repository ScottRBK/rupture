use anyhow::Result;
use rupture_server::app::App;
use rupture_server::settings::Settings;

#[tokio::main]
async fn main() -> Result<()> {

    let settings = Settings::new()?;

    let app = App::new(format!("{}:{}", settings.host, settings.port));

    app.start().await?;

    Ok(())
}


