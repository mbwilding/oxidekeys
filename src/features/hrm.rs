use crate::features::{Context, Feature, FeatureResult, KeyEvent, OutputEvent};
use anyhow::Result;
use crossbeam_channel::{Receiver, Sender, select, unbounded};
use evdev::KeyCode;
use std::time::Duration;

enum TimerMsg {
    HoldTimeout(KeyCode),
}

pub struct HrmFeature {
    timer_tx: Sender<TimerMsg>,
    timer_rx: Receiver<TimerMsg>,
}

impl HrmFeature {
    pub fn new() -> Self {
        let (tx, rx) = unbounded();
        Self {
            timer_tx: tx,
            timer_rx: rx,
        }
    }
}

fn schedule_pending_key_timer(key: KeyCode, duration: Duration, tx: Sender<TimerMsg>) {
    std::thread::spawn(move || {
        std::thread::sleep(duration);
        let _ = tx.send(TimerMsg::HoldTimeout(key));
    });
}

impl Feature for HrmFeature {
    fn name(&self) -> &'static str {
        "hrm"
    }

    fn on_event(&mut self, event: KeyEvent, ctx: &mut Context) -> Result<FeatureResult> {
        // Drain timers non-blocking
        select! {
            default => {},
            recv(self.timer_rx) -> msg => {
                if let Ok(TimerMsg::HoldTimeout(key)) = msg
                    && let Some(pending_key) = ctx.pending.get_mut(&key)
                    && !pending_key.hold_sent && !pending_key.timer_fired {
                        let remap = &pending_key.remap;
                        if remap.hrm == Some(true) && pending_key.tap_sent {
                            pending_key.timer_fired = true;
                        } else if let Some(hold) = &remap.hold {
                            // Emit hold immediately
                            return Ok(FeatureResult::Emit(vec![OutputEvent::PressMany(hold.clone())]));
                        }
                        pending_key.timer_fired = true;
                    }
            }
        }

        if event.state == crate::consts::PRESS {
            // For each remapped key that is HRM with hold, start timer
            if let Some(remap) = ctx.device_config.mappings.get(&event.key)
                && remap.hrm == Some(true)
                && remap.hold.is_some()
            {
                let duration = Duration::from_millis(
                    remap.hrm_term.unwrap_or(ctx.config.globals.hrm_term) as u64,
                );
                schedule_pending_key_timer(event.key, duration, self.timer_tx.clone());
            }

            // Add to pending or emit immediate tap-only
            if let Some(remap) = ctx.device_config.mappings.get(&event.key) {
                if let Some(keys) = remap.tap.as_ref()
                    && remap.hold.is_none()
                {
                    return Ok(FeatureResult::Emit(vec![OutputEvent::PressMany(
                        keys.clone(),
                    )]));
                }
                crate::state::add_pending(ctx.pending, event.key, remap);
            } else {
                return Ok(FeatureResult::Continue(event));
            }
            Ok(FeatureResult::Consume)
        } else if event.state == crate::consts::RELEASE {
            if let Some(pending_key) = crate::state::remove_pending(ctx.pending, &event.key) {
                let remap = pending_key.remap;
                let is_hrm = remap.hrm == Some(true);
                if is_hrm {
                    let hrm_term = remap.hrm_term.unwrap_or(ctx.config.globals.hrm_term);
                    let elapsed = pending_key.time_pressed.elapsed();
                    if elapsed < Duration::from_millis(hrm_term as u64) {
                        if let Some(tap) = remap.tap
                            && !pending_key.tap_sent
                        {
                            return Ok(FeatureResult::Emit(vec![
                                OutputEvent::PressMany(tap.clone()),
                                OutputEvent::ReleaseMany(tap),
                            ]));
                        }
                    } else if remap.hold.is_some() && pending_key.hold_sent {
                        return Ok(FeatureResult::Emit(vec![OutputEvent::ReleaseMany(
                            remap.hold.unwrap(),
                        )]));
                    }
                } else if pending_key.overlap_hold_sent {
                    if let Some(hold) = remap.hold {
                        return Ok(FeatureResult::Emit(vec![OutputEvent::ReleaseMany(hold)]));
                    }
                } else {
                    match (remap.tap, remap.hold, pending_key.hold_sent) {
                        (_, Some(hold), true) => {
                            return Ok(FeatureResult::Emit(vec![OutputEvent::ReleaseMany(hold)]));
                        }
                        (Some(tap), _, _) => {
                            return Ok(FeatureResult::Emit(vec![
                                OutputEvent::PressMany(tap.clone()),
                                OutputEvent::ReleaseMany(tap),
                            ]));
                        }
                        _ => {}
                    }
                }
                Ok(FeatureResult::Consume)
            } else {
                Ok(FeatureResult::Continue(event))
            }
        } else {
            Ok(FeatureResult::Consume)
        }
    }
}
