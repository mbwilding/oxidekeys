mod dvorak;

use evdev::KeyCode;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum Layout {
    #[default]
    Dvorak,
    Qwerty,
}

impl FromStr for Layout {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "dvorak" => Ok(Layout::Dvorak),
            "qwerty" => Ok(Layout::Qwerty),
            _ => Err(format!("invalid layout: {}", s)),
        }
    }
}

impl Layout {
    pub fn resolve(self, key: &KeyCode) -> KeyCode {
        match self {
            Layout::Dvorak => dvorak::resolve(key),
            Layout::Qwerty => *key,
        }
    }

    pub fn resolve_reverse(self, key: &KeyCode) -> KeyCode {
        match self {
            Layout::Dvorak => dvorak::resolve_reverse(key),
            Layout::Qwerty => *key,
        }
    }
}
