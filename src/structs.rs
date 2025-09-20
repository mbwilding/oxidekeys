use evdev::KeyCode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

fn default_no_emit() -> bool {
    false
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Config {
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
