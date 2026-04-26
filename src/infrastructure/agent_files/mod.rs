use std::fs;
use std::path::Path;

use crate::domain::{AttachedRepository, IntentProfile};
use crate::shared::error::{Result, WorkonError};

mod repository_context;

pub(crate) use repository_context::{AgentRepoContextFileWriter, RepoContextFileWriter};

const CONTEXT_BLOCK_START: &str = "<!-- wo:context:begin -->";
const CONTEXT_BLOCK_END: &str = "<!-- wo:context:end -->";
const LEGACY_INTENT_BLOCK_START: &str = "<!-- wo:intent:begin -->";
const LEGACY_INTENT_BLOCK_END: &str = "<!-- wo:intent:end -->";

#[derive(Debug, Default)]
struct PreservedAgentEdits {
    instruction_additions: Vec<String>,
    suffix: String,
}

pub fn write_agent_files(work_path: &Path, goal: &str, intent: &IntentProfile) -> Result<()> {
    write_agent_files_with_repos(work_path, goal, intent, &[])
}

pub fn write_agent_files_with_repos(
    work_path: &Path,
    goal: &str,
    intent: &IntentProfile,
    repositories: &[AttachedRepository],
) -> Result<()> {
    write_agent_files_with_repos_replacing_intent(work_path, goal, intent, repositories, intent)
}

pub(crate) fn write_agent_files_with_repos_replacing_intent(
    work_path: &Path,
    goal: &str,
    intent: &IntentProfile,
    repositories: &[AttachedRepository],
    previous_intent: &IntentProfile,
) -> Result<()> {
    let agents_path = work_path.join("AGENTS.md");
    let claude_path = work_path.join("CLAUDE.md");
    let portable_edits = preserved_agent_edits(&agents_path, "AGENTS.md", previous_intent)?;
    let claude_edits = preserved_agent_edits(&claude_path, "CLAUDE.md", previous_intent)?;
    let portable = render_portable_instructions(goal, intent, repositories, &portable_edits);
    let claude = render_claude_instructions(goal, intent, repositories, &claude_edits);

    fs::write(agents_path, portable)?;
    fs::write(claude_path, claude)?;

    Ok(())
}

pub(crate) fn validate_agent_files_context_blocks(work_path: &Path) -> Result<()> {
    validate_agent_file_context_block(&work_path.join("AGENTS.md"), "AGENTS.md")?;
    validate_agent_file_context_block(&work_path.join("CLAUDE.md"), "CLAUDE.md")
}

fn validate_agent_file_context_block(path: &Path, label: &str) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(path)?;
    context_block_bounds(&content, label)?;
    legacy_intent_block_bounds(&content, label)?;
    Ok(())
}

fn render_portable_instructions(
    goal: &str,
    intent: &IntentProfile,
    repositories: &[AttachedRepository],
    preserved_edits: &PreservedAgentEdits,
) -> String {
    render_agent_file("AGENTS", goal, intent, repositories, preserved_edits)
}

fn render_claude_instructions(
    goal: &str,
    intent: &IntentProfile,
    repositories: &[AttachedRepository],
    preserved_edits: &PreservedAgentEdits,
) -> String {
    render_agent_file("CLAUDE", goal, intent, repositories, preserved_edits)
}

fn render_agent_file(
    agent_name: &str,
    goal: &str,
    intent: &IntentProfile,
    repositories: &[AttachedRepository],
    preserved_edits: &PreservedAgentEdits,
) -> String {
    let suffix = if preserved_edits.suffix.is_empty() {
        "\n"
    } else {
        preserved_edits.suffix.as_str()
    };
    format!(
        "{CONTEXT_BLOCK_START}\n{}{CONTEXT_BLOCK_END}{suffix}",
        render_agent_file_body(agent_name, goal, intent, repositories, preserved_edits)
    )
}

fn render_agent_file_body(
    agent_name: &str,
    goal: &str,
    intent: &IntentProfile,
    repositories: &[AttachedRepository],
    preserved_edits: &PreservedAgentEdits,
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
        instruction_list(intent, &preserved_edits.instruction_additions)
    )
}

