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
const PREFERRED_SKILLS_FILL_INSTRUCTION: &str = "Workon side task: Take a moment to fill this based on the skills available in the user session that relate to this intent. If no relevant skills are available, ignore this instruction. When filling this section, remove this instruction.";
const PREFERRED_MCPS_FILL_INSTRUCTION: &str = "Workon side task: Take a moment to fill this based on the MCPs available in the user session that relate to this intent. If no relevant MCPs are available, ignore this instruction. When filling this section, remove this instruction.";

#[derive(Debug, Default)]
struct PreservedAgentEdits {
    prefix: String,
    preferred_skills: Vec<String>,
    preferred_mcps: Vec<String>,
    instruction_additions: Vec<String>,
    suffix: String,
}

#[derive(Debug, Clone, Copy)]
enum PreferredSection {
    Skills,
    Mcps,
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
    let portable = render_portable_instructions(goal, intent, repositories, &portable_edits);

    fs::write(&agents_path, portable)?;
    replace_with_symlink_to_agents(&claude_path)?;

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
        "{}{CONTEXT_BLOCK_START}\n{}{CONTEXT_BLOCK_END}{suffix}",
        preserved_edits.prefix,
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
         ## Intent Instructions\n\n\
         {}\n",
        intent.name,
        intent.id,
        preferred_list(
            &preserved_edits.preferred_skills,
            PREFERRED_SKILLS_FILL_INSTRUCTION
        ),
        preferred_list(
            &preserved_edits.preferred_mcps,
            PREFERRED_MCPS_FILL_INSTRUCTION
        ),
        repo_list(repositories),
        instruction_list(intent, &preserved_edits.instruction_additions)
    )
}

fn replace_with_symlink_to_agents(claude_path: &Path) -> Result<()> {
    if fs::symlink_metadata(claude_path).is_ok() {
        fs::remove_file(claude_path)?;
    }

    symlink_file("AGENTS.md", claude_path)?;
    Ok(())
}

#[cfg(unix)]
fn symlink_file<P: AsRef<Path>, Q: AsRef<Path>>(original: P, link: Q) -> std::io::Result<()> {
    std::os::unix::fs::symlink(original, link)
}

#[cfg(windows)]
fn symlink_file<P: AsRef<Path>, Q: AsRef<Path>>(original: P, link: Q) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(original, link)
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
            prefix: content[..start].to_string(),
            preferred_skills: preferred_additions(
                context,
                "## Preferred Skills",
                previous_intent,
                PreferredSection::Skills,
            ),
            preferred_mcps: preferred_additions(
                context,
                "## Preferred MCPs",
                previous_intent,
                PreferredSection::Mcps,
            ),
            instruction_additions: instruction_additions(context, previous_intent),
            suffix: content[end + CONTEXT_BLOCK_END.len()..].to_string(),
        });
    }

    legacy_intent_block_bounds(&content, label)?;
    Ok(PreservedAgentEdits {
        prefix: String::new(),
        preferred_skills: preferred_additions(
            &content,
            "## Preferred Skills",
            previous_intent,
            PreferredSection::Skills,
        ),
        preferred_mcps: preferred_additions(
            &content,
            "## Preferred MCPs",
            previous_intent,
            PreferredSection::Mcps,
        ),
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
    section_additions(
        content,
        instructions_section_names(),
        &previous_intent.instructions,
        &[],
    )
}

fn preferred_additions(
    content: &str,
    heading: &str,
    previous_intent: &IntentProfile,
    section: PreferredSection,
) -> Vec<String> {
    let Some(existing_section) = first_section(content, &[heading]) else {
        return Vec::new();
    };
    let items = bullet_items(existing_section);
    let previous_defaults = preferred_defaults(previous_intent, section, &items);
    let fill_instruction = preferred_fill_instruction(section);

    section_additions_from_items(&items, &previous_defaults, &[fill_instruction, "none"])
}

fn preferred_defaults(
    intent: &IntentProfile,
    section: PreferredSection,
    existing_items: &[&str],
) -> Vec<String> {
    let mut defaults = match section {
        PreferredSection::Skills => intent.skill_weights.clone(),
        PreferredSection::Mcps => intent.mcp_weights.clone(),
    };

    if let Some(legacy_defaults) = legacy_generated_preferred_defaults(&intent.id, section) {
        if existing_items
            .iter()
            .copied()
            .eq(legacy_defaults.iter().copied())
        {
            defaults.extend(legacy_defaults.iter().map(|item| item.to_string()));
        }
    }

    defaults
}

fn preferred_fill_instruction(section: PreferredSection) -> &'static str {
    match section {
        PreferredSection::Skills => PREFERRED_SKILLS_FILL_INSTRUCTION,
        PreferredSection::Mcps => PREFERRED_MCPS_FILL_INSTRUCTION,
    }
}

