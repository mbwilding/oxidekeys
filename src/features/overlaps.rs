use crate::features::{Context, Feature, FeatureResult, KeyEvent};
use anyhow::Result;

pub struct OverlapsFeature;

impl OverlapsFeature {
    pub fn new() -> Self {
        Self
    }
}

impl Feature for OverlapsFeature {
    fn name(&self) -> &'static str {
        "overlaps"
    }

    fn on_event(&mut self, event: KeyEvent, ctx: &mut Context) -> Result<FeatureResult> {
        if event.state == crate::consts::PRESS {
            for (pending_key_code, pending_key) in ctx.pending.iter_mut() {
                if pending_key.remap.hrm != Some(true)
                    && pending_key.remap.tap.is_some()
                    && pending_key.remap.hold.is_some()
                    && !pending_key.hold_sent
                    && !pending_key.overlap_hold_sent
                    && ctx.keys_down.contains(pending_key_code)
                {
                    if let Some(_hold) = pending_key.remap.hold.as_ref() {
                        pending_key.hold_sent = true;
                        pending_key.overlap_hold_sent = true;
                    }
                }
            }
            ctx.keys_down.insert(event.key);
        } else if event.state == crate::consts::RELEASE {
            ctx.keys_down.remove(&event.key);
        }
        Ok(FeatureResult::Continue(event))
    }
}
