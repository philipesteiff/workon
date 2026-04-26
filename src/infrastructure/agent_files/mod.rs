use std::fs;
use std::path::Path;

use crate::domain::{AttachedRepository, IntentProfile};
use crate::shared::error::Result;

mod repository_context;

pub(crate) use repository_context::{AgentRepoContextFileWriter, RepoContextFileWriter};

pub fn write_agent_files(work_path: &Path, goal: &str, intent: &IntentProfile) -> Result<()> {
    write_agent_files_with_repos(work_path, goal, intent, &[])
}

pub fn write_agent_files_with_repos(
    work_path: &Path,
    goal: &str,
    intent: &IntentProfile,
    repositories: &[AttachedRepository],
) -> Result<()> {
    let portable = render_portable_instructions(goal, intent, repositories);
    let claude = render_claude_instructions(goal, intent, repositories);

    fs::write(work_path.join("AGENTS.md"), portable)?;
    fs::write(work_path.join("CLAUDE.md"), claude)?;

    Ok(())
}

fn render_portable_instructions(
    goal: &str,
    intent: &IntentProfile,
    repositories: &[AttachedRepository],
) -> String {
    render_agent_file("AGENTS", goal, intent, repositories)
}

fn render_claude_instructions(
    goal: &str,
    intent: &IntentProfile,
    repositories: &[AttachedRepository],
) -> String {
    render_agent_file("CLAUDE", goal, intent, repositories)
}

fn render_agent_file(
    agent_name: &str,
    goal: &str,
    intent: &IntentProfile,
    repositories: &[AttachedRepository],
) -> String {
    let source_note = if agent_name == "AGENTS" {
        "AGENTS.md is the source for this Work. Agent-specific files are projections from it."
            .to_string()
    } else {
        format!(
            "AGENTS.md is the source for this Work. This file is a projection for {agent_name}."
        )
    };

    format!(
        "# Work Context for {agent_name}\n\n\
         {source_note}\n\n\
         ## Goal\n\n\
         {goal}\n\n\
         ## Current Intent\n\n\
         {} ({})\n\n\
         Intent weights are preferences, not hard requirements.\n\n\
         ## Preferred Skills\n\n\
         {}\n\n\
         ## Preferred MCPs\n\n\
         {}\n\n\
         ## Repos\n\n\
         {}\n\n\
         ## How to Work\n\n\
         - Before acting, restate the goal, current intent, and any missing context that could change the answer.\n\
         - Treat Preferred Skills and Preferred MCPs as suggestions. Use them when available, and say when they are missing or not useful.\n\
         - Keep changes focused on the goal. Do not broaden product behavior without an explicit decision.\n\
         - Record decisions and evidence in plain language so the next agent or engineer can continue.\n\n\
         ## Next\n\n\
         - Inspect attached repos before editing.\n\
         - Check existing notes, evidence, and outputs in this Work folder.\n\
         - Leave a short handoff when you stop.\n\n\
         ## Instructions\n\n\
         {}\n",
        intent.name,
        intent.id,
        bullet_list(&intent.skill_weights),
        bullet_list(&intent.mcp_weights),
        repo_list(repositories),
        bullet_list(&intent.instructions)
    )
}

fn bullet_list(items: &[String]) -> String {
    if items.is_empty() {
        "- none".to_string()
    } else {
        items
            .iter()
            .map(|item| format!("- {item}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn repo_list(repositories: &[AttachedRepository]) -> String {
    if repositories.is_empty() {
        return "No GitHub repositories attached yet. Repos attached to this work belong in `repos/`. When present, treat them as context for this goal.".to_string();
    }

    repositories
        .iter()
        .map(|repository| {
            format!(
                "- {} (`{}`, branch `{}`)",
                repository.name_with_owner,
                repository.path.display(),
                repository.branch
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
