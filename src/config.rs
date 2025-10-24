use anyhow::Result;
use evdev::KeyCode;
use log::{info, trace};
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

    trace!("Config: {:#?}", config);

    Ok(config)
}

pub(crate) type Keyboards = HashMap<String, KeyboardConfig>;
pub(crate) type Mappings = HashMap<KeyCode, RemapAction>;
pub(crate) type Layers = HashMap<String, HashMap<KeyCode, HashMap<KeyCode, Vec<KeyCode>>>>;
pub(crate) type Features = HashMap<String, bool>;

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
                hold: Some(vec![KeyCode::KEY_LEFTMETA]),
                ..Default::default()
            },
        ),
        #[allow(clippy::needless_update)]
        (
            KeyCode::KEY_CAPSLOCK,
            RemapAction {
                tap: Some(vec![KeyCode::KEY_BACKSPACE]),
                hold: Some(vec![KeyCode::KEY_LEFTCTRL]),
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
    HashMap::from([(
        "Symbols & Navigation".into(),
        HashMap::from([(
            KeyCode::KEY_RIGHTALT,
            HashMap::from([
                // Vim Arrows
                (KeyCode::KEY_H, vec![KeyCode::KEY_LEFT]),
                (KeyCode::KEY_J, vec![KeyCode::KEY_DOWN]),
                (KeyCode::KEY_K, vec![KeyCode::KEY_UP]),
                (KeyCode::KEY_L, vec![KeyCode::KEY_RIGHT]),
                // (
                (KeyCode::KEY_I, vec![KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_9]),
                // )
                (KeyCode::KEY_D, vec![KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_0]),
                // {
                (
                    KeyCode::KEY_X,
                    vec![KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_LEFTBRACE],
                ),
                // }
                (
                    KeyCode::KEY_B,
                    vec![KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_RIGHTBRACE],
                ),
                // [
                (KeyCode::KEY_Y, vec![KeyCode::KEY_LEFTBRACE]),
                // ]
                (KeyCode::KEY_F, vec![KeyCode::KEY_RIGHTBRACE]),
                // /
                (KeyCode::KEY_SEMICOLON, vec![KeyCode::KEY_SLASH]),
                // \
                (KeyCode::KEY_Z, vec![KeyCode::KEY_BACKSLASH]),
                // `
                (KeyCode::KEY_APOSTROPHE, vec![KeyCode::KEY_GRAVE]),
                // !
                (
                    KeyCode::KEY_COMMA,
                    vec![KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_1],
                ),
                // ?
                (
                    KeyCode::KEY_DOT,
                    vec![KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_SLASH],
                ),
                // @
                (KeyCode::KEY_P, vec![KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_2]),
                // =
                (KeyCode::KEY_A, vec![KeyCode::KEY_EQUAL]),
                // |
                (
                    KeyCode::KEY_O,
                    vec![KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_BACKSLASH],
                ),
                // ^
                (KeyCode::KEY_E, vec![KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_6]),
                // _
                (
                    KeyCode::KEY_U,
                    vec![KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_MINUS],
                ),
                // #
                (KeyCode::KEY_Q, vec![KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_3]),
                // $
                (KeyCode::KEY_T, vec![KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_4]),
                // &
                (KeyCode::KEY_N, vec![KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_7]),
                // -
                (KeyCode::KEY_S, vec![KeyCode::KEY_MINUS]),
                // +
                (
                    KeyCode::KEY_M,
                    vec![KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_EQUAL],
                ),
                // %
                (KeyCode::KEY_W, vec![KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_5]),
                // *
                (KeyCode::KEY_V, vec![KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_8]),
                // ~
                (
                    KeyCode::KEY_G,
                    vec![KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_GRAVE],
                ),
            ]),
        )]),
        "Numbers".into(),
        HashMap::from([(
            // NOTE: CoPilot Key
            KeyCode::KEY_F23,
            HashMap::from([
                // Vim Arrows
                (KeyCode::KEY_H, vec![KeyCode::KEY_LEFT]),
                (KeyCode::KEY_J, vec![KeyCode::KEY_DOWN]),
                (KeyCode::KEY_K, vec![KeyCode::KEY_UP]),
                (KeyCode::KEY_L, vec![KeyCode::KEY_RIGHT]),
                // (
                (KeyCode::KEY_I, vec![KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_9]),
                // )
                (KeyCode::KEY_D, vec![KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_0]),
                // {
                (
                    KeyCode::KEY_X,
                    vec![KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_LEFTBRACE],
                ),
                // }
                (
                    KeyCode::KEY_B,
                    vec![KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_RIGHTBRACE],
                ),
                // [
                (KeyCode::KEY_Y, vec![KeyCode::KEY_LEFTBRACE]),
                // ]
                (KeyCode::KEY_F, vec![KeyCode::KEY_RIGHTBRACE]),
                // /
                (KeyCode::KEY_SEMICOLON, vec![KeyCode::KEY_SLASH]),
                // \
                (KeyCode::KEY_Z, vec![KeyCode::KEY_BACKSLASH]),
                // `
                (KeyCode::KEY_APOSTROPHE, vec![KeyCode::KEY_GRAVE]),
                // !
                (
                    KeyCode::KEY_COMMA,
                    vec![KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_1],
                ),
                // ?
                (
                    KeyCode::KEY_DOT,
                    vec![KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_SLASH],
                ),
                // @
                (KeyCode::KEY_P, vec![KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_2]),
                // =
                (KeyCode::KEY_A, vec![KeyCode::KEY_EQUAL]),
                // |
                (
                    KeyCode::KEY_O,
                    vec![KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_BACKSLASH],
                ),
                // ^
                (KeyCode::KEY_E, vec![KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_6]),
                // _
                (
                    KeyCode::KEY_U,
                    vec![KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_MINUS],
                ),
                // #
                (KeyCode::KEY_Q, vec![KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_3]),
                // $
                (KeyCode::KEY_T, vec![KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_4]),
                // &
                (KeyCode::KEY_N, vec![KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_7]),
                // -
                (KeyCode::KEY_S, vec![KeyCode::KEY_MINUS]),
                // +
                (
                    KeyCode::KEY_M,
                    vec![KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_EQUAL],
                ),
                // %
                (KeyCode::KEY_W, vec![KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_5]),
                // *
                (KeyCode::KEY_V, vec![KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_8]),
                // ~
                (
                    KeyCode::KEY_G,
                    vec![KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_GRAVE],
                ),
            ]),
        )]),
    )])
}

fn default_keyboards() -> Keyboards {
    HashMap::from([(
        "AT Translated Set 2 keyboard".to_owned(),
        KeyboardConfig {
            layout: default_layout(),
            mappings: default_mappings(),
            layers: default_layers(),
            double_tap_timeout: default_double_tap_timeout(),
        },
    )])
}

fn default_layout() -> Option<String> {
    Some("dvorak".to_string())
}

fn default_double_tap_timeout() -> Option<u16> {
    Some(144)
}

fn default_features() -> Features {
    HashMap::from([
        ("dual_function".to_owned(), true),
        ("layers".to_owned(), true),
    ])
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Config {
    #[serde(default = "default_features")]
    pub features: HashMap<String, bool>,
    #[serde(default = "default_keyboards")]
    pub keyboards: Keyboards,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct KeyboardConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<String>,
    #[serde(default = "default_mappings")]
    pub mappings: Mappings,
    #[serde(default = "default_layers")]
    pub layers: Layers,
    #[serde(default = "default_double_tap_timeout")]
    pub double_tap_timeout: Option<u16>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            features: default_features(),
            keyboards: default_keyboards(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct RemapAction {
    /// Tap sequence
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tap: Option<Vec<KeyCode>>,

    /// Hold sequence
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hold: Option<Vec<KeyCode>>,
}
