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
    hold_emitted: bool,
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
        if let Some(remap) = ctx.device_config.mappings.get(&key)
            && let Some(term_ms) = remap.term
        {
            return Duration::from_millis(term_ms as u64);
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

                        // Add key to keys_down since we're tracking it
                        ctx.keys_down.insert(event.key);

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
                                hold_emitted: false,
                            },
                        );

                        // Consume the press event - we'll decide what to emit later
                        return Ok(FeatureResult::Consume);
                    }
                    RELEASE => {
                        if let Some(active) = self.active.remove(&event.key) {
                            // Remove key from keys_down since we're no longer tracking it
                            ctx.keys_down.remove(&event.key);

                            if active.hold_emitted {
                                // Hold was already emitted, just release it
                                if !active.hold.is_empty() {
                                    return Ok(FeatureResult::Emit(vec![
                                        OutputEvent::ReleaseMany(active.hold),
                                    ]));
                                } else {
                                    return Ok(FeatureResult::Consume);
                                }
                            } else {
                                // Hold was not emitted, emit tap sequence
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

    fn on_timer(&mut self, key: KeyCode, ctx: &mut Context) -> Result<Option<Vec<OutputEvent>>> {
        // Check if this key is still active and needs hold emission
        if let Some(active) = self.active.get(&key) {
            let elapsed = active.press_time.elapsed();

            // Only emit hold if:
            // 1. Term time has expired
            // 2. Key is still being held down (in keys_down)
            // 3. Hold sequence is not empty
            // 4. Hold hasn't been emitted yet
            if elapsed >= active.term_duration
                && ctx.keys_down.contains(&key)
                && !active.hold.is_empty()
                && !active.hold_emitted
            {
                // Mark this key as having emitted its hold
                if let Some(mut active) = self.active.remove(&key) {
                    active.hold_emitted = true;
                    let hold_sequence = active.hold.clone();

                    // Put the key back with hold_emitted = true
                    self.active.insert(key, active);

                    return Ok(Some(vec![OutputEvent::PressMany(hold_sequence)]));
                }
            }
        }

        Ok(None)
    }
}
