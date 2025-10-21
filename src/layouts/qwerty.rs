use crate::layouts::Layout;
use evdev::KeyCode as K;

pub(crate) struct QwertyLayout;

impl Layout for QwertyLayout {
    fn to(&self, key: &K) -> K {
        *key
    }

    fn from(&self, key: &K) -> K {
        *key
    }
}
