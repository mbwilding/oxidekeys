use crate::{
    consts::*,
    features::{Context, Feature, FeatureResult, KeyEvent, OutputEvent},
};
use anyhow::Result;
use evdev::KeyCode;
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
struct ActiveTerm {
    tap: Vec<KeyCode>,
    hold: Vec<KeyCode>,
    term_duration: Duration,
    press_time: Instant,
}

pub struct TermsFeature {
    /// Keys currently in term mode and their config/state
    active: HashMap<KeyCode, ActiveTerm>,
    /// Channel to send timer events
    timer_sender: crossbeam_channel::Sender<KeyCode>,
}

impl TermsFeature {
    pub fn new(timer_sender: crossbeam_channel::Sender<KeyCode>) -> Self {
        Self {
            active: HashMap::new(),
            timer_sender,
        }
    }

    /// Get the term duration for a key, using per-mapping term if available, otherwise global term
    fn get_term_duration(&self, key: KeyCode, ctx: &Context) -> Duration {
        if let Some(remap) = ctx.device_config.mappings.get(&key) {
            if let Some(term_ms) = remap.term {
                return Duration::from_millis(term_ms as u64);
            }
        }
        // Use global term (convert from u16 milliseconds to Duration)
        Duration::from_millis(ctx.global_term as u64)
    }

    /// Start a timer for a key that will send a timer event when term time expires
    fn start_timer(&self, key: KeyCode, term_duration: Duration) {
        let sender = self.timer_sender.clone();
        std::thread::spawn(move || {
            std::thread::sleep(term_duration);
            let _ = sender.send(key);
        });
    }


}

