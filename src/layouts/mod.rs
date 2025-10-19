use evdev::KeyCode;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

mod dvorak;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Layout {
    Dvorak,
    Qwerty,
}

impl Default for Layout {
    fn default() -> Self {
        Layout::Dvorak
    }
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

impl ToString for Layout {
    fn to_string(&self) -> String {
        match self {
            Layout::Dvorak => "dvorak",
            Layout::Qwerty => "qwerty",
        }
        .to_string()
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
