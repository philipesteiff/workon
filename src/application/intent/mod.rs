use crate::application::command::{IntentProfileInput, IntentProfilePatch};
use crate::application::CommandOutput;
use crate::domain::{
    IntentCatalog, IntentList, IntentProfile, IntentProfileChange, IntentSource, WorkIntentSwitch,
    WorkSummary,
};
use crate::infrastructure::agent_files::{
    validate_agent_files_context_blocks, write_agent_files_with_repos_replacing_intent,
};
use crate::infrastructure::git_worktree::{GitWorktree, WorktreeInspector};
use crate::infrastructure::process::StdProcessRunner;
use crate::infrastructure::storage::WorkStore;
use crate::infrastructure::storage::{JsonIntentStore, JsonRepoMetadataStore, RepoMetadataStore};
use crate::shared::error::{Result, WorkonError};

pub(crate) fn list(intents: &IntentCatalog) -> Result<CommandOutput> {
    Ok(CommandOutput::IntentList(IntentList {
        intents: intents.summaries(),
    }))
}

pub(crate) fn show(intents: &IntentCatalog, intent_id: &str) -> Result<CommandOutput> {
    let Some(intent) = intents.find(intent_id) else {
        return Err(WorkonError::UnknownIntent {
            intent_id: intent_id.to_string(),
            available: intents.available_ids(),
        });
    };

    Ok(CommandOutput::IntentShown(IntentProfileChange {
        source: intents.source(intent_id).unwrap_or(IntentSource::BuiltIn),
        intent,
    }))
}

pub(crate) fn create(
    store: &JsonIntentStore,
    intents: &IntentCatalog,
    input: IntentProfileInput,
) -> Result<CommandOutput> {
    let profile = normalize_profile(input.into())?;
    ensure_new_custom_intent(intents, &profile.id)?;
    let intent = store.create(profile)?;

    Ok(CommandOutput::IntentCreated(IntentProfileChange {
        intent,
        source: IntentSource::Custom,
    }))
}

pub(crate) fn edit(
    store: &JsonIntentStore,
    intents: &IntentCatalog,
    intent_id: &str,
    patch: IntentProfilePatch,
) -> Result<CommandOutput> {
    ensure_custom_intent(intents, intent_id, "edited")?;
    let Some(mut profile) = store.find_active(intent_id)? else {
        return Err(WorkonError::UnknownIntent {
            intent_id: intent_id.to_string(),
            available: intents.available_ids(),
        });
    };

    apply_patch(&mut profile, patch);
    let profile = normalize_profile(profile)?;
    let intent = store.update(profile)?;
    Ok(CommandOutput::IntentUpdated(IntentProfileChange {
        intent,
        source: IntentSource::Custom,
    }))
}

pub(crate) fn duplicate(
    store: &JsonIntentStore,
    intents: &IntentCatalog,
    source_intent_id: &str,
    new_intent_id: &str,
    name: Option<String>,
) -> Result<CommandOutput> {
    let Some(mut profile) = intents.find(source_intent_id) else {
        return Err(WorkonError::UnknownIntent {
            intent_id: source_intent_id.to_string(),
            available: intents.available_ids(),
        });
    };
    profile.id = new_intent_id.to_string();
    if let Some(name) = name {
        profile.name = name;
    }

    let profile = normalize_profile(profile)?;
    ensure_new_custom_intent(intents, &profile.id)?;
    let intent = store.create(profile)?;
    Ok(CommandOutput::IntentDuplicated(IntentProfileChange {
        intent,
        source: IntentSource::Custom,
    }))
}

pub(crate) fn archive(
    store: &JsonIntentStore,
    intents: &IntentCatalog,
    intent_id: &str,
) -> Result<CommandOutput> {
    ensure_custom_intent(intents, intent_id, "archived")?;
    let intent = store.archive(intent_id)?;
    Ok(CommandOutput::IntentArchived(IntentProfileChange {
        intent,
        source: IntentSource::Custom,
    }))
}

pub(crate) fn switch_work_intent(
    store: &WorkStore,
    intents: &IntentCatalog,
    query: &str,
    intent_id: &str,
) -> Result<CommandOutput> {
    let Some(intent) = intents.find(intent_id) else {
        return Err(WorkonError::UnknownIntent {
            intent_id: intent_id.to_string(),
            available: intents.available_ids(),
        });
    };

    let current_work: WorkSummary = store.open(query)?.into();
    validate_agent_files_context_blocks(&current_work.path)?;

    let (previous_intent_id, work) = store.update_intent(query, &intent.id)?;
    let previous_intent = intents
        .find(&previous_intent_id)
        .unwrap_or_else(|| intent.clone());
    let metadata = JsonRepoMetadataStore;
    let runner = StdProcessRunner;
    let inspector = GitWorktree::new(&runner);
    let repository_metadata = metadata.read(&work.path)?;
    let repositories = inspector.scan(&work.path, &repository_metadata)?;
    write_agent_files_with_repos_replacing_intent(
        &work.path,
        &work.goal,
        &intent,
        &repositories,
        &previous_intent,
    )?;

    Ok(CommandOutput::WorkIntentSwitched(WorkIntentSwitch {
        work,
        previous_intent_id,
        intent,
    }))
}

fn apply_patch(profile: &mut IntentProfile, patch: IntentProfilePatch) {
    if let Some(name) = patch.name {
        profile.name = name;
    }
    if let Some(summary) = patch.summary {
        profile.summary = summary;
    }
    if let Some(skill_weights) = patch.skill_weights {
        profile.skill_weights = skill_weights;
    }
    if let Some(mcp_weights) = patch.mcp_weights {
        profile.mcp_weights = mcp_weights;
    }
    if let Some(instructions) = patch.instructions {
        profile.instructions = instructions;
    }
}

fn ensure_new_custom_intent(intents: &IntentCatalog, intent_id: &str) -> Result<()> {
    if intents.find(intent_id).is_some() {
        return Err(WorkonError::IntentContext {
            message: format!("intent already exists: `{intent_id}`"),
        });
    }
    Ok(())
}

fn ensure_custom_intent(intents: &IntentCatalog, intent_id: &str, action: &str) -> Result<()> {
    match intents.source(intent_id) {
        Some(IntentSource::Custom) => Ok(()),
        Some(IntentSource::BuiltIn) => Err(WorkonError::IntentContext {
            message: format!("built-in intent `{intent_id}` cannot be {action}"),
        }),
        None => Err(WorkonError::UnknownIntent {
            intent_id: intent_id.to_string(),
            available: intents.available_ids(),
        }),
    }
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
