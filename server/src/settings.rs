use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::fs::File;
use std::io::Read;

const SETTINGS_FILE_NAME_ENV: &str = "RUPTURE_SETTINGS_FILENAME";
const DEFAULT_SETTINGS_FILE: &str = "rupture.toml";

/// The single source of truth for settings.
/// One line per setting: field name, env var override, default value.
/// The macro derives the struct, the defaults, and the env overrides from it.
macro_rules! define_settings {
    ( $( $name:ident : $env:expr, $default:expr ; )+ ) => {
        #[derive(Deserialize, Serialize, Debug, Clone)]
        #[serde(default)]
        pub struct Settings {
            $( pub $name: String, )*
        }

        impl Default for Settings {
            fn default() -> Self {
                Self {
                    $( $name: $default.to_string(), )*
                }
            }
        }

        impl Settings {
            /// Resolution order per setting: env var -> file value -> default.
            /// A missing file falls back to defaults; a malformed file is an error.
            pub fn load_from(path: &str) -> Result<Settings> {
                let mut settings = match File::open(path) {
                    Ok(mut file) => {
                        let mut contents = String::new();
                        file.read_to_string(&mut contents)?;
                        let parsed: SettingsFile = toml::from_str(&contents)?;
                        parsed.settings
                    }
                    Err(_) => Settings::default(),
                };
                $(
                    if let Ok(value) = std::env::var($env) {
                        settings.$name = value;
                    }
                )*
                Ok(settings)
            }

            pub fn new() -> Result<Settings> {
                let path = std::env::var(SETTINGS_FILE_NAME_ENV)
                    .unwrap_or_else(|_| DEFAULT_SETTINGS_FILE.to_string());

                if !std::path::Path::new(&path).exists() {
                    Self::write_settings_file(&path);
                }

                Self::load_from(&path)
            }

            fn write_settings_file(path: &str) -> Result<()> {
                let settings_file = SettingsFile{ settings: Settings::default() };
                let toml = toml::to_string(&settings_file)?;
                fs::write(path, toml)?;
                Ok(())
            }
        }
    };
}

#[derive(Deserialize, Serialize)]
struct SettingsFile {
    settings: Settings,
}

define_settings! {
    models_filename: "RUPTURE_MODELS_FILENAME", "models.toml";
    host:            "RUPTURE_HOST",            "127.0.0.1";
    port:            "RUPTURE_PORT",            "3000";
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;
    use std::sync::{Mutex, MutexGuard};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn lock_env() -> MutexGuard<'static, ()> {
        let guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        unsafe {
            env::remove_var("RUPTURE_PORT");
            env::remove_var("RUPTURE_HOST");
            env::remove_var("RUPTURE_MODELS_FILENAME");
            env::remove_var(SETTINGS_FILE_NAME_ENV);
        }
        guard
    }

    #[test]
    fn loads_values_from_file() {
        let _env = lock_env();
        let settings = Settings::load_from("rupture.example.toml").unwrap();

        assert_eq!(settings.models_filename, "models.json");
        assert_eq!(settings.host, "127.0.0.1");
        assert_eq!(settings.port, "8080");
    }

    #[test]
    fn keys_missing_from_file_fall_back_to_defaults() {
        let _env = lock_env();
        let dir = env::temp_dir().join("rupture_partial_settings.toml");
        fs::write(&dir, "[settings]\nport = \"9000\"\n").unwrap();

        let settings = Settings::load_from(dir.to_str().unwrap()).unwrap();

        assert_eq!(settings.models_filename, "models.toml"); 
        assert_eq!(settings.host, "127.0.0.1");
        assert_eq!(settings.port, "9000"); 
    }

    #[test]
    fn missing_file_falls_back_to_defaults() {
        let _env = lock_env();
        let settings = Settings::load_from("no_such_settings_file.toml").unwrap();

        assert_eq!(settings.models_filename, "models.toml");
        assert_eq!(settings.host, "127.0.0.1");
        assert_eq!(settings.port, "3000");
    }

    #[test]
    fn env_var_overrides_file_value() {
        let _env = lock_env();
        unsafe { env::set_var("RUPTURE_PORT", "7777") };

        let settings = Settings::load_from("rupture.example.toml").unwrap();

        assert_eq!(settings.port, "7777");
        assert_eq!(settings.host, "127.0.0.1"); // untouched
    }

    #[test]
    fn new_uses_env_var_path_for_the_settings_file() {
        let _env = lock_env();
        let dir = env::temp_dir().join("rupture_new_path_settings.toml");
        fs::write(&dir, "[settings]\nport = \"9090\"\n").unwrap();
        unsafe { env::set_var(SETTINGS_FILE_NAME_ENV, dir.to_str().unwrap()) };

        let settings = Settings::new().unwrap();

        assert_eq!(settings.port, "9090");
    }

    #[test]
    fn new_creates_settings_file_when_missing() {
        let _env = lock_env();
        let dir = env::temp_dir().join("rupture_generated_settings.toml");
        let _ = fs::remove_file(&dir); // ensure a clean slate
        unsafe { env::set_var(SETTINGS_FILE_NAME_ENV, dir.to_str().unwrap()) };

        let settings = Settings::new().unwrap();

        // generation fell back to defaults, and the file now exists on disk
        assert_eq!(settings.models_filename, "models.toml");
        assert_eq!(settings.host, "127.0.0.1");
        assert_eq!(settings.port, "3000");
        assert!(dir.exists(), "settings file was not generated");
    }

    #[test]
    fn generated_file_roundtrips_through_load() {
        let _env = lock_env();
        let dir = env::temp_dir().join("rupture_roundtrip_settings.toml");
        let _ = fs::remove_file(&dir);
        unsafe { env::set_var(SETTINGS_FILE_NAME_ENV, dir.to_str().unwrap()) };

        Settings::new().unwrap(); // generates the file

        // a user edits a value in the generated file, then restarts rupture
        let edited = fs::read_to_string(&dir).unwrap().replace("3000", "9999");
        fs::write(&dir, &edited).unwrap();

        let settings = Settings::new().unwrap(); // reads, does not regenerate

        assert_eq!(settings.port, "9999");
        let after = fs::read_to_string(&dir).unwrap();
        assert_eq!(after, edited, "file was rewritten; user edits were lost");
    }
    #[test]
    fn malformed_file_is_an_error() {
        let _env = lock_env();
        let dir = env::temp_dir().join("rupture_bad_settings.toml");
        fs::write(&dir, "not [valid toml").unwrap();

        assert!(Settings::load_from(dir.to_str().unwrap()).is_err());
    }
}
