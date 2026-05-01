use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::domain::IntentProfile;
use crate::shared::error::{Result, WorkonError};

const BUNDLED_DEFAULT_INTENTS: &[(&str, &str)] = &[
    (
        "address-pr-comments.yaml",
        include_str!("../../../config/intents/default/address-pr-comments.yaml"),
    ),
    (
        "blank.yaml",
        include_str!("../../../config/intents/default/blank.yaml"),
    ),
    (
        "brainstorm.yaml",
        include_str!("../../../config/intents/default/brainstorm.yaml"),
    ),
    (
        "design-to-prs.yaml",
        include_str!("../../../config/intents/default/design-to-prs.yaml"),
    ),
    (
        "investigate.yaml",
        include_str!("../../../config/intents/default/investigate.yaml"),
    ),
    (
        "presentation.yaml",
        include_str!("../../../config/intents/default/presentation.yaml"),
    ),
    (
        "review-pr.yaml",
        include_str!("../../../config/intents/default/review-pr.yaml"),
    ),
    (
        "slack-to-pr.yaml",
        include_str!("../../../config/intents/default/slack-to-pr.yaml"),
    ),
];
const CUSTOM_INTENT_DIR: &str = "custom";
const DEFAULT_INTENT_DIR: &str = "default";
const LEGACY_CUSTOM_INTENT_FILE: &str = "custom.json";

#[derive(Debug, Clone)]
pub(crate) struct YamlIntentStore {
    root: PathBuf,
}

impl YamlIntentStore {
    pub(crate) fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
        }
    }

    pub(crate) fn default_profiles(&self) -> Result<Vec<IntentProfile>> {
        self.ensure_default_profiles()?;
        self.read_profiles(&self.default_path())
    }

    pub(crate) fn custom_profiles(&self) -> Result<Vec<IntentProfile>> {
        self.migrate_legacy_custom_profiles()?;
        self.read_profiles(&self.custom_path())
    }

    fn ensure_default_profiles(&self) -> Result<()> {
        let directory = self.default_path();
        std::fs::create_dir_all(&directory)?;
        for (filename, content) in BUNDLED_DEFAULT_INTENTS {
            let path = directory.join(filename);
            if !path.exists() {
                std::fs::write(path, content)?;
            }
        }
        Ok(())
    }

    fn read_profiles(&self, directory: &Path) -> Result<Vec<IntentProfile>> {
        if !directory.exists() {
            return Ok(Vec::new());
        }

        let mut files = Vec::new();
        for entry in std::fs::read_dir(directory)? {
            let path = entry?.path();
            if is_yaml_file(&path) {
                files.push(path);
            }
        }
        files.sort();

        files.into_iter().map(read_profile).collect()
    }

    fn migrate_legacy_custom_profiles(&self) -> Result<()> {
        let legacy_path = self.legacy_custom_path();
        if !legacy_path.exists() {
            return Ok(());
        }

        let content = std::fs::read_to_string(&legacy_path)?;
        let legacy: LegacyIntentCatalog =
            serde_json::from_str(&content).map_err(|error| WorkonError::IntentContext {
                message: format!(
                    "invalid legacy intent catalog at {}: {error}",
                    legacy_path.display()
                ),
            })?;

        let directory = self.custom_path();
        std::fs::create_dir_all(&directory)?;

        for stored in legacy.intents {
            if stored.archived {
                continue;
            }

            let profile = normalize_profile(stored.profile)?;
            let path = directory.join(format!("{}.yaml", profile.id));
            if path.exists() {
                continue;
            }

            write_profile(path, &profile)?;
        }

        Ok(())
    }

    fn custom_path(&self) -> PathBuf {
        self.root
            .join(".workon")
            .join("intents")
            .join(CUSTOM_INTENT_DIR)
    }

    fn default_path(&self) -> PathBuf {
        self.root
            .join(".workon")
            .join("intents")
            .join(DEFAULT_INTENT_DIR)
    }

    fn legacy_custom_path(&self) -> PathBuf {
        self.root
            .join(".workon")
            .join("intents")
            .join(LEGACY_CUSTOM_INTENT_FILE)
    }
}

#[derive(Debug, Deserialize)]
struct LegacyIntentCatalog {
    intents: Vec<LegacyStoredIntent>,
}

#[derive(Debug, Deserialize)]
struct LegacyStoredIntent {
    #[serde(flatten)]
    profile: IntentProfile,
    archived: bool,
}

fn is_yaml_file(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension == "yaml" || extension == "yml")
}

fn read_profile(path: PathBuf) -> Result<IntentProfile> {
    let content = std::fs::read_to_string(&path)?;
    let profile = serde_yml::from_str(&content).map_err(|error| WorkonError::IntentContext {
        message: format!("invalid intent file at {}: {error}", path.display()),
    })?;
    normalize_profile(profile)
}

fn write_profile(path: PathBuf, profile: &IntentProfile) -> Result<()> {
    let content = serde_yml::to_string(profile).map_err(|error| WorkonError::IntentContext {
        message: format!("could not write intent file at {}: {error}", path.display()),
    })?;
    std::fs::write(path, content)?;
    Ok(())
}

fn normalize_profile(profile: IntentProfile) -> Result<IntentProfile> {
    let id = profile.id.trim().to_ascii_lowercase();
    validate_intent_id(&id)?;
    let name = required_text("intent name", &profile.name)?;
    let summary = required_text("intent summary", &profile.summary)?;

    Ok(IntentProfile {
        id,
        name,
        summary,
        skill_weights: clean_list(profile.skill_weights),
        mcp_weights: clean_list(profile.mcp_weights),
        instructions: clean_list(profile.instructions),
    })
}

fn validate_intent_id(id: &str) -> Result<()> {
    if id.is_empty() {
        return Err(WorkonError::IntentContext {
            message: "intent id cannot be empty".to_string(),
        });
    }

    let valid = id.chars().all(|character| {
        character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
    }) && !id.starts_with('-')
        && !id.ends_with('-')
        && !id.contains("--");

    if !valid {
        return Err(WorkonError::IntentContext {
            message: format!(
                "intent id must use lowercase letters, numbers, and single hyphens: `{id}`"
            ),
        });
    }

    Ok(())
}

fn required_text(label: &str, value: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(WorkonError::IntentContext {
            message: format!("{label} cannot be empty"),
        });
    }
    Ok(value.to_string())
}

fn clean_list(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}
