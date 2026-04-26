use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use crate::application::CommandOutput;
use crate::domain::{IntentProfileChange, IntentSource, WorkList};
use crate::shared::error::Result;

pub(crate) fn render_output(
    output: &CommandOutput,
    writer: &mut dyn Write,
    machine: bool,
    root: &Path,
) -> Result<()> {
    match output {
        CommandOutput::WorkArchived(work) => {
            writeln!(writer, "work archived: {}", work.title)?;
            writeln!(writer, "slug: {}", work.slug)?;
            writeln!(writer, "from: {}", work.path.display())?;
            writeln!(writer, "to: {}", work.archive_path.display())?;
            writeln!(writer, "next: wo list")?;
        }
        CommandOutput::IntentArchived(change) => {
            writeln!(writer, "intent archived: {}", change.intent.id)?;
            writeln!(writer, "next: wo intent list")?;
        }
        CommandOutput::IntentCreated(change) => {
            writeln!(writer, "intent created: {}", change.intent.id)?;
            render_intent_profile(writer, change)?;
        }
        CommandOutput::IntentDuplicated(change) => {
            writeln!(writer, "intent duplicated: {}", change.intent.id)?;
            render_intent_profile(writer, change)?;
        }
        CommandOutput::IntentList(list) => {
            writeln!(writer, "intents:")?;
            for intent in &list.intents {
                writeln!(
                    writer,
                    "{}  {}  {}  {}",
                    intent.id,
                    source_label(intent.source),
                    intent.name,
                    intent.summary
                )?;
            }
        }
        CommandOutput::IntentShown(change) => {
            render_intent_profile(writer, change)?;
        }
        CommandOutput::IntentUpdated(change) => {
            writeln!(writer, "intent updated: {}", change.intent.id)?;
            render_intent_profile(writer, change)?;
        }
        CommandOutput::RepositoryCatalog(catalog) => {
            for repository in &catalog.repositories {
                writeln!(
                    writer,
                    "{}  {}  {}",
                    repository.name_with_owner, repository.default_branch, repository.url
                )?;
            }
        }
        CommandOutput::RepositoryCandidates(candidates) => {
            writeln!(
                writer,
                "repository candidates for work: {}",
                candidates.work.title
            )?;
            if candidates.candidates.is_empty() {
                writeln!(
                    writer,
                    "No repository candidates found in configured workspaces."
                )?;
            } else {
                for candidate in &candidates.candidates {
                    writeln!(
                        writer,
                        "{}  {}  {}",
                        candidate.name_with_owner,
                        candidate.branch,
                        candidate.path.display()
                    )?;
                }
            }
        }
        CommandOutput::RepositoryWorkspaces(list) => {
            if list.workspaces.is_empty() {
                writeln!(writer, "No repository workspaces configured.")?;
                writeln!(writer, "Add one with: wo repos workspace add <path>...")?;
            } else {
                writeln!(writer, "repository workspaces:")?;
                for workspace in &list.workspaces {
                    writeln!(writer, "{}", workspace.path.display())?;
                }
            }
        }
        CommandOutput::ShellInstalled {
            label,
            script_path,
            zshrc_path,
        } => {
            writeln!(writer, "{label}: {}", script_path.display())?;
            writeln!(writer, "zshrc updated: {}", zshrc_path.display())?;
            writeln!(writer, "restart your shell or run:")?;
            writeln!(writer, "  source {}", script_path.display())?;
        }
        CommandOutput::WorkCreated(work) => {
            writeln!(writer, "work created: {}", work.title)?;
            writeln!(writer, "intent: {}", work.intent_id)?;
            writeln!(writer, "slug: {}", work.slug)?;
            writeln!(writer, "folder created: {}", work.path.display())?;
            writeln!(writer, "files: AGENTS.md, CLAUDE.md")?;
            writeln!(writer, "next: work from the folder")?;
            render_switch_signal(writer, &work.path, &work.title, root, machine)?;
        }
        CommandOutput::WorkList(list) => {
            render_work_list(list, writer, root)?;
        }
        CommandOutput::WorkOpened(work) => {
            writeln!(writer, "work opened: {}", work.title)?;
            writeln!(writer, "slug: {}", work.slug)?;
            writeln!(writer, "path: {}", work.path.display())?;
            writeln!(writer, "next: work from the folder")?;
            render_switch_signal(writer, &work.path, &work.title, root, machine)?;
        }
        CommandOutput::WorkRepositories(list) => {
            writeln!(writer, "repositories for work: {}", list.work.title)?;
            if list.repositories.is_empty() {
                writeln!(writer, "No repositories attached.")?;
            } else {
                for repository in &list.repositories {
                    writeln!(
                        writer,
                        "{}  {}  {}",
                        repository.name_with_owner,
                        repository.branch,
                        repository.path.display()
                    )?;
                }
            }
        }
        CommandOutput::WorkRepositoriesAdded(change) => {
            writeln!(writer, "repositories added: {}", change.work.title)?;
            if change.repositories.is_empty() {
                writeln!(
                    writer,
                    "No repository changes. Requested repositories were already attached."
                )?;
            } else {
                for repository in &change.repositories {
                    writeln!(
                        writer,
                        "{}  {}  {}",
                        repository.name_with_owner,
                        repository.branch,
                        repository.path.display()
                    )?;
                }
            }
        }
        CommandOutput::WorkRepositoriesRemoved(change) => {
            writeln!(writer, "repositories removed: {}", change.work.title)?;
            if change.repositories.is_empty() {
                writeln!(writer, "No repository changes.")?;
            } else {
                for repository in &change.repositories {
                    writeln!(
                        writer,
                        "{}  {}  {}",
                        repository.name_with_owner,
                        repository.branch,
                        repository.path.display()
                    )?;
                }
            }
        }
        CommandOutput::WorkIntentSwitched(change) => {
            writeln!(writer, "work intent switched: {}", change.work.title)?;
            writeln!(writer, "from: {}", change.previous_intent_id)?;
            writeln!(writer, "to: {}", change.intent.id)?;
            writeln!(writer, "files: AGENTS.md, CLAUDE.md")?;
        }
    }
    Ok(())
}

