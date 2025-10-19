pub mod layers;
pub mod overlaps;
pub mod terms;

use crate::config::KeyboardConfig;
use anyhow::Result;
use evdev::KeyCode;
use std::collections::HashSet;

#[derive(Clone, Copy, Debug)]
pub struct KeyEvent {
    pub key: KeyCode,
    pub state: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum OutputEvent {
    #[allow(dead_code)]
    Press(KeyCode),
    #[allow(dead_code)]
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
    pub device_config: &'a KeyboardConfig,
    pub keys_down: &'a mut HashSet<KeyCode>,
    pub active_layers: &'a mut HashSet<String>,
    pub no_emit: bool,
    pub global_term: u16,
}

pub trait Feature {
    fn name(&self) -> &'static str;

    fn on_event(&mut self, event: KeyEvent, ctx: &mut Context) -> Result<FeatureResult>;

    fn on_timer(&mut self, _key: KeyCode, _ctx: &mut Context) -> Result<Option<Vec<OutputEvent>>> {
        Ok(None)
    }
}
