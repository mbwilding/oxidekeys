use evdev::KeyCode;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::Instant};

fn default_keyboards() -> HashMap<String, HashMap<KeyCode, RemapAction>> {
    HashMap::from([(
        "AT Translated Set 2 keyboard".to_string(),
        [
            (
                KeyCode::KEY_SPACE,
                RemapAction {
                    tap: Some(KeyCode::KEY_SPACE),
                    hold: Some(KeyCode::KEY_LEFTSHIFT),
                    ..Default::default()
                },
            ),
            (
                KeyCode::KEY_LEFTSHIFT,
                RemapAction {
                    tap: Some(KeyCode::KEY_ESC),
                    hold: Some(KeyCode::KEY_LEFTMETA),
                    ..Default::default()
                },
            ),
            (
                KeyCode::KEY_CAPSLOCK,
                RemapAction {
                    tap: Some(KeyCode::KEY_BACKSPACE),
                    hold: Some(KeyCode::KEY_LEFTCTRL),
                    ..Default::default()
                },
            ),
            #[allow(clippy::needless_update)]
            (
                KeyCode::KEY_A,
                RemapAction {
                    tap: Some(KeyCode::KEY_A),
                    hold: Some(KeyCode::KEY_LEFTCTRL),
                    hrm: Some(true),
                    hrm_term: Some(144),
                    ..Default::default()
                },
            ),
        ]
        .into_iter()
        .collect::<HashMap<KeyCode, RemapAction>>(),
    )])
}

fn default_no_emit() -> bool {
    false
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Config {
    #[serde(default = "default_keyboards")]
    pub keyboards: HashMap<String, HashMap<KeyCode, RemapAction>>,
    #[serde(default = "default_no_emit")]
    pub no_emit: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            keyboards: default_keyboards(),
            no_emit: default_no_emit(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) struct RemapAction {
    /// Tap key
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tap: Option<KeyCode>,
    /// Hold key
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hold: Option<KeyCode>,
    /// Homerow Mod
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hrm: Option<bool>,
    /// Homerow Mod Term
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hrm_term: Option<u16>,
}

impl Default for RemapAction {
    fn default() -> Self {
        Self {
            tap: Some(KeyCode::KEY_RESERVED),
            hold: None,
            hrm: None,
            hrm_term: None,
        }
    }
}

pub(crate) struct PendingKey {
    pub remap: RemapAction,
    pub hold_sent: bool,
    pub time_pressed: Instant,
}
