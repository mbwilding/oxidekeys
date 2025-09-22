use anyhow::Result;
use evdev::KeyCode;
use log::{debug, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
        let serialized = serde_yaml::to_string(&config)?;
        fs::write(&config_path, serialized)?;
        info!("Default config written to {}", config_path.display());
        config
    } else {
        let config_content = fs::read_to_string(&config_path)?;
        serde_yaml::from_str(&config_content)?
    };

    debug!("Config: {:#?}", config);

    Ok(config)
}

pub(crate) type Keyboards = HashMap<String, KeyboardConfig>;
pub(crate) type Mappings = HashMap<KeyCode, RemapAction>;
pub(crate) type Layers = HashMap<String, HashMap<KeyCode, HashMap<KeyCode, Vec<KeyCode>>>>;
pub(crate) type Plugins = HashMap<String, bool>;

fn default_no_emit() -> bool {
    false
}

fn default_hrm_term() -> u16 {
    144
}

fn default_mappings() -> Mappings {
    HashMap::from([
        #[allow(clippy::needless_update)]
        (
            KeyCode::KEY_SPACE,
            RemapAction {
                tap: Some(vec![KeyCode::KEY_SPACE]),
                hold: Some(vec![KeyCode::KEY_LEFTSHIFT]),
                ..Default::default()
            },
        ),
        #[allow(clippy::needless_update)]
        (
            KeyCode::KEY_LEFTSHIFT,
            RemapAction {
                tap: Some(vec![KeyCode::KEY_ESC]),
                ..Default::default()
            },
        ),
        #[allow(clippy::needless_update)]
        (
            KeyCode::KEY_CAPSLOCK,
            RemapAction {
                tap: Some(vec![KeyCode::KEY_BACKSPACE]),
                ..Default::default()
            },
        ),
        #[allow(clippy::needless_update)]
        (
            KeyCode::KEY_A,
            RemapAction {
                tap: Some(vec![KeyCode::KEY_A]),
                hold: Some(vec![KeyCode::KEY_LEFTCTRL]),
                hrm: Some(true),
                ..Default::default()
            },
        ),
        #[allow(clippy::needless_update)]
        (
            KeyCode::KEY_SEMICOLON,
            RemapAction {
                tap: Some(vec![KeyCode::KEY_SEMICOLON]),
                hold: Some(vec![KeyCode::KEY_RIGHTCTRL]),
                hrm: Some(true),
                ..Default::default()
            },
        ),
        #[allow(clippy::needless_update)]
        (
            KeyCode::KEY_S,
            RemapAction {
                tap: Some(vec![KeyCode::KEY_S]),
                hold: Some(vec![KeyCode::KEY_LEFTMETA]),
                hrm: Some(true),
                ..Default::default()
            },
        ),
        #[allow(clippy::needless_update)]
        (
            KeyCode::KEY_L,
            RemapAction {
                tap: Some(vec![KeyCode::KEY_L]),
                hold: Some(vec![KeyCode::KEY_RIGHTMETA]),
                hrm: Some(true),
                ..Default::default()
            },
        ),
        #[allow(clippy::needless_update)]
        (
            KeyCode::KEY_D,
            RemapAction {
                tap: Some(vec![KeyCode::KEY_D]),
                hold: Some(vec![KeyCode::KEY_LEFTALT]),
                hrm: Some(true),
                ..Default::default()
            },
        ),
        #[allow(clippy::needless_update)]
        (
            KeyCode::KEY_K,
            RemapAction {
                tap: Some(vec![KeyCode::KEY_K]),
                hold: Some(vec![KeyCode::KEY_RIGHTALT]),
                hrm: Some(true),
                ..Default::default()
            },
        ),
        #[allow(clippy::needless_update)]
        (
            KeyCode::KEY_BACKSPACE,
            RemapAction {
                ..Default::default()
            },
        ),
    ])
}

fn default_layers() -> Layers {
    HashMap::from([
        (
            "Navigation".into(),
            HashMap::from([(
                KeyCode::KEY_RIGHTALT,
                HashMap::from([
                    // Vim Arrows
                    (KeyCode::KEY_J, vec![KeyCode::KEY_LEFT]),
                    (KeyCode::KEY_C, vec![KeyCode::KEY_DOWN]),
                    (KeyCode::KEY_V, vec![KeyCode::KEY_UP]),
                    (KeyCode::KEY_P, vec![KeyCode::KEY_RIGHT]),
                ]),
            )]),
        ),
        (
            "Symbols".into(),
            HashMap::from([(
                KeyCode::KEY_LEFTALT,
                HashMap::from([
                    // (
                    (
                        KeyCode::KEY_F,
                        vec![KeyCode::KEY_RIGHTSHIFT, KeyCode::KEY_9],
                    ),
                    // )
                    (
                        KeyCode::KEY_J,
                        vec![KeyCode::KEY_RIGHTSHIFT, KeyCode::KEY_0],
                    ),
                    // {
                    (
                        KeyCode::KEY_D,
                        vec![KeyCode::KEY_RIGHTSHIFT, KeyCode::KEY_MINUS],
                    ),
                    // }
                    (
                        KeyCode::KEY_K,
                        vec![KeyCode::KEY_RIGHTSHIFT, KeyCode::KEY_EQUAL],
                    ),
                    // [
                    (KeyCode::KEY_S, vec![KeyCode::KEY_MINUS]),
                    // ]
                    (KeyCode::KEY_L, vec![KeyCode::KEY_EQUAL]),
                    // <
                    (
                        KeyCode::KEY_A,
                        vec![KeyCode::KEY_RIGHTSHIFT, KeyCode::KEY_W],
                    ),
                    // >
                    (
                        KeyCode::KEY_SEMICOLON,
                        vec![KeyCode::KEY_RIGHTSHIFT, KeyCode::KEY_E],
                    ),
                    // /
                    (KeyCode::KEY_G, vec![KeyCode::KEY_LEFTBRACE]),
                    // \
                    (KeyCode::KEY_H, vec![KeyCode::KEY_BACKSLASH]),
                ]),
            )]),
        ),
    ])
}

fn default_keyboards() -> Keyboards {
    HashMap::from([(
        "AT Translated Set 2 keyboard".to_owned(),
        KeyboardConfig {
            mappings: default_mappings(),
            layers: default_layers(),
        },
    )])
}

fn default_plugins() -> Plugins {
    HashMap::from([
        ("hrm".to_owned(), false),
        ("overlaps".to_owned(), false),
        ("layers".to_owned(), true),
    ])
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Config {
    pub globals: Globals,
    #[serde(default = "default_plugins")]
    pub plugins: HashMap<String, bool>,
    #[serde(default = "default_keyboards")]
    pub keyboards: Keyboards,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Globals {
    #[serde(default = "default_no_emit")]
    pub no_emit: bool,
    #[serde(default = "default_hrm_term")]
    pub hrm_term: u16,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct KeyboardConfig {
    #[serde(default = "default_mappings")]
    pub mappings: Mappings,
    #[serde(default = "default_layers")]
    pub layers: Layers,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            globals: Globals {
                no_emit: default_no_emit(),
                hrm_term: default_hrm_term(),
            },
            plugins: default_plugins(),
            keyboards: default_keyboards(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct RemapAction {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tap: Option<Vec<KeyCode>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hold: Option<Vec<KeyCode>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hrm: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hrm_term: Option<u16>,
}
