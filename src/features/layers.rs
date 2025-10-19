use crate::{
    config::Layers,
    consts::*,
    features::{Context, Feature, FeatureResult, KeyEvent, OutputEvent},
    layouts::Layout,
};
use anyhow::Result;
use evdev::KeyCode;
use log::debug;
use std::collections::HashSet;

pub struct LayersFeature;

impl LayersFeature {
    pub fn new() -> Self {
        Self
    }
}

impl Feature for LayersFeature {
    fn name(&self) -> &'static str {
        "layers"
    }

    fn on_event(&mut self, event: KeyEvent, ctx: &mut Context) -> Result<FeatureResult> {
        let mut is_layer_trigger = false;

        for (layer_name, layer_def) in &ctx.device_config.layers {
            if layer_def.contains_key(&event.key) {
                is_layer_trigger = true;
                match event.state {
                    PRESS => {
                        ctx.active_layers.insert(layer_name.clone());
                    }
                    RELEASE => {
                        ctx.active_layers.remove(layer_name);
                    }
                    _ => {}
                }
                break;
            }
        }

        if is_layer_trigger {
            match event.state {
                PRESS => {
                    ctx.keys_down.insert(event.key);
                }
                RELEASE => {
                    ctx.keys_down.remove(&event.key);
                }
                _ => {}
            }
            return Ok(FeatureResult::Consume);
        }

        let remapped = resolve_layered_keys(
            event.key,
            ctx.active_layers,
            &ctx.device_config.layers,
            &ctx.device_config.layout,
        );

        if remapped.len() == 1 && remapped[0] == ctx.device_config.layout.resolve_reverse(&event.key) {
            return Ok(FeatureResult::Continue(event));
        }

        debug!("{:#?}", &remapped);

        match event.state {
            PRESS => Ok(FeatureResult::Emit(vec![OutputEvent::PressMany(remapped)])),
            RELEASE => Ok(FeatureResult::Emit(vec![OutputEvent::ReleaseMany(
                remapped,
            )])),
            _ => Ok(FeatureResult::Consume),
        }
    }
}

fn resolve_layered_keys(
    key: KeyCode,
    active_layers: &HashSet<String>,
    layers: &Layers,
    layout: &Layout,
) -> Vec<KeyCode> {
    for layer in active_layers {
        if let Some(layer_map) = layers.get(layer) {
            for mapping in layer_map.values() {
                if let Some(remapped) = mapping.get(&key) {
                    let mut keys_reversed: Vec<KeyCode> = Vec::with_capacity(remapped.len());
                    for key in remapped {
                        let key_reversed = layout.resolve_reverse(key);
                        keys_reversed.push(key_reversed);
                    }
                    return keys_reversed;
                }
            }
        }
    }

    vec![key]
}
