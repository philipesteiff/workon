use std::collections::BTreeSet;

use crate::domain::IntentProfile;
use crate::domain::{IntentSource, IntentSummary};

#[derive(Debug, Clone)]
pub struct IntentCatalog {
    profiles: Vec<IntentProfile>,
    custom_ids: BTreeSet<String>,
}

impl IntentCatalog {
    pub const BLANK_INTENT_ID: &'static str = "blank";

    pub fn with_profiles(defaults: Vec<IntentProfile>, custom: Vec<IntentProfile>) -> Self {
        let mut profiles = Vec::new();
        let mut default_ids = BTreeSet::new();
        for profile in defaults {
            if default_ids.insert(profile.id.clone()) {
                profiles.push(profile);
            }
        }

        let mut catalog = Self {
            profiles,
            custom_ids: BTreeSet::new(),
        };
        let existing = catalog
            .profiles
            .iter()
            .map(|profile| profile.id.clone())
            .collect::<BTreeSet<_>>();

        for profile in custom {
            if existing.contains(&profile.id) || catalog.custom_ids.contains(&profile.id) {
                continue;
            }
            catalog.custom_ids.insert(profile.id.clone());
            catalog.profiles.push(profile);
        }

        catalog
    }

    pub fn empty() -> Self {
        Self {
            profiles: Vec::new(),
            custom_ids: BTreeSet::new(),
        }
    }

    pub fn find(&self, id: &str) -> Option<IntentProfile> {
        self.profiles
            .iter()
            .find(|profile| profile.id == id)
            .cloned()
    }

    pub fn available_ids(&self) -> Vec<String> {
        self.profiles
            .iter()
            .map(|profile| profile.id.clone())
            .collect()
    }

    pub fn profiles(&self) -> &[IntentProfile] {
        &self.profiles
    }

    pub fn summaries(&self) -> Vec<IntentSummary> {
        self.profiles
            .iter()
            .map(|profile| IntentSummary {
                id: profile.id.clone(),
                name: profile.name.clone(),
                summary: profile.summary.clone(),
                source: self.source(&profile.id).unwrap_or(IntentSource::Default),
            })
            .collect()
    }

    pub fn source(&self, id: &str) -> Option<IntentSource> {
        if self.custom_ids.contains(id) {
            Some(IntentSource::Custom)
        } else if self.profiles.iter().any(|profile| profile.id == id) {
            Some(IntentSource::Default)
        } else {
            None
        }
    }
}