impl Feature for TermsFeature {
    fn name(&self) -> &'static str {
        "terms"
    }

    fn on_event(&mut self, event: KeyEvent, ctx: &mut Context) -> Result<FeatureResult> {
        // Check if this key is configured for term behavior (has tap/hold but NOT overlap)
        if let Some(remap) = ctx.device_config.mappings.get(&event.key) {
            let has_tap = remap.tap.is_some();
            let has_hold = remap.hold.is_some();
            let is_overlap = remap.overlap.unwrap_or(false);

            // Only handle term behavior if we have tap or hold, and overlap is NOT true
            if (has_tap || has_hold) && !is_overlap {
                match event.state {
                    PRESS => {
                        let term_duration = self.get_term_duration(event.key, ctx);
                        let tap = remap.tap.clone().unwrap_or_default();
                        let hold = remap.hold.clone().unwrap_or_default();

                        // Start timer for hold emission
                        if !hold.is_empty() {
                            self.start_timer(event.key, term_duration);
                        }

                        self.active.insert(
                            event.key,
                            ActiveTerm {
                                tap,
                                hold,
                                term_duration,
                                press_time: Instant::now(),
                            },
                        );

                        // Consume the press event - we'll decide what to emit later
                        return Ok(FeatureResult::Consume);
                    }
                    RELEASE => {
                        if let Some(active) = self.active.remove(&event.key) {
                            let elapsed = active.press_time.elapsed();

                            if elapsed >= active.term_duration {
                                // Term time exceeded - hold was already emitted, just release it
                                if !active.hold.is_empty() {
                                    return Ok(FeatureResult::Emit(vec![OutputEvent::ReleaseMany(active.hold)]));
                                } else {
                                    return Ok(FeatureResult::Consume);
                                }
                            } else {
                                // Term time not exceeded - emit tap sequence
                                if !active.tap.is_empty() {
                                    return Ok(FeatureResult::Emit(vec![
                                        OutputEvent::PressMany(active.tap.clone()),
                                        OutputEvent::ReleaseMany(active.tap),
                                    ]));
                                } else {
                                    return Ok(FeatureResult::Consume);
                                }
                            }
                        }

                        // Not tracked, pass through
                        return Ok(FeatureResult::Continue(event));
                    }
                    _ => {}
                }
            }
        }

        // Default: let it pass through
        Ok(FeatureResult::Continue(event))
    }

    fn on_timer(&mut self, key: KeyCode, _ctx: &mut Context) -> Result<Option<Vec<OutputEvent>>> {
        // Check if this key is still active and needs hold emission
        if let Some(active) = self.active.get(&key) {
            let elapsed = active.press_time.elapsed();

            if elapsed >= active.term_duration && !active.hold.is_empty() {
                // Term time has expired, emit hold
                return Ok(Some(vec![OutputEvent::PressMany(active.hold.clone())]));
            }
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RemapAction;
    use crate::features::tests::{TestContext, press, release};
    use std::collections::HashMap;
    use std::thread;
    use std::time::Duration;

    fn create_test_mappings() -> HashMap<KeyCode, RemapAction> {
        let mut mappings = HashMap::new();

        // Create a simple term mapping: SPACE -> SPACE on tap, LEFTSHIFT on hold
        mappings.insert(
            KeyCode::KEY_SPACE,
            RemapAction {
                tap: Some(vec![KeyCode::KEY_SPACE]),
                hold: Some(vec![KeyCode::KEY_LEFTSHIFT]),
                overlap: Some(false), // Explicitly set overlap to false
                ..Default::default()
            },
        );

        // Create another term mapping: A -> A on tap, LEFTCTRL on hold
        mappings.insert(
            KeyCode::KEY_A,
            RemapAction {
                tap: Some(vec![KeyCode::KEY_A]),
                hold: Some(vec![KeyCode::KEY_LEFTCTRL]),
                overlap: Some(false), // Explicitly set overlap to false
                ..Default::default()
            },
        );

        mappings
    }

    fn create_test_feature() -> TermsFeature {
        let (timer_tx, _timer_rx) = crossbeam_channel::unbounded();
        TermsFeature::new(timer_tx)
    }

    #[test]
    fn test_simple_tap_behavior() {
        let mappings = create_test_mappings();
        let mut test_ctx = TestContext::with_mappings(mappings);
        let mut feature = create_test_feature();

        // Press SPACE (term key)
        let press_event = press(KeyCode::KEY_SPACE);
        let press_result = feature
            .on_event(press_event, &mut test_ctx.as_context())
            .unwrap();

        // Should consume the press (start term window)
        match press_result {
            FeatureResult::Consume => {}
            _ => panic!("Expected Consume result for press"),
        }

        // Release SPACE quickly (tap behavior)
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
    fn test_hold_behavior_after_term() {
        let mappings = create_test_mappings();
        let mut test_ctx = TestContext::with_mappings(mappings);
        let mut feature = create_test_feature();

        // Press SPACE (term key)
        let space_press = press(KeyCode::KEY_SPACE);
        let _ = feature
            .on_event(space_press, &mut test_ctx.as_context())
            .unwrap();

        // Wait for term time to elapse (144ms + buffer)
        thread::sleep(Duration::from_millis(150));

        // Check for hold emission using on_timer (simulating timer expiration)
        let hold_result = feature
            .on_timer(KeyCode::KEY_SPACE, &mut test_ctx.as_context())
            .unwrap();

        // Should emit hold press when term time expires
        match hold_result {
            Some(events) => {
                assert_eq!(events.len(), 1);
                match &events[0] {
                    OutputEvent::PressMany(keys) => {
                        assert_eq!(keys.len(), 1);
                        assert_eq!(keys[0], KeyCode::KEY_LEFTSHIFT);
                    }
                    _ => panic!("Expected PressMany for hold"),
                }
            }
            _ => panic!("Expected Some(events) for hold"),
        }

        // Release SPACE after term time (should release the hold)
        let space_release = release(KeyCode::KEY_SPACE);
        let release_result = feature
            .on_event(space_release, &mut test_ctx.as_context())
            .unwrap();

        // Should emit hold release
        match release_result {
            FeatureResult::Emit(events) => {
                assert_eq!(events.len(), 1);
                match &events[0] {
                    OutputEvent::ReleaseMany(keys) => {
                        assert_eq!(keys.len(), 1);
                        assert_eq!(keys[0], KeyCode::KEY_LEFTSHIFT);
                    }
                    _ => panic!("Expected ReleaseMany for hold"),
                }
            }
            _ => panic!("Expected Emit result for hold release"),
        }
    }

    #[test]
    fn test_non_term_key_passthrough() {
        let mappings = create_test_mappings();
        let mut test_ctx = TestContext::with_mappings(mappings);
        let mut feature = create_test_feature();

        // Press a key that's not configured for term behavior
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
    fn test_overlap_key_ignored() {
        let mut mappings = HashMap::new();
        mappings.insert(
            KeyCode::KEY_SPACE,
            RemapAction {
                tap: Some(vec![KeyCode::KEY_SPACE]),
                hold: Some(vec![KeyCode::KEY_LEFTSHIFT]),
                overlap: Some(true), // This should be ignored by terms feature
                ..Default::default()
            },
        );

        let mut test_ctx = TestContext::with_mappings(mappings);
        let mut feature = create_test_feature();

        // Press SPACE (overlap key - should be ignored by terms feature)
        let event = press(KeyCode::KEY_SPACE);
        let result = feature.on_event(event, &mut test_ctx.as_context()).unwrap();

        // Should continue with the original event (terms feature ignores overlap keys)
        match result {
            FeatureResult::Continue(continued_event) => {
                assert_eq!(continued_event.key, KeyCode::KEY_SPACE);
            }
            _ => panic!("Expected Continue result for overlap key"),
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
                overlap: Some(false),
                ..Default::default()
            },
        );

        let mut test_ctx = TestContext::with_mappings(mappings);
        let mut feature = create_test_feature();

        // Press and release SPACE quickly
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
                overlap: Some(false),
                ..Default::default()
            },
        );

        let mut test_ctx = TestContext::with_mappings(mappings);
        let mut feature = create_test_feature();

        // Press SPACE, wait for term time, then release
        let _ = feature
            .on_event(press(KeyCode::KEY_SPACE), &mut test_ctx.as_context())
            .unwrap();
        thread::sleep(Duration::from_millis(150));
        let result = feature
            .on_event(release(KeyCode::KEY_SPACE), &mut test_ctx.as_context())
            .unwrap();

        // Should consume (no hold sequence to emit)
        match result {
            FeatureResult::Consume => {}
            _ => panic!("Expected Consume result for empty hold"),
        }
    }

    #[test]
    fn test_per_mapping_term_override() {
        let mut mappings = HashMap::new();
        mappings.insert(
            KeyCode::KEY_SPACE,
            RemapAction {
                tap: Some(vec![KeyCode::KEY_SPACE]),
                hold: Some(vec![KeyCode::KEY_LEFTSHIFT]),
                overlap: Some(false),
                term: Some(50), // Custom term of 50ms
                ..Default::default()
            },
        );

        let mut test_ctx = TestContext::with_mappings(mappings);
        let mut feature = create_test_feature();

        // Press SPACE
        let _ = feature
            .on_event(press(KeyCode::KEY_SPACE), &mut test_ctx.as_context())
            .unwrap();

        // Wait 30ms (less than custom term)
        thread::sleep(Duration::from_millis(30));
        let result = feature
            .on_event(release(KeyCode::KEY_SPACE), &mut test_ctx.as_context())
            .unwrap();

        // Should emit tap (released before custom term)
        match result {
            FeatureResult::Emit(events) => {
                assert_eq!(events.len(), 2);
                match &events[0] {
                    OutputEvent::PressMany(keys) => {
                        assert_eq!(keys.len(), 1);
                        assert_eq!(keys[0], KeyCode::KEY_SPACE);
                    }
                    _ => panic!("Expected PressMany for tap"),
                }
            }
            _ => panic!("Expected Emit result for tap"),
        }

        // Test hold behavior with custom term
        let _ = feature
            .on_event(press(KeyCode::KEY_SPACE), &mut test_ctx.as_context())
            .unwrap();

        // Wait 60ms (more than custom term)
        thread::sleep(Duration::from_millis(60));

        // Check for hold emission using on_timer (simulating timer expiration)
        let hold_result = feature
            .on_timer(KeyCode::KEY_SPACE, &mut test_ctx.as_context())
            .unwrap();

        // Should emit hold press when term time expires
        match hold_result {
            Some(events) => {
                assert_eq!(events.len(), 1);
                match &events[0] {
                    OutputEvent::PressMany(keys) => {
                        assert_eq!(keys.len(), 1);
                        assert_eq!(keys[0], KeyCode::KEY_LEFTSHIFT);
                    }
                    _ => panic!("Expected PressMany for hold"),
                }
            }
            _ => panic!("Expected Some(events) for hold"),
        }

        // Release SPACE (should release the hold)
        let result = feature
            .on_event(release(KeyCode::KEY_SPACE), &mut test_ctx.as_context())
            .unwrap();

        // Should emit hold release
        match result {
            FeatureResult::Emit(events) => {
                assert_eq!(events.len(), 1);
                match &events[0] {
                    OutputEvent::ReleaseMany(keys) => {
                        assert_eq!(keys.len(), 1);
                        assert_eq!(keys[0], KeyCode::KEY_LEFTSHIFT);
                    }
                    _ => panic!("Expected ReleaseMany for hold"),
                }
            }
            _ => panic!("Expected Emit result for hold release"),
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
                overlap: Some(false),
                ..Default::default()
            },
        );

        let mut test_ctx = TestContext::with_mappings(mappings);
        let mut feature = create_test_feature();

        // Press SPACE
        let _ = feature
            .on_event(press(KeyCode::KEY_SPACE), &mut test_ctx.as_context())
            .unwrap();

        // Wait for term time to elapse
        thread::sleep(Duration::from_millis(150));

        // Check for hold emission using on_timer (simulating timer expiration)
        let hold_result = feature
            .on_timer(KeyCode::KEY_SPACE, &mut test_ctx.as_context())
            .unwrap();

        // Should emit complex hold sequence press
        match hold_result {
            Some(events) => {
                assert_eq!(events.len(), 1);
                match &events[0] {
                    OutputEvent::PressMany(keys) => {
                        assert_eq!(keys.len(), 2);
                        assert_eq!(keys[0], KeyCode::KEY_LEFTCTRL);
                        assert_eq!(keys[1], KeyCode::KEY_C);
                    }
                    _ => panic!("Expected PressMany for complex hold"),
                }
            }
            _ => panic!("Expected Some(events) for hold"),
        }

        // Release SPACE (should release the complex hold sequence)
        let result = feature
            .on_event(release(KeyCode::KEY_SPACE), &mut test_ctx.as_context())
            .unwrap();

        // Should emit complex hold sequence release
        match result {
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

    #[test]
    fn test_untracked_key_release() {
        let mappings = create_test_mappings();
        let mut test_ctx = TestContext::with_mappings(mappings);
        let mut feature = create_test_feature();

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
    fn test_global_term_usage() {
        let mut mappings = HashMap::new();
        mappings.insert(
            KeyCode::KEY_SPACE,
            RemapAction {
                tap: Some(vec![KeyCode::KEY_SPACE]),
                hold: Some(vec![KeyCode::KEY_LEFTSHIFT]),
                overlap: Some(false),
                // No custom term - should use global term
                ..Default::default()
            },
        );

        let mut test_ctx = TestContext::with_mappings(mappings);
        test_ctx.global_term = 200; // Set custom global term
        let mut feature = create_test_feature();

        // Press SPACE
        let _ = feature
            .on_event(press(KeyCode::KEY_SPACE), &mut test_ctx.as_context())
            .unwrap();

        // Wait 100ms (less than global term)
        thread::sleep(Duration::from_millis(100));
        let result = feature
            .on_event(release(KeyCode::KEY_SPACE), &mut test_ctx.as_context())
            .unwrap();

        // Should emit tap (released before global term)
        match result {
            FeatureResult::Emit(events) => {
                assert_eq!(events.len(), 2);
                match &events[0] {
                    OutputEvent::PressMany(keys) => {
                        assert_eq!(keys.len(), 1);
                        assert_eq!(keys[0], KeyCode::KEY_SPACE);
                    }
                    _ => panic!("Expected PressMany for tap"),
                }
            }
            _ => panic!("Expected Emit result for tap"),
        }

        // Test hold behavior with global term
        let _ = feature
            .on_event(press(KeyCode::KEY_SPACE), &mut test_ctx.as_context())
            .unwrap();

        // Wait 250ms (more than global term)
        thread::sleep(Duration::from_millis(250));

        // Check for hold emission using on_timer (simulating timer expiration)
        let hold_result = feature
            .on_timer(KeyCode::KEY_SPACE, &mut test_ctx.as_context())
            .unwrap();

        // Should emit hold press when term time expires
        match hold_result {
            Some(events) => {
                assert_eq!(events.len(), 1);
                match &events[0] {
                    OutputEvent::PressMany(keys) => {
                        assert_eq!(keys.len(), 1);
                        assert_eq!(keys[0], KeyCode::KEY_LEFTSHIFT);
                    }
                    _ => panic!("Expected PressMany for hold"),
                }
            }
            _ => panic!("Expected Some(events) for hold"),
        }

        // Release SPACE (should release the hold)
        let result = feature
            .on_event(release(KeyCode::KEY_SPACE), &mut test_ctx.as_context())
            .unwrap();

        // Should emit hold release
        match result {
            FeatureResult::Emit(events) => {
                assert_eq!(events.len(), 1);
                match &events[0] {
                    OutputEvent::ReleaseMany(keys) => {
                        assert_eq!(keys.len(), 1);
                        assert_eq!(keys[0], KeyCode::KEY_LEFTSHIFT);
                    }
                    _ => panic!("Expected ReleaseMany for hold"),
                }
            }
            _ => panic!("Expected Emit result for hold release"),
        }
    }
}
