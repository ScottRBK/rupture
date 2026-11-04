use serde::{Serialize, Deserialize};

const SETTINGS_FILE_NAME_ENV: &str = "RUPTURE_SETTINGS_FILENAME";

#[derive(Deserialize, Serialize, Debug, Default)]
#[serde(default)]
pub struct Settings {
    pub models_filename: String,
    pub host: String,
    pub port: String,
}

impl Settings {
    pub fn new() -> Settings {
        let settings_file = std::env::var(SETTINGS_FILE_NAME_ENV)
            .unwrap_or_else(|_| "settings.toml".to_string());

        Settings {
            models_filename: String::from("models.toml"),
            host: String::from("127.0.0.1"),
            port: String::from("3000"),
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_setings() {
        let settings = Settings::new();
        assert_eq!(settings.models_filename, "models.toml");
        assert_eq!(settings.host, "127.0.0.1");
        assert_eq!(settings.port, "3000");
    }

}