fn render_intent_profile(writer: &mut dyn Write, change: &IntentProfileChange) -> Result<()> {
    writeln!(writer, "id: {}", change.intent.id)?;
    writeln!(writer, "source: {}", source_label(change.source))?;
    writeln!(writer, "name: {}", change.intent.name)?;
    writeln!(writer, "summary: {}", change.intent.summary)?;
    writeln!(
        writer,
        "skills: {}",
        list_label(&change.intent.skill_weights)
    )?;
    writeln!(writer, "mcps: {}", list_label(&change.intent.mcp_weights))?;
    writeln!(
        writer,
        "instructions: {}",
        list_label(&change.intent.instructions)
    )?;
    Ok(())
}

fn source_label(source: IntentSource) -> &'static str {
    match source {
        IntentSource::BuiltIn => "built-in",
        IntentSource::Custom => "custom",
    }
}

fn list_label(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values.join(" | ")
    }
}

fn render_work_list(list: &WorkList, writer: &mut dyn Write, root: &Path) -> Result<()> {
    if list.works.is_empty() {
        writeln!(writer, "No active work.")?;
        writeln!(writer, "Next: wo --intent <intent-id> \"<goal>\"")?;
        writeln!(writer, "Try:  wo --help")?;
        return Ok(());
    }

    let rows: Vec<WorkListRow> = list
        .works
        .iter()
        .map(|work| WorkListRow {
            title: work.title.clone(),
            intent_id: work.intent_id.clone(),
            slug: work.slug.clone(),
            path: display_folder(&work.path, root),
            goal: work.goal.clone(),
        })
        .collect();

    let widths = WorkListColumnWidths::from_rows(&rows);
    writeln!(
        writer,
        "  {:<title_width$}  {:<intent_width$}  {:<slug_width$}  {:<path_width$}  Goal",
        "Work",
        "Intent",
        "Slug",
        "Path",
        title_width = widths.title,
        intent_width = widths.intent_id,
        slug_width = widths.slug,
        path_width = widths.path,
    )?;

    for row in rows {
        writeln!(
            writer,
            "  {:<title_width$}  {:<intent_width$}  {:<slug_width$}  {:<path_width$}  {}",
            row.title,
            row.intent_id,
            row.slug,
            row.path,
            row.goal,
            title_width = widths.title,
            intent_width = widths.intent_id,
            slug_width = widths.slug,
            path_width = widths.path,
        )?;
    }

    Ok(())
}

struct WorkListRow {
    title: String,
    intent_id: String,
    slug: String,
    path: String,
    goal: String,
}

struct WorkListColumnWidths {
    title: usize,
    intent_id: usize,
    slug: usize,
    path: usize,
}

impl WorkListColumnWidths {
    fn from_rows(rows: &[WorkListRow]) -> Self {
        let mut widths = Self {
            title: "Work".len(),
            intent_id: "Intent".len(),
            slug: "Slug".len(),
            path: "Path".len(),
        };

        for row in rows {
            widths.title = widths.title.max(row.title.len());
            widths.intent_id = widths.intent_id.max(row.intent_id.len());
            widths.slug = widths.slug.max(row.slug.len());
            widths.path = widths.path.max(row.path.len());
        }

        widths
    }
}

