use crate::features::{Context, Feature, FeatureResult, KeyEvent};
use anyhow::Result;

pub struct TermsFeature {}

impl TermsFeature {
    pub fn new() -> Self {
        Self {}
    }
}

impl Feature for TermsFeature {
    fn name(&self) -> &'static str {
        "terms"
    }

    fn on_event(&mut self, event: KeyEvent, _ctx: &mut Context) -> Result<FeatureResult> {
        Ok(FeatureResult::Continue(event))
    }
}
