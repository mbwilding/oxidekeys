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
}

pub trait Feature {
    #[allow(dead_code)]
    fn name(&self) -> &'static str;

    fn on_event(&mut self, event: KeyEvent, ctx: &mut Context) -> Result<FeatureResult>;

    #[allow(dead_code)]
    fn on_timer(&mut self, _key: KeyCode, _ctx: &mut Context) -> Result<Option<Vec<OutputEvent>>> {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{KeyboardConfig, Layers, RemapAction};
    use std::collections::HashMap;

    /// Test utilities for creating mock contexts and configurations
    pub struct TestContext {
        pub device_config: KeyboardConfig,
        pub keys_down: HashSet<KeyCode>,
        pub active_layers: HashSet<String>,
        pub no_emit: bool,
    }

    impl TestContext {
        pub fn new() -> Self {
            Self {
                device_config: KeyboardConfig::default(),
                keys_down: HashSet::new(),
                active_layers: HashSet::new(),
                no_emit: false,
            }
        }

        pub fn with_mappings(mappings: HashMap<KeyCode, RemapAction>) -> Self {
            Self {
                device_config: KeyboardConfig {
                    mappings,
                    layers: HashMap::new(),
                },
                keys_down: HashSet::new(),
                active_layers: HashSet::new(),
                no_emit: false,
            }
        }

        pub fn with_layers(layers: Layers) -> Self {
            Self {
                device_config: KeyboardConfig {
                    mappings: HashMap::new(),
                    layers,
                },
                keys_down: HashSet::new(),
                active_layers: HashSet::new(),
                no_emit: false,
            }
        }

        pub fn as_context(&mut self) -> Context<'_> {
            Context {
                device_config: &self.device_config,
                keys_down: &mut self.keys_down,
                active_layers: &mut self.active_layers,
                no_emit: self.no_emit,
            }
        }
    }

    /// Helper function to create a key event
    pub fn key_event(key: KeyCode, state: i32) -> KeyEvent {
        KeyEvent { key, state }
    }

    /// Helper function to create a press event
    pub fn press(key: KeyCode) -> KeyEvent {
        key_event(key, crate::consts::PRESS)
    }

    /// Helper function to create a release event
    pub fn release(key: KeyCode) -> KeyEvent {
        key_event(key, crate::consts::RELEASE)
    }

    #[test]
    fn test_key_event_creation() {
        let event = key_event(KeyCode::KEY_A, crate::consts::PRESS);
        assert_eq!(event.key, KeyCode::KEY_A);
        assert_eq!(event.state, crate::consts::PRESS);
    }

    #[test]
    fn test_output_event_variants() {
        let press_event = OutputEvent::Press(KeyCode::KEY_A);
        let release_event = OutputEvent::Release(KeyCode::KEY_A);
        let press_many = OutputEvent::PressMany(vec![KeyCode::KEY_A, KeyCode::KEY_B]);
        let release_many = OutputEvent::ReleaseMany(vec![KeyCode::KEY_A, KeyCode::KEY_B]);

        // Test that all variants can be created
        match press_event {
            OutputEvent::Press(_) => {}
            _ => panic!("Expected Press variant"),
        }

        match release_event {
            OutputEvent::Release(_) => {}
            _ => panic!("Expected Release variant"),
        }

        match press_many {
            OutputEvent::PressMany(keys) => assert_eq!(keys.len(), 2),
            _ => panic!("Expected PressMany variant"),
        }

        match release_many {
            OutputEvent::ReleaseMany(keys) => assert_eq!(keys.len(), 2),
            _ => panic!("Expected ReleaseMany variant"),
        }
    }

    #[test]
    fn test_feature_result_variants() {
        let continue_result =
            FeatureResult::Continue(key_event(KeyCode::KEY_A, crate::consts::PRESS));
        let emit_result = FeatureResult::Emit(vec![OutputEvent::Press(KeyCode::KEY_A)]);
        let consume_result = FeatureResult::Consume;

        // Test that all variants can be created
        match continue_result {
            FeatureResult::Continue(event) => assert_eq!(event.key, KeyCode::KEY_A),
            _ => panic!("Expected Continue variant"),
        }

        match emit_result {
            FeatureResult::Emit(events) => assert_eq!(events.len(), 1),
            _ => panic!("Expected Emit variant"),
        }

        match consume_result {
            FeatureResult::Consume => {}
            _ => panic!("Expected Consume variant"),
        }
    }

    #[test]
    fn test_context_creation() {
        let mut test_ctx = TestContext::new();
        let ctx = test_ctx.as_context();

        assert_eq!(ctx.keys_down.len(), 0);
        assert_eq!(ctx.active_layers.len(), 0);
        assert_eq!(ctx.no_emit, false);
    }

    #[test]
    fn test_context_with_mappings() {
        let mut mappings = HashMap::new();
        mappings.insert(
            KeyCode::KEY_A,
            RemapAction {
                tap: Some(vec![KeyCode::KEY_B]),
                ..Default::default()
            },
        );

        let mut test_ctx = TestContext::with_mappings(mappings);
        let ctx = test_ctx.as_context();

        assert!(ctx.device_config.mappings.contains_key(&KeyCode::KEY_A));
    }

    #[test]
    fn test_context_with_layers() {
        let mut layers = HashMap::new();
        let mut layer_map = HashMap::new();
        let mut key_map = HashMap::new();
        key_map.insert(KeyCode::KEY_A, vec![KeyCode::KEY_B]);
        layer_map.insert(KeyCode::KEY_LEFTSHIFT, key_map);
        layers.insert("test_layer".to_string(), layer_map);

        let mut test_ctx = TestContext::with_layers(layers);
        let ctx = test_ctx.as_context();

        assert!(ctx.device_config.layers.contains_key("test_layer"));
    }
}
