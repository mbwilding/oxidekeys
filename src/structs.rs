use evdev::KeyCode;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

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
    /// Double tap key
    #[allow(dead_code)]
    pub double: Option<KeyCode>,
    /// If set, this sets how long the key needs to be pressed before triggering hold instead,
    /// Holds up tapping until the term has elapsed
    pub tapping_term: Option<u16>,
    /// If true, holding and pressing another key triggers hold
    pub overlap: bool,
}

impl RemapAction {
    pub fn tapping_term_duration(&self) -> Option<Duration> {
        self.tapping_term
            .map(|tapping_term| Duration::from_millis(tapping_term as u64))
    }
}

impl Default for RemapAction {
    fn default() -> Self {
        Self {
            tap: KeyCode::KEY_RESERVED,
            hold: None,
            double: None,
            tapping_term: None,
            overlap: false,
        }
    }
}

pub(crate) struct PendingKey {
    pub start: Instant,
    pub remap: RemapAction,
    pub hold_sent: bool,
}
