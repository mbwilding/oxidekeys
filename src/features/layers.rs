use crate::{
    config::Layers,
    consts::*,
    features::{Context, Feature, FeatureResult, KeyEvent, OutputEvent},
};
use anyhow::Result;
use evdev::KeyCode;
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

        let remapped =
            resolve_layered_keys(event.key, ctx.active_layers, &ctx.device_config.layers);
        if remapped.len() == 1 && remapped[0] == event.key {
            return Ok(FeatureResult::Continue(event));
        }

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
) -> Vec<KeyCode> {
    for layer in active_layers {
        if let Some(layer_map) = layers.get(layer) {
            for mapping in layer_map.values() {
                if let Some(remapped) = mapping.get(&key) {
                    return remapped.clone();
                }
            }
        }
    }

    vec![key]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::tests::{TestContext, press, release};
    use std::collections::HashMap;

    fn create_test_layers() -> Layers {
        let mut layers = HashMap::new();
        let mut layer_map = HashMap::new();
        let mut key_map = HashMap::new();

        // Create a simple layer mapping: A -> B when LEFTSHIFT is held
        key_map.insert(KeyCode::KEY_A, vec![KeyCode::KEY_B]);
        layer_map.insert(KeyCode::KEY_LEFTSHIFT, key_map);
        layers.insert("test_layer".to_string(), layer_map);

        layers
    }

    #[test]
    fn test_layer_activation_on_press() {
        let layers = create_test_layers();
        let mut test_ctx = TestContext::with_layers(layers);
        let mut feature = LayersFeature::new();

        // Press the layer trigger key (LEFTSHIFT)
        let event = press(KeyCode::KEY_LEFTSHIFT);
        let result = feature.on_event(event, &mut test_ctx.as_context()).unwrap();

        // Should consume the event and activate the layer
        match result {
            FeatureResult::Consume => {}
            _ => panic!("Expected Consume result"),
        }

        // Check that the layer is active and the key is tracked
        assert!(test_ctx.active_layers.contains("test_layer"));
        assert!(test_ctx.keys_down.contains(&KeyCode::KEY_LEFTSHIFT));
    }

    #[test]
    fn test_layer_deactivation_on_release() {
        let layers = create_test_layers();
        let mut test_ctx = TestContext::with_layers(layers);
        let mut feature = LayersFeature::new();

        // First activate the layer
        test_ctx.active_layers.insert("test_layer".to_string());
        test_ctx.keys_down.insert(KeyCode::KEY_LEFTSHIFT);

        // Release the layer trigger key
        let event = release(KeyCode::KEY_LEFTSHIFT);
        let result = feature.on_event(event, &mut test_ctx.as_context()).unwrap();

        // Should consume the event and deactivate the layer
        match result {
            FeatureResult::Consume => {}
            _ => panic!("Expected Consume result"),
        }

        // Check that the layer is no longer active and the key is not tracked
        assert!(!test_ctx.active_layers.contains("test_layer"));
        assert!(!test_ctx.keys_down.contains(&KeyCode::KEY_LEFTSHIFT));
    }

    #[test]
    fn test_key_remapping_with_active_layer() {
        let layers = create_test_layers();
        let mut test_ctx = TestContext::with_layers(layers);
        let mut feature = LayersFeature::new();

        // Activate the layer
        test_ctx.active_layers.insert("test_layer".to_string());

        // Press a key that should be remapped (A -> B)
        let event = press(KeyCode::KEY_A);
        let result = feature.on_event(event, &mut test_ctx.as_context()).unwrap();

        // Should emit the remapped key
        match result {
            FeatureResult::Emit(events) => {
                assert_eq!(events.len(), 1);
                match &events[0] {
                    OutputEvent::PressMany(keys) => {
                        assert_eq!(keys.len(), 1);
                        assert_eq!(keys[0], KeyCode::KEY_B);
                    }
                    _ => panic!("Expected PressMany with remapped key"),
                }
            }
            _ => panic!("Expected Emit result"),
        }
    }

    #[test]
    fn test_key_remapping_with_inactive_layer() {
        let layers = create_test_layers();
        let mut test_ctx = TestContext::with_layers(layers);
        let mut feature = LayersFeature::new();

        // Don't activate the layer

        // Press a key that would be remapped if layer was active
        let event = press(KeyCode::KEY_A);
        let result = feature.on_event(event, &mut test_ctx.as_context()).unwrap();

        // Should continue with the original key (no remapping)
        match result {
            FeatureResult::Continue(continued_event) => {
                assert_eq!(continued_event.key, KeyCode::KEY_A);
            }
            _ => panic!("Expected Continue result"),
        }
    }

    #[test]
    fn test_key_release_with_active_layer() {
        let layers = create_test_layers();
        let mut test_ctx = TestContext::with_layers(layers);
        let mut feature = LayersFeature::new();

        // Activate the layer
        test_ctx.active_layers.insert("test_layer".to_string());

        // Release a key that should be remapped
        let event = release(KeyCode::KEY_A);
        let result = feature.on_event(event, &mut test_ctx.as_context()).unwrap();

        // Should emit the remapped key release
        match result {
            FeatureResult::Emit(events) => {
                assert_eq!(events.len(), 1);
                match &events[0] {
                    OutputEvent::ReleaseMany(keys) => {
                        assert_eq!(keys.len(), 1);
                        assert_eq!(keys[0], KeyCode::KEY_B);
                    }
                    _ => panic!("Expected ReleaseMany with remapped key"),
                }
            }
            _ => panic!("Expected Emit result"),
        }
    }

    #[test]
    fn test_multiple_active_layers() {
        let mut layers = HashMap::new();

        // Create two layers with different triggers
        let mut layer1_map = HashMap::new();
        let mut key1_map = HashMap::new();
        key1_map.insert(KeyCode::KEY_A, vec![KeyCode::KEY_B]);
        layer1_map.insert(KeyCode::KEY_LEFTSHIFT, key1_map);
        layers.insert("layer1".to_string(), layer1_map);

        let mut layer2_map = HashMap::new();
        let mut key2_map = HashMap::new();
        key2_map.insert(KeyCode::KEY_A, vec![KeyCode::KEY_C]);
        layer2_map.insert(KeyCode::KEY_RIGHTSHIFT, key2_map);
        layers.insert("layer2".to_string(), layer2_map);

        let mut test_ctx = TestContext::with_layers(layers);
        let mut feature = LayersFeature::new();

        // Activate both layers
        test_ctx.active_layers.insert("layer1".to_string());
        test_ctx.active_layers.insert("layer2".to_string());

        // Press a key that exists in both layers
        let event = press(KeyCode::KEY_A);
        let result = feature.on_event(event, &mut test_ctx.as_context()).unwrap();

        // Should use one of the layers (either B or C, depending on iteration order)
        match result {
            FeatureResult::Emit(events) => {
                assert_eq!(events.len(), 1);
                match &events[0] {
                    OutputEvent::PressMany(keys) => {
                        assert_eq!(keys.len(), 1);
                        // The key should be either B or C (from layer1 or layer2)
                        assert!(keys[0] == KeyCode::KEY_B || keys[0] == KeyCode::KEY_C);
                    }
                    _ => panic!("Expected PressMany with remapped key"),
                }
            }
            _ => panic!("Expected Emit result"),
        }
    }

    #[test]
    fn test_non_layer_key_passthrough() {
        let layers = create_test_layers();
        let mut test_ctx = TestContext::with_layers(layers);
        let mut feature = LayersFeature::new();

        // Press a key that's not a layer trigger and not in any layer
        let event = press(KeyCode::KEY_Z);
        let result = feature.on_event(event, &mut test_ctx.as_context()).unwrap();

        // Should continue with the original key
        match result {
            FeatureResult::Continue(continued_event) => {
                assert_eq!(continued_event.key, KeyCode::KEY_Z);
            }
            _ => panic!("Expected Continue result"),
        }
    }

    #[test]
    fn test_resolve_layered_keys_no_active_layers() {
        let layers = create_test_layers();
        let active_layers = HashSet::new();

        let result = resolve_layered_keys(KeyCode::KEY_A, &active_layers, &layers);
        assert_eq!(result, vec![KeyCode::KEY_A]);
    }

    #[test]
    fn test_resolve_layered_keys_key_not_in_layer() {
        let layers = create_test_layers();
        let mut active_layers = HashSet::new();
        active_layers.insert("test_layer".to_string());

        let result = resolve_layered_keys(KeyCode::KEY_Z, &active_layers, &layers);
        assert_eq!(result, vec![KeyCode::KEY_Z]);
    }

    #[test]
    fn test_resolve_layered_keys_key_in_layer() {
        let layers = create_test_layers();
        let mut active_layers = HashSet::new();
        active_layers.insert("test_layer".to_string());

        let result = resolve_layered_keys(KeyCode::KEY_A, &active_layers, &layers);
        assert_eq!(result, vec![KeyCode::KEY_B]);
    }

    #[test]
    fn test_complex_key_sequence_remapping() {
        let mut layers = HashMap::new();
        let mut layer_map = HashMap::new();
        let mut key_map = HashMap::new();

        // Map A to a sequence of keys (Ctrl+C)
        key_map.insert(KeyCode::KEY_A, vec![KeyCode::KEY_LEFTCTRL, KeyCode::KEY_C]);
        layer_map.insert(KeyCode::KEY_LEFTSHIFT, key_map);
        layers.insert("test_layer".to_string(), layer_map);

        let mut test_ctx = TestContext::with_layers(layers);
        let mut feature = LayersFeature::new();

        // Activate the layer
        test_ctx.active_layers.insert("test_layer".to_string());

        // Press A
        let event = press(KeyCode::KEY_A);
        let result = feature.on_event(event, &mut test_ctx.as_context()).unwrap();

        // Should emit the key sequence
        match result {
            FeatureResult::Emit(events) => {
                assert_eq!(events.len(), 1);
                match &events[0] {
                    OutputEvent::PressMany(keys) => {
                        assert_eq!(keys.len(), 2);
                        assert_eq!(keys[0], KeyCode::KEY_LEFTCTRL);
                        assert_eq!(keys[1], KeyCode::KEY_C);
                    }
                    _ => panic!("Expected PressMany with key sequence"),
                }
            }
            _ => panic!("Expected Emit result"),
        }
    }
}
