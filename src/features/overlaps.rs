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
        if event.state == PRESS
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
