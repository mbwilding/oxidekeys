use evdev::KeyCode;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::Instant};

fn default_no_emit() -> bool {
    false
}

fn default_hrm_term() -> u16 {
    130
}

fn default_keyboards() -> HashMap<String, HashMap<KeyCode, RemapAction>> {
    HashMap::from([(
        "AT Translated Set 2 keyboard".to_string(),
        [
            #[allow(clippy::needless_update)]
            (
                KeyCode::KEY_SPACE,
                RemapAction {
                    tap: Some(KeyCode::KEY_SPACE),
                    hold: Some(KeyCode::KEY_LEFTSHIFT),
                    ..Default::default()
                },
            ),
            #[allow(clippy::needless_update)]
            (
                KeyCode::KEY_LEFTSHIFT,
                RemapAction {
                    tap: Some(KeyCode::KEY_ESC),
                    ..Default::default()
                },
            ),
            #[allow(clippy::needless_update)]
            (
                KeyCode::KEY_CAPSLOCK,
                RemapAction {
                    tap: Some(KeyCode::KEY_BACKSPACE),
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
                    ..Default::default()
                },
            ),
            #[allow(clippy::needless_update)]
            (
                KeyCode::KEY_SEMICOLON,
                RemapAction {
                    tap: Some(KeyCode::KEY_SEMICOLON),
                    hold: Some(KeyCode::KEY_RIGHTCTRL),
                    hrm: Some(true),
                    ..Default::default()
                },
            ),
            #[allow(clippy::needless_update)]
            (
                KeyCode::KEY_S,
                RemapAction {
                    tap: Some(KeyCode::KEY_S),
                    hold: Some(KeyCode::KEY_LEFTMETA),
                    hrm: Some(true),
                    ..Default::default()
                },
            ),
            #[allow(clippy::needless_update)]
            (
                KeyCode::KEY_L,
                RemapAction {
                    tap: Some(KeyCode::KEY_L),
                    hold: Some(KeyCode::KEY_RIGHTMETA),
                    hrm: Some(true),
                    ..Default::default()
                },
            ),
            #[allow(clippy::needless_update)]
            (
                KeyCode::KEY_D,
                RemapAction {
                    tap: Some(KeyCode::KEY_D),
                    hold: Some(KeyCode::KEY_LEFTALT),
                    hrm: Some(true),
                    ..Default::default()
                },
            ),
            #[allow(clippy::needless_update)]
            (
                KeyCode::KEY_K,
                RemapAction {
                    tap: Some(KeyCode::KEY_K),
                    hold: Some(KeyCode::KEY_RIGHTALT),
                    hrm: Some(true),
                    ..Default::default()
                },
            ),
        ]
        .into_iter()
        .collect::<HashMap<KeyCode, RemapAction>>(),
    )])
}

fn default_layers() -> HashMap<String, HashMap<KeyCode, HashMap<KeyCode, Vec<KeyCode>>>> {
    HashMap::from([
        (
            "Navigation".to_string(),
            HashMap::from([(
                KeyCode::KEY_RIGHTALT,
                // NOTE: Dvorak
                HashMap::from([
                    (KeyCode::KEY_J, vec![KeyCode::KEY_LEFT]),
                    (KeyCode::KEY_C, vec![KeyCode::KEY_DOWN]),
                    (KeyCode::KEY_V, vec![KeyCode::KEY_UP]),
                    (KeyCode::KEY_P, vec![KeyCode::KEY_RIGHT]),
                ]),
            )]),
        ),
        (
            "Symbols".to_string(),
            HashMap::from([(
                KeyCode::KEY_LEFTALT,
                // NOTE: Dvorak
                HashMap::from([
                    // ()
                    (
                        KeyCode::KEY_F,
                        vec![KeyCode::KEY_RIGHTSHIFT, KeyCode::KEY_9],
                    ),
                    (
                        KeyCode::KEY_J,
                        vec![KeyCode::KEY_RIGHTSHIFT, KeyCode::KEY_0],
                    ),
                    // {}
                    (
                        KeyCode::KEY_D,
                        vec![KeyCode::KEY_RIGHTSHIFT, KeyCode::KEY_MINUS],
                    ),
                    (
                        KeyCode::KEY_K,
                        vec![KeyCode::KEY_RIGHTSHIFT, KeyCode::KEY_EQUAL],
                    ),
                    // []
                    (KeyCode::KEY_S, vec![KeyCode::KEY_MINUS]),
                    (KeyCode::KEY_L, vec![KeyCode::KEY_EQUAL]),
                    // <>
                    (
                        KeyCode::KEY_A,
                        vec![KeyCode::KEY_RIGHTSHIFT, KeyCode::KEY_W],
                    ),
                    (
                        KeyCode::KEY_SEMICOLON,
                        vec![KeyCode::KEY_RIGHTSHIFT, KeyCode::KEY_E],
                    ),
                ]),
            )]),
        ),
    ])
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Config {
    #[serde(default = "default_no_emit")]
    pub no_emit: bool,

    #[serde(default = "default_hrm_term")]
    pub hrm_term: u16,

    #[serde(default = "default_keyboards")]
    pub keyboards: HashMap<String, HashMap<KeyCode, RemapAction>>,

    #[serde(default = "default_layers")]
    pub layers: HashMap<String, HashMap<KeyCode, HashMap<KeyCode, Vec<KeyCode>>>>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            no_emit: default_no_emit(),
            hrm_term: default_hrm_term(),
            keyboards: default_keyboards(),
            layers: default_layers(),
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
