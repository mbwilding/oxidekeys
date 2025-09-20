use evdev::KeyCode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

fn default_keyboard() -> String {
    "AT Translated Set 2 keyboard".to_string()
}

fn default_remaps() -> HashMap<KeyCode, RemapAction> {
    [
        (
            KeyCode::KEY_SPACE,
            RemapAction {
                tap: KeyCode::KEY_SPACE,
                hold: Some(KeyCode::KEY_LEFTSHIFT),
            },
        ),
        (
            KeyCode::KEY_LEFTSHIFT,
            RemapAction {
                tap: KeyCode::KEY_ESC,
                hold: Some(KeyCode::KEY_LEFTMETA),
            },
        ),
        (
            KeyCode::KEY_CAPSLOCK,
            RemapAction {
                tap: KeyCode::KEY_BACKSPACE,
                hold: Some(KeyCode::KEY_LEFTCTRL),
            },
        ),
    ]
    .into_iter()
    .collect()
}

fn default_no_emit() -> bool {
    false
}

// AT Translated Set 2 keyboard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Config {
    #[serde(default = "default_keyboard")]
    pub keyboard: String,
    #[serde(default = "default_remaps")]
    pub remaps: HashMap<KeyCode, RemapAction>,
    #[serde(default = "default_no_emit")]
    pub no_emit: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) struct RemapAction {
    /// Tap key
    pub tap: KeyCode,
    /// Hold key
    pub hold: Option<KeyCode>,
}

impl Default for RemapAction {
    fn default() -> Self {
        Self {
            tap: KeyCode::KEY_RESERVED,
            hold: None,
        }
    }
}

pub(crate) struct PendingKey {
    pub remap: RemapAction,
    pub hold_sent: bool,
}
