mod dvorak;
mod qwerty;

use evdev::KeyCode;

/// Used for mapping layout definitions
pub(crate) trait Layout {
    /// Convert a Qwerty key to the layout’s key
    fn to(&self, key: &KeyCode) -> KeyCode;

    /// Convert a layout-specific key back to Qwerty
    fn from(&self, key: &KeyCode) -> KeyCode;
}

pub(crate) fn get(layout: &Option<String>) -> Box<dyn Layout> {
    match layout.as_deref().map(str::to_lowercase).as_deref() {
        Some("dvorak") => Box::new(dvorak::DvorakLayout),
        Some("qwerty") => Box::new(qwerty::QwertyLayout),
        _ => Box::new(qwerty::QwertyLayout),
    }
}
