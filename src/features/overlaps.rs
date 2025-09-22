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

    fn on_event(&mut self, event: KeyEvent, _ctx: &mut Context) -> Result<FeatureResult> {
        Ok(FeatureResult::Continue(event))
    }
}
