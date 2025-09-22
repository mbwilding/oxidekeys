use crate::{
    config::{Config, KeyboardConfig},
    features::{Context, Feature, FeatureResult, KeyEvent},
    io::{emit, emit_passthrough},
};
use anyhow::Result;
use evdev::KeyCode;
use std::collections::HashSet;
use uinput::device::Device as UInputDevice;

pub struct Pipeline {
    features: Vec<Box<dyn Feature + Send>>,
}

impl Pipeline {
    pub fn new(features: Vec<Box<dyn Feature + Send>>) -> Self {
        Self { features }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn process_event(
        &mut self,
        virt: &mut UInputDevice,
        config: &Config,
        kb_config: &KeyboardConfig,
        keys_down: &mut HashSet<KeyCode>,
        active_layers: &mut HashSet<String>,
        key: KeyCode,
        state: i32,
    ) -> Result<()> {
        let mut ctx = Context {
            device_config: kb_config,
            keys_down,
            active_layers,
            no_emit: config.globals.no_emit,
        };

        let mut action = FeatureResult::Continue(KeyEvent { key, state });
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
