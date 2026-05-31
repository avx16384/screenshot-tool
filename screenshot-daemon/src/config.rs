use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub save_dir: PathBuf,
    pub controlbar_draggable: bool,
}

#[derive(Debug, Default, serde::Deserialize)]
struct TomlConfig {
    save_dir: Option<PathBuf>,
    controlbar_draggable: Option<bool>,
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let default_save_dir = dirs::picture_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("screenshots");

        let path = config_path();
        let toml_config = match std::fs::read_to_string(&path) {
            Ok(content) => toml::from_str::<TomlConfig>(&content).map_err(|error| {
                anyhow::anyhow!("parse config {} failed: {error}", path.display())
            })?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => TomlConfig::default(),
            Err(error) => {
                return Err(anyhow::anyhow!(
                    "read config {} failed: {error}",
                    path.display()
                ));
            }
        };

        Ok(Self {
            save_dir: toml_config.save_dir.unwrap_or(default_save_dir),
            controlbar_draggable: toml_config.controlbar_draggable.unwrap_or(true),
        })
    }
}

fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("screenshot-daemon")
        .join("config.toml")
}