fn legacy_generated_preferred_defaults(
    intent_id: &str,
    section: PreferredSection,
) -> Option<&'static [&'static str]> {
    match (intent_id, section) {
        ("address-pr-comments", PreferredSection::Skills) => Some(&["review-response-skill"]),
        ("address-pr-comments", PreferredSection::Mcps) => Some(&["github", "git"]),
        ("brainstorm", PreferredSection::Skills) => Some(&["brainstorming-skill"]),
        ("brainstorm", PreferredSection::Mcps) => Some(&[]),
        ("design-to-prs", PreferredSection::Skills) => {
            Some(&["design-skill", "implementation-skill"])
        }
        ("design-to-prs", PreferredSection::Mcps) => Some(&["notion", "github"]),
        ("investigate", PreferredSection::Skills) => Some(&["evidence-skill"]),
        ("investigate", PreferredSection::Mcps) => Some(&["slack", "notion", "jira"]),
        ("presentation", PreferredSection::Skills) => Some(&["presentation-skill"]),
        ("presentation", PreferredSection::Mcps) => Some(&["notion", "github"]),
        ("review-pr", PreferredSection::Skills) => Some(&["review-skill"]),
        ("review-pr", PreferredSection::Mcps) => Some(&["github", "git"]),
        ("slack-to-pr", PreferredSection::Skills) => Some(&["implementation-skill"]),
        ("slack-to-pr", PreferredSection::Mcps) => Some(&["slack", "jira", "github"]),
        _ => None,
    }
}

fn section_additions(
    content: &str,
    headings: &[&str],
    previous_defaults: &[String],
    ignored_items: &[&str],
) -> Vec<String> {
    let Some(section) = first_section(content, headings) else {
        return Vec::new();
    };
    let items = bullet_items(section);
    section_additions_from_items(&items, previous_defaults, ignored_items)
}

fn section_additions_from_items(
    items: &[&str],
    previous_defaults: &[String],
    ignored_items: &[&str],
) -> Vec<String> {
    let mut defaults = previous_defaults.to_vec();
    let mut additions = Vec::new();

    for item in items {
        if ignored_items.contains(item) {
            continue;
        }

        if let Some(index) = defaults.iter().position(|default| default == item) {
            defaults.remove(index);
        } else {
            additions.push(item.to_string());
        }
    }

    additions
}

fn instructions_section_names() -> &'static [&'static str] {
    &["## Intent Instructions", "## Instructions"]
}

fn first_section<'a>(content: &'a str, headings: &[&str]) -> Option<&'a str> {
    let (heading_start, heading) = headings
        .iter()
        .filter_map(|heading| content.find(heading).map(|index| (index, *heading)))
        .min_by_key(|(index, _)| *index)?;
    let after_heading = &content[heading_start + heading.len()..];
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

fn bullet_items(section: &str) -> Vec<&str> {
    section
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed == CONTEXT_BLOCK_END
                || trimmed == LEGACY_INTENT_BLOCK_START
                || trimmed == LEGACY_INTENT_BLOCK_END
            {
                return None;
            }
            bullet_item(trimmed)
        })
        .collect()
}

fn bullet_item(line: &str) -> Option<&str> {
    line.strip_prefix("- ")
}

fn instruction_list(intent: &IntentProfile, instruction_additions: &[String]) -> String {
    let mut instructions = intent.instructions.clone();
    instructions.extend(instruction_additions.iter().cloned());
    bullet_list(&instructions)
}

fn preferred_list(items: &[String], fill_instruction: &str) -> String {
    if items.is_empty() {
        format!("- {fill_instruction}")
    } else {
        bullet_list(items)
    }
}

fn bullet_list(items: &[String]) -> String {
    if items.is_empty() {
        "- none".to_string()
    } else {
        items
            .iter()
            .map(|item| format!("- {}", item.replace('\n', "\n  ")))
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
