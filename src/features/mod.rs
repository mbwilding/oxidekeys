pub mod hrm;
pub mod layers;
pub mod overlaps;

use crate::config::{Config, KeyboardConfig};
use crate::state::Pending;
use anyhow::Result;
use evdev::KeyCode;
use std::collections::HashSet;
use std::time::Instant;

#[derive(Clone, Copy, Debug)]
pub struct KeyEvent {
    pub key: KeyCode,
    pub state: i32,
}

#[derive(Clone, Debug)]
pub enum OutputEvent {
    Press(KeyCode),
    Release(KeyCode),
    PressMany(Vec<KeyCode>),
    ReleaseMany(Vec<KeyCode>),
}

pub enum FeatureResult {
    Continue(KeyEvent),
    Emit(Vec<OutputEvent>),
    Consume,
}

pub struct Context<'a> {
    pub config: &'a Config,
    pub device_config: &'a KeyboardConfig,
    pub pending: &'a mut Pending,
    pub keys_down: &'a mut HashSet<KeyCode>,
    pub active_layers: &'a mut HashSet<String>,
    pub now: Instant,
    pub no_emit: bool,
}

pub trait Feature {
    fn name(&self) -> &'static str;
    fn on_event(&mut self, event: KeyEvent, ctx: &mut Context) -> Result<FeatureResult>;
}
