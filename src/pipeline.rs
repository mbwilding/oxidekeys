use crate::config::{Config, KeyboardConfig};
use crate::features::{Context, Feature, FeatureResult, KeyEvent};
use crate::io::{emit, emit_passthrough};
use crate::state::Pending;
use anyhow::Result;
use evdev::KeyCode;
use std::collections::HashSet;
use std::time::Instant;
use uinput::device::Device as UInputDevice;

pub struct Pipeline {
    features: Vec<Box<dyn Feature + Send>>,
}

impl Pipeline {
    pub fn new(features: Vec<Box<dyn Feature + Send>>) -> Self {
        Self { features }
    }

    pub fn process_event(
        &mut self,
        virt: &mut UInputDevice,
        cfg: &Config,
        kcfg: &KeyboardConfig,
        pending: &mut Pending,
        keys_down: &mut HashSet<KeyCode>,
        active_layers: &mut HashSet<String>,
        key: KeyCode,
        state: i32,
    ) -> Result<()> {
        let now = Instant::now();
        let mut ctx = Context {
            config: cfg,
            device_config: kcfg,
            pending,
            keys_down,
            active_layers,
            now,
            no_emit: cfg.globals.no_emit,
        };

        let mut action = FeatureResult::Continue(KeyEvent {
            key,
            state,
            ts: now,
        });
        for f in self.features.iter_mut() {
            action = match action {
                FeatureResult::Continue(e) => f.on_event(e, &mut ctx)?,
                _ => action,
            };
            if !matches!(action, FeatureResult::Continue(_)) {
                break;
            }
        }

        match action {
            FeatureResult::Continue(e) => emit_passthrough(virt, e.key, e.state, ctx.no_emit),
            FeatureResult::Emit(out) => emit(virt, out, ctx.no_emit),
            FeatureResult::Consume => Ok(()),
        }
    }
}
