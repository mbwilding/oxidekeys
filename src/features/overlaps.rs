use crate::{
    consts::*,
    features::{Context, Feature, FeatureResult, KeyEvent, OutputEvent},
};
use anyhow::Result;
use evdev::KeyCode;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug)]
struct ActiveOverlap {
    tap: Vec<KeyCode>,
    hold: Vec<KeyCode>,
    triggered: bool,
}

pub struct OverlapsFeature {
    /// Keys currently in overlap mode and their config/state
    active: HashMap<KeyCode, ActiveOverlap>,

    /// Keys whose raw events we will swallow because we emitted synthetic ones
    swallowed: HashSet<KeyCode>,
}

impl OverlapsFeature {
    pub fn new() -> Self {
        Self {
            active: HashMap::new(),
            swallowed: HashSet::new(),
        }
    }
}

impl Feature for OverlapsFeature {
    fn name(&self) -> &'static str {
        "overlaps"
    }

    fn on_event(&mut self, event: KeyEvent, ctx: &mut Context) -> Result<FeatureResult> {
        // If this key's press/release was already emitted synthetically, swallow raw
        if self.swallowed.contains(&event.key) {
            match event.state {
                // Swallow raw press (we already emitted it)
                PRESS => return Ok(FeatureResult::Consume),
                // Emit synthetic release and stop swallowing afterwards
                RELEASE => {
                    self.swallowed.remove(&event.key);
                    return Ok(FeatureResult::Emit(vec![OutputEvent::Release(event.key)]));
                }
                _ => {}
            }
        }

        // Is this key configured for overlap behavior?
        if let Some(remap) = ctx.device_config.mappings.get(&event.key)
            && remap.overlap.unwrap_or(false)
        {
            match event.state {
                // Start overlap window: defer emission until we know if another key is pressed
                PRESS => {
                    let tap = remap.tap.clone().unwrap_or_default();
                    let hold = remap.hold.clone().unwrap_or_default();
                    self.active.insert(
                        event.key,
                        ActiveOverlap {
                            tap,
                            hold,
                            triggered: false,
                        },
                    );
                    return Ok(FeatureResult::Consume);
                }
                // Decide on release: if no other key was pressed, send tap; otherwise release hold
                RELEASE => {
                    if let Some(active) = self.active.remove(&event.key) {
                        if active.triggered {
                            if active.hold.is_empty() {
                                return Ok(FeatureResult::Consume);
                            }
                            return Ok(FeatureResult::Emit(vec![OutputEvent::ReleaseMany(
                                active.hold,
                            )]));
                        } else {
                            if active.tap.is_empty() {
                                return Ok(FeatureResult::Consume);
                            }
                            return Ok(FeatureResult::Emit(vec![
                                OutputEvent::PressMany(active.tap.clone()),
                                OutputEvent::ReleaseMany(active.tap),
                            ]));
                        }
                    }

                    // Not tracked, pass through
                    return Ok(FeatureResult::Continue(event));
                }
                _ => {}
            }
        }

        // If some overlap is active and not yet triggered, and we press another key,
        // trigger the hold and emit this key's press synthetically so we can swallow raw.
        if event.state == 1
            && let Some((origin_key, active)) = self.active.iter_mut().find(|(_, a)| !a.triggered)
        {
            // Mark as triggered: hold stays down until origin_key is released
            active.triggered = true;
            let mut out = Vec::new();
            if !active.hold.is_empty() {
                out.push(OutputEvent::PressMany(active.hold.clone()));
            }

            // Emit the current key press explicitly and swallow its raw events
            out.push(OutputEvent::Press(event.key));
            self.swallowed.insert(event.key);

            // Keep the origin overlap active; its release will free the hold
            let _ = origin_key; // silence unused warning for pattern binding
            return Ok(FeatureResult::Emit(out));
        }

        // Default: let it pass through
        Ok(FeatureResult::Continue(event))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RemapAction;
    use crate::features::tests::{TestContext, press, release};
    use std::collections::HashMap;

    fn create_test_mappings() -> HashMap<KeyCode, RemapAction> {
        let mut mappings = HashMap::new();

        // Create a simple overlap mapping: SPACE -> SPACE on tap, LEFTSHIFT on hold
        mappings.insert(
            KeyCode::KEY_SPACE,
            RemapAction {
                tap: Some(vec![KeyCode::KEY_SPACE]),
                hold: Some(vec![KeyCode::KEY_LEFTSHIFT]),
                overlap: Some(true),
                ..Default::default()
            },
        );

        // Create another overlap mapping: A -> A on tap, LEFTCTRL on hold
        mappings.insert(
            KeyCode::KEY_A,
            RemapAction {
                tap: Some(vec![KeyCode::KEY_A]),
                hold: Some(vec![KeyCode::KEY_LEFTCTRL]),
                overlap: Some(true),
                ..Default::default()
            },
        );

        mappings
    }

    #[test]
    fn test_simple_tap_behavior() {
        let mappings = create_test_mappings();
        let mut test_ctx = TestContext::with_mappings(mappings);
        let mut feature = OverlapsFeature::new();

        // Press SPACE (overlap key)
        let press_event = press(KeyCode::KEY_SPACE);
        let press_result = feature
            .on_event(press_event, &mut test_ctx.as_context())
            .unwrap();

        // Should consume the press (start overlap window)
        match press_result {
            FeatureResult::Consume => {}
            _ => panic!("Expected Consume result for press"),
        }

        // Release SPACE without pressing another key (tap behavior)
        let release_event = release(KeyCode::KEY_SPACE);
        let release_result = feature
            .on_event(release_event, &mut test_ctx.as_context())
            .unwrap();

        // Should emit tap sequence (press and release SPACE)
        match release_result {
            FeatureResult::Emit(events) => {
                assert_eq!(events.len(), 2);
                match &events[0] {
                    OutputEvent::PressMany(keys) => {
                        assert_eq!(keys.len(), 1);
                        assert_eq!(keys[0], KeyCode::KEY_SPACE);
                    }
                    _ => panic!("Expected PressMany for tap"),
                }
                match &events[1] {
                    OutputEvent::ReleaseMany(keys) => {
                        assert_eq!(keys.len(), 1);
                        assert_eq!(keys[0], KeyCode::KEY_SPACE);
                    }
                    _ => panic!("Expected ReleaseMany for tap"),
                }
            }
            _ => panic!("Expected Emit result for tap"),
        }
    }

    #[test]
    fn test_hold_behavior_with_overlap() {
        let mappings = create_test_mappings();
        let mut test_ctx = TestContext::with_mappings(mappings);
        let mut feature = OverlapsFeature::new();

        // Press SPACE (overlap key)
        let space_press = press(KeyCode::KEY_SPACE);
        let _ = feature
            .on_event(space_press, &mut test_ctx.as_context())
            .unwrap();

        // Press another key (B) while SPACE is held (triggers hold behavior)
        let b_press = press(KeyCode::KEY_B);
        let b_result = feature
            .on_event(b_press, &mut test_ctx.as_context())
            .unwrap();

        // Should emit hold sequence and synthetic B press
        match b_result {
            FeatureResult::Emit(events) => {
                assert_eq!(events.len(), 2);
                match &events[0] {
                    OutputEvent::PressMany(keys) => {
                        assert_eq!(keys.len(), 1);
                        assert_eq!(keys[0], KeyCode::KEY_LEFTSHIFT);
                    }
                    _ => panic!("Expected PressMany for hold"),
                }
                match &events[1] {
                    OutputEvent::Press(key) => {
                        assert_eq!(*key, KeyCode::KEY_B);
                    }
                    _ => panic!("Expected Press for synthetic B"),
                }
            }
            _ => panic!("Expected Emit result for hold"),
        }

        // Release B (should be swallowed and emit synthetic release)
        let b_release = release(KeyCode::KEY_B);
        let b_release_result = feature
            .on_event(b_release, &mut test_ctx.as_context())
            .unwrap();

        match b_release_result {
            FeatureResult::Emit(events) => {
                assert_eq!(events.len(), 1);
                match &events[0] {
                    OutputEvent::Release(key) => {
                        assert_eq!(*key, KeyCode::KEY_B);
                    }
                    _ => panic!("Expected Release for synthetic B"),
                }
            }
            _ => panic!("Expected Emit result for B release"),
        }

        // Release SPACE (should release the hold)
        let space_release = release(KeyCode::KEY_SPACE);
        let space_release_result = feature
            .on_event(space_release, &mut test_ctx.as_context())
            .unwrap();

        match space_release_result {
            FeatureResult::Emit(events) => {
                assert_eq!(events.len(), 1);
                match &events[0] {
                    OutputEvent::ReleaseMany(keys) => {
                        assert_eq!(keys.len(), 1);
                        assert_eq!(keys[0], KeyCode::KEY_LEFTSHIFT);
                    }
                    _ => panic!("Expected ReleaseMany for hold release"),
                }
            }
            _ => panic!("Expected Emit result for hold release"),
        }
    }

    #[test]
    fn test_non_overlap_key_passthrough() {
        let mappings = create_test_mappings();
        let mut test_ctx = TestContext::with_mappings(mappings);
        let mut feature = OverlapsFeature::new();

        // Press a key that's not configured for overlap
        let event = press(KeyCode::KEY_Z);
        let result = feature.on_event(event, &mut test_ctx.as_context()).unwrap();

        // Should continue with the original event
        match result {
            FeatureResult::Continue(continued_event) => {
                assert_eq!(continued_event.key, KeyCode::KEY_Z);
            }
            _ => panic!("Expected Continue result"),
        }
    }

    #[test]
    fn test_empty_tap_sequence() {
        let mut mappings = HashMap::new();
        mappings.insert(
            KeyCode::KEY_SPACE,
            RemapAction {
                tap: Some(vec![]), // Empty tap sequence
                hold: Some(vec![KeyCode::KEY_LEFTSHIFT]),
                overlap: Some(true),
                ..Default::default()
            },
        );

        let mut test_ctx = TestContext::with_mappings(mappings);
        let mut feature = OverlapsFeature::new();

        // Press and release SPACE without overlap
        let _ = feature
            .on_event(press(KeyCode::KEY_SPACE), &mut test_ctx.as_context())
            .unwrap();
        let result = feature
            .on_event(release(KeyCode::KEY_SPACE), &mut test_ctx.as_context())
            .unwrap();

        // Should consume (no tap sequence to emit)
        match result {
            FeatureResult::Consume => {}
            _ => panic!("Expected Consume result for empty tap"),
        }
    }

    #[test]
    fn test_empty_hold_sequence() {
        let mut mappings = HashMap::new();
        mappings.insert(
            KeyCode::KEY_SPACE,
            RemapAction {
                tap: Some(vec![KeyCode::KEY_SPACE]),
                hold: Some(vec![]), // Empty hold sequence
                overlap: Some(true),
                ..Default::default()
            },
        );

        let mut test_ctx = TestContext::with_mappings(mappings);
        let mut feature = OverlapsFeature::new();

        // Press SPACE, then B (trigger hold), then release SPACE
        let _ = feature
            .on_event(press(KeyCode::KEY_SPACE), &mut test_ctx.as_context())
            .unwrap();
        let _ = feature
            .on_event(press(KeyCode::KEY_B), &mut test_ctx.as_context())
            .unwrap();
        let result = feature
            .on_event(release(KeyCode::KEY_SPACE), &mut test_ctx.as_context())
            .unwrap();

        // Should consume (no hold sequence to release)
        match result {
            FeatureResult::Consume => {}
            _ => panic!("Expected Consume result for empty hold"),
        }
    }

    #[test]
    fn test_multiple_overlap_keys() {
        let mappings = create_test_mappings();
        let mut test_ctx = TestContext::with_mappings(mappings);
        let mut feature = OverlapsFeature::new();

        // Press SPACE (first overlap key)
        let _ = feature
            .on_event(press(KeyCode::KEY_SPACE), &mut test_ctx.as_context())
            .unwrap();

        // Press B (non-overlap key) - should trigger SPACE's hold
        let b_result = feature
            .on_event(press(KeyCode::KEY_B), &mut test_ctx.as_context())
            .unwrap();

        // Should emit SPACE's hold and synthetic B press
        match b_result {
            FeatureResult::Emit(events) => {
                assert_eq!(events.len(), 2);
                match &events[0] {
                    OutputEvent::PressMany(keys) => {
                        assert_eq!(keys.len(), 1);
                        assert_eq!(keys[0], KeyCode::KEY_LEFTSHIFT); // SPACE's hold
                    }
                    _ => panic!("Expected PressMany for SPACE hold"),
                }
                match &events[1] {
                    OutputEvent::Press(key) => {
                        assert_eq!(*key, KeyCode::KEY_B);
                    }
                    _ => panic!("Expected Press for synthetic B"),
                }
            }
            _ => panic!("Expected Emit result"),
        }
    }

    #[test]
    fn test_swallowed_key_behavior() {
        let mappings = create_test_mappings();
        let mut test_ctx = TestContext::with_mappings(mappings);
        let mut feature = OverlapsFeature::new();

        // Press SPACE, then B (triggers hold and swallows B)
        let _ = feature
            .on_event(press(KeyCode::KEY_SPACE), &mut test_ctx.as_context())
            .unwrap();
        let _ = feature
            .on_event(press(KeyCode::KEY_B), &mut test_ctx.as_context())
            .unwrap();

        // Try to press B again (should be swallowed)
        let b_press_again = press(KeyCode::KEY_B);
        let result = feature
            .on_event(b_press_again, &mut test_ctx.as_context())
            .unwrap();

        // Should consume (swallow the raw press)
        match result {
            FeatureResult::Consume => {}
            _ => panic!("Expected Consume result for swallowed key"),
        }

        // Release B (should emit synthetic release and stop swallowing)
        let b_release = release(KeyCode::KEY_B);
        let release_result = feature
            .on_event(b_release, &mut test_ctx.as_context())
            .unwrap();

        match release_result {
            FeatureResult::Emit(events) => {
                assert_eq!(events.len(), 1);
                match &events[0] {
                    OutputEvent::Release(key) => {
                        assert_eq!(*key, KeyCode::KEY_B);
                    }
                    _ => panic!("Expected Release for synthetic B"),
                }
            }
            _ => panic!("Expected Emit result for B release"),
        }
    }

    #[test]
    fn test_untracked_key_release() {
        let mappings = create_test_mappings();
        let mut test_ctx = TestContext::with_mappings(mappings);
        let mut feature = OverlapsFeature::new();

        // Try to release a key that's not being tracked
        let event = release(KeyCode::KEY_SPACE);
        let result = feature.on_event(event, &mut test_ctx.as_context()).unwrap();

        // Should continue with the original event
        match result {
            FeatureResult::Continue(continued_event) => {
                assert_eq!(continued_event.key, KeyCode::KEY_SPACE);
            }
            _ => panic!("Expected Continue result"),
        }
    }

    #[test]
    fn test_complex_hold_sequence() {
        let mut mappings = HashMap::new();
        mappings.insert(
            KeyCode::KEY_SPACE,
            RemapAction {
                tap: Some(vec![KeyCode::KEY_SPACE]),
                hold: Some(vec![KeyCode::KEY_LEFTCTRL, KeyCode::KEY_C]),
                overlap: Some(true),
                ..Default::default()
            },
        );

        let mut test_ctx = TestContext::with_mappings(mappings);
        let mut feature = OverlapsFeature::new();

        // Press SPACE, then B (trigger hold)
        let _ = feature
            .on_event(press(KeyCode::KEY_SPACE), &mut test_ctx.as_context())
            .unwrap();
        let b_result = feature
            .on_event(press(KeyCode::KEY_B), &mut test_ctx.as_context())
            .unwrap();

        // Should emit complex hold sequence
        match b_result {
            FeatureResult::Emit(events) => {
                assert_eq!(events.len(), 2);
                match &events[0] {
                    OutputEvent::PressMany(keys) => {
                        assert_eq!(keys.len(), 2);
                        assert_eq!(keys[0], KeyCode::KEY_LEFTCTRL);
                        assert_eq!(keys[1], KeyCode::KEY_C);
                    }
                    _ => panic!("Expected PressMany for complex hold"),
                }
            }
            _ => panic!("Expected Emit result"),
        }

        // Release SPACE (should release the complex hold sequence)
        let space_release = release(KeyCode::KEY_SPACE);
        let release_result = feature
            .on_event(space_release, &mut test_ctx.as_context())
            .unwrap();

        match release_result {
            FeatureResult::Emit(events) => {
                assert_eq!(events.len(), 1);
                match &events[0] {
                    OutputEvent::ReleaseMany(keys) => {
                        assert_eq!(keys.len(), 2);
                        assert_eq!(keys[0], KeyCode::KEY_LEFTCTRL);
                        assert_eq!(keys[1], KeyCode::KEY_C);
                    }
                    _ => panic!("Expected ReleaseMany for complex hold"),
                }
            }
            _ => panic!("Expected Emit result for hold release"),
        }
    }
}