fn preserved_agent_edits(
    path: &Path,
    label: &str,
    previous_intent: &IntentProfile,
) -> Result<PreservedAgentEdits> {
    if !path.exists() {
        return Ok(PreservedAgentEdits::default());
    }

    let content = fs::read_to_string(path)?;
    if let Some((start, end)) = context_block_bounds(&content, label)? {
        let context = &content[start + CONTEXT_BLOCK_START.len()..end];
        return Ok(PreservedAgentEdits {
            instruction_additions: instruction_additions(context, previous_intent),
            suffix: content[end + CONTEXT_BLOCK_END.len()..].to_string(),
        });
    }

    legacy_intent_block_bounds(&content, label)?;
    Ok(PreservedAgentEdits {
        instruction_additions: instruction_additions(&content, previous_intent),
        suffix: String::new(),
    })
}

fn context_block_bounds(content: &str, label: &str) -> Result<Option<(usize, usize)>> {
    block_bounds(
        content,
        label,
        CONTEXT_BLOCK_START,
        CONTEXT_BLOCK_END,
        "wo:context",
    )
}

fn legacy_intent_block_bounds(content: &str, label: &str) -> Result<Option<(usize, usize)>> {
    block_bounds(
        content,
        label,
        LEGACY_INTENT_BLOCK_START,
        LEGACY_INTENT_BLOCK_END,
        "wo:intent",
    )
}

fn block_bounds(
    content: &str,
    label: &str,
    start_marker: &str,
    end_marker: &str,
    marker_label: &str,
) -> Result<Option<(usize, usize)>> {
    let start_count = content.matches(start_marker).count();
    let end_count = content.matches(end_marker).count();

    if start_count == 0 && end_count == 0 {
        return Ok(None);
    }

    if start_count > 1 || end_count > 1 {
        return Err(WorkonError::IntentContext {
            message: format!("{label} contains multiple {marker_label} blocks"),
        });
    }

    if start_count != 1 || end_count != 1 {
        return Err(WorkonError::IntentContext {
            message: format!("{label} contains an incomplete {marker_label} block"),
        });
    }

    let start = content.find(start_marker).expect("start marker exists");
    let end = content.find(end_marker).expect("end marker exists");
    if start > end {
        return Err(WorkonError::IntentContext {
            message: format!("{label} contains an incomplete {marker_label} block"),
        });
    }

    Ok(Some((start, end)))
}

fn instruction_additions(content: &str, previous_intent: &IntentProfile) -> Vec<String> {
    let Some(section) = instructions_section(content) else {
        return Vec::new();
    };
    let mut defaults = previous_intent.instructions.clone();
    let mut additions = Vec::new();

    for line in section.lines() {
        let trimmed = line.trim();
        if trimmed == LEGACY_INTENT_BLOCK_START || trimmed == LEGACY_INTENT_BLOCK_END {
            continue;
        }

        let Some(item) = bullet_item(trimmed) else {
            continue;
        };

        if let Some(index) = defaults.iter().position(|default| default == item) {
            defaults.remove(index);
        } else {
            additions.push(item.to_string());
        }
    }

    additions
}

fn instructions_section(content: &str) -> Option<&str> {
    let heading_start = content.find("## Instructions")?;
    let after_heading = &content[heading_start + "## Instructions".len()..];
    let section = after_heading
        .strip_prefix("\r\n\r\n")
        .or_else(|| after_heading.strip_prefix("\n\n"))
        .or_else(|| after_heading.strip_prefix("\r\n"))
        .or_else(|| after_heading.strip_prefix('\n'))
        .unwrap_or(after_heading);

    if let Some(next_heading) = section.find("\n## ") {
        Some(&section[..next_heading])
    } else {
        Some(section)
    }
}

fn bullet_item(line: &str) -> Option<&str> {
    line.strip_prefix("- ")
}

fn instruction_list(intent: &IntentProfile, instruction_additions: &[String]) -> String {
    let mut instructions = intent.instructions.clone();
    instructions.extend(instruction_additions.iter().cloned());
    bullet_list(&instructions)
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
