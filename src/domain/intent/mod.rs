mod catalog;

pub use catalog::IntentCatalog;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntentProfile {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub skill_weights: Vec<String>,
    pub mcp_weights: Vec<String>,
    pub instructions: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntentSource {
    BuiltIn,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentSummary {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub source: IntentSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentList {
    pub intents: Vec<IntentSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentProfileChange {
    pub intent: IntentProfile,
    pub source: IntentSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkIntentSwitch {
    pub work: crate::domain::WorkSummary,
    pub previous_intent_id: String,
    pub intent: IntentProfile,
}
