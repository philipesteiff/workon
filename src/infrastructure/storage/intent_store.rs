use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::domain::IntentProfile;
use crate::shared::error::{Result, WorkonError};

const INTENT_FILE: &str = "custom.json";

#[derive(Debug, Clone)]
pub(crate) struct JsonIntentStore {
    root: PathBuf,
}

impl JsonIntentStore {
    pub(crate) fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
        }
    }

    pub(crate) fn active_profiles(&self) -> Result<Vec<IntentProfile>> {
        Ok(self
            .read_catalog()?
            .intents
            .into_iter()
            .filter(|intent| !intent.archived)
            .map(|intent| intent.profile)
            .collect())
    }

    pub(crate) fn find_active(&self, intent_id: &str) -> Result<Option<IntentProfile>> {
        Ok(self
            .active_profiles()?
            .into_iter()
            .find(|intent| intent.id == intent_id))
    }

    pub(crate) fn create(&self, profile: IntentProfile) -> Result<IntentProfile> {
        let mut catalog = self.read_catalog()?;
        if catalog
            .intents
            .iter()
            .any(|intent| !intent.archived && intent.profile.id == profile.id)
        {
            return Err(WorkonError::IntentContext {
                message: format!("intent already exists: `{}`", profile.id),
            });
        }

        catalog
            .intents
            .retain(|intent| intent.profile.id != profile.id);
        catalog.intents.push(StoredIntent {
            profile: profile.clone(),
            archived: false,
        });
        catalog.sort();
        self.write_catalog(&catalog)?;
        Ok(profile)
    }

    pub(crate) fn update(&self, profile: IntentProfile) -> Result<IntentProfile> {
        let mut catalog = self.read_catalog()?;
        let Some(intent) = catalog
            .intents
            .iter_mut()
            .find(|intent| !intent.archived && intent.profile.id == profile.id)
        else {
            return Err(WorkonError::UnknownIntent {
                intent_id: profile.id,
                available: self
                    .active_profiles()?
                    .into_iter()
                    .map(|intent| intent.id)
                    .collect(),
            });
        };

        intent.profile = profile.clone();
        catalog.sort();
        self.write_catalog(&catalog)?;
        Ok(profile)
    }

    pub(crate) fn archive(&self, intent_id: &str) -> Result<IntentProfile> {
        let mut catalog = self.read_catalog()?;
        let Some(intent) = catalog
            .intents
            .iter_mut()
            .find(|intent| !intent.archived && intent.profile.id == intent_id)
        else {
            return Err(WorkonError::UnknownIntent {
                intent_id: intent_id.to_string(),
                available: self
                    .active_profiles()?
                    .into_iter()
                    .map(|intent| intent.id)
                    .collect(),
            });
        };

        intent.archived = true;
        let profile = intent.profile.clone();
        self.write_catalog(&catalog)?;
        Ok(profile)
    }

    fn read_catalog(&self) -> Result<StoredIntentCatalog> {
        let path = self.path();
        if !path.exists() {
            return Ok(StoredIntentCatalog::default());
        }

        let content = std::fs::read_to_string(&path)?;
        serde_json::from_str(&content).map_err(|error| WorkonError::IntentContext {
            message: format!("invalid intent catalog at {}: {error}", path.display()),
        })
    }

    fn write_catalog(&self, catalog: &StoredIntentCatalog) -> Result<()> {
        let path = self.path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content =
            serde_json::to_string_pretty(catalog).map_err(|error| WorkonError::IntentContext {
                message: format!("could not write intent catalog: {error}"),
            })?;
        std::fs::write(path, content)?;
        Ok(())
    }

    fn path(&self) -> PathBuf {
        self.root.join(".workon").join("intents").join(INTENT_FILE)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct StoredIntentCatalog {
    intents: Vec<StoredIntent>,
}

impl StoredIntentCatalog {
    fn sort(&mut self) {
        self.intents
            .sort_by(|left, right| left.profile.id.cmp(&right.profile.id));
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredIntent {
    #[serde(flatten)]
    profile: IntentProfile,
    archived: bool,
}