fn display_folder(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

pub(crate) fn write_help(writer: &mut dyn Write) -> Result<()> {
    writer.write_all(
        b"wo - start, open, and archive work folders

Usage:
  wo                         list active work
  wo list                    list active work
  wo <work-query>            open matching work
  wo archive <work-query>    archive matching work
  wo intent list             list reusable intent profiles
  wo intent switch <work> <intent>
                             switch current intent for matching Work
  wo intent new <id> --name <name> --summary <summary>
                             create a reusable custom intent
  wo intent edit <id>        update a custom intent
  wo intent                  show intent workflow help
  wo repos list <work-query>
                             list attached repositories
  wo repos discover <work-query>
                             discover repo candidates from configured workspaces
  wo repos add [--workspace <path>] <work> <owner/repo>...
                             create/link GitHub repos from a configured workspace
  wo repos link <work> <path>...
                             link existing repo worktrees or checkouts
  wo repos workspace add <path>...
                             configure repo workspaces for discovery and creation
  wo repos workspace list|remove
                             manage configured repo workspaces
  wo repos remove [--force] <work> <owner/repo>...
                             remove attached repo links
  wo repos                   show GitHub repo context help
  wo --intent <id> \"<goal>\"  create work
  wo install-shell           install folder switching
  wo version                 print version

Options:
  -i, --intent <id>          intent for new work
  --machine                  emit shell switch signals
  -h, --help                 show this help
  -V, --version              print version

Env:
  WORKON_ROOT=/path          override the default ~/.workon root
",
    )?;
    Ok(())
}

pub(crate) fn write_version(writer: &mut dyn Write) -> Result<()> {
    writeln!(writer, "wo {}", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}

pub(crate) fn write_intent_help(writer: &mut dyn Write) -> Result<()> {
    writer.write_all(
        b"wo intent - define and switch reusable Work intentions

Usage:
  wo intent list
      List built-in and custom intent profiles.

  wo intent show <intent-id>
      Show one intent profile.

  wo intent switch <work-query> <intent-id>
      Set the current intent for the matching Work and rewrite agent files.

  wo intent new <intent-id> --name <name> --summary <summary>
      Create a custom reusable intent.

  wo intent edit <intent-id> [--name <name>] [--summary <summary>]
      Edit a custom intent. Built-in intents are read-only.

  wo intent duplicate <source-intent-id> <new-intent-id> [--name <name>]
      Copy a built-in or custom intent into an editable custom intent.

  wo intent archive <intent-id>
      Hide a custom intent from the active catalog. Built-ins cannot be archived.

List fields:
  --skill <skill>            Add one preferred skill. Repeat as needed.
  --mcp <mcp>                Add one preferred MCP. Repeat as needed.
  --instruction <text>       Add one instruction. Repeat as needed.

Edit clearing:
  --clear-skills
  --clear-mcps
  --clear-instructions

TUI:
  wo
      Highlight a Work, press /i, filter intents, then press enter to switch.
",
    )?;
    Ok(())
}

pub(crate) fn write_repos_help(writer: &mut dyn Write) -> Result<()> {
    writer.write_all(
        b"wo repos - link repositories to a Work

Usage:
  wo repos list <work-query>
      List repositories attached to the matching Work.

  wo repos discover <work-query>
      Discover Git working trees under configured repo workspaces.

  wo repos add [--workspace <path>] <work-query> <owner/repo>...
      Create one or more GitHub repositories in a configured repo workspace,
      then expose them under <work>/repos/ with symlinks.

  wo repos link <work-query> <path>...
      Link existing Git working trees or worktrees to the matching Work.

  wo repos workspace list
      List folders Workon may scan and create repos in.

  wo repos workspace add <path>...
      Add one or more repo workspaces. Workon creates repos only inside configured workspaces.

  wo repos workspace remove <path>
      Remove a repo workspace from discovery and creation.

  wo repos remove [--force] <work-query> <owner/repo>...
      Remove one or more attached repository links.
      The underlying checkout or worktree is kept.

TUI:
  wo
      Highlight a Work, press /r, select GitHub or local repositories, tab to
      choose the creation path when multiple workspaces exist, then press enter.

Requires:
  gh auth login
",
    )?;
    Ok(())
}

fn render_switch_signal(
    writer: &mut dyn Write,
    path: &Path,
    title: &str,
    root: &Path,
    machine: bool,
) -> Result<()> {
    if machine || std::env::var_os("WORKON_SIGNAL_FILE").is_some() {
        write_machine_signal(writer, path, title, root)?;
    }
    Ok(())
}

fn write_machine_signal(
    writer: &mut dyn Write,
    path: &Path,
    title: &str,
    root: &Path,
) -> Result<()> {
    match std::env::var_os("WORKON_SIGNAL_FILE") {
        Some(signal_file) => {
            let mut signal_writer = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(PathBuf::from(signal_file))?;
            write_switch_lines(&mut signal_writer, path, title, root)?;
        }
        None => write_switch_lines(writer, path, title, root)?,
    }

    Ok(())
}

fn write_switch_lines(writer: &mut dyn Write, path: &Path, title: &str, root: &Path) -> Result<()> {
    writeln!(writer, "__WORKON_CD={}", path.display())?;
    writeln!(writer, "__WORKON_ROOT={}", root.display())?;
    writeln!(writer, "__WORKON_TITLE={title}")?;
    Ok(())
}
