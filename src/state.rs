use crate::config::RemapAction;
use evdev::KeyCode;
use std::collections::HashMap;
use std::time::Instant;

pub type Pending = HashMap<KeyCode, PendingKey>;

#[derive(Clone)]
pub struct PendingKey {
    pub remap: RemapAction,
    pub hold_sent: bool,
    pub overlap_hold_sent: bool,
    pub tap_sent: bool,
    pub time_pressed: Instant,
    pub timer_fired: bool,
}

pub fn add_pending(pending: &mut Pending, key: KeyCode, remap: &RemapAction) {
    pending.entry(key).or_insert(PendingKey {
        remap: remap.clone(),
        hold_sent: false,
        time_pressed: Instant::now(),
        timer_fired: false,
        overlap_hold_sent: false,
        tap_sent: false,
    });
}

pub fn remove_pending(pending: &mut Pending, key: &KeyCode) -> Option<PendingKey> {
    pending.remove(key)
}
