use crate::structs::*;
use anyhow::Result;
use log::{debug, info};
use std::path::PathBuf;
use std::{env, fs};

pub(crate) fn config() -> Result<Config> {
    let config_path = match env::args().nth(1) {
        Some(arg_path) => PathBuf::from(arg_path),
        None => dirs::config_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("~/.config"))
            .join("oxidekeys")
            .join("config.yml"),
    };

    let config = if !config_path.exists() {
        let config = Config::default();
        fs::create_dir_all(config_path.parent().unwrap())?;
        let config_yaml = serde_yaml::to_string(&config)?;
        fs::write(&config_path, config_yaml)?;
        info!("Default config written to {}", config_path.display());
        config
    } else {
        let config_content = fs::read_to_string(&config_path)?;
        serde_yaml::from_str(&config_content)?
    };

    debug!("Config: {:#?}", config);

    Ok(config)
}
