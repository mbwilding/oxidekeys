use crate::{
    config::Layers,
    consts::*,
    features::{Context, Feature, FeatureResult, KeyEvent, OutputEvent},
};
use anyhow::Result;
use evdev::KeyCode;
use std::collections::HashSet;

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
        Ok(FeatureResult::Consume)
    }
}
