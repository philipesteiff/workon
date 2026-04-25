use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use crate::application::CommandOutput;
use crate::domain::WorkList;
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
        CommandOutput::Context(context) => {
            writeln!(writer, "context: {}", context.status)?;
            writeln!(writer, "{}", context.message)?;
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
    }
    Ok(())
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
  wo repos list <work-query>
                             list attached GitHub repos
  wo repos add <work> <owner/repo>...
                             attach GitHub repos as worktrees
  wo repos remove [--force] <work> <owner/repo>...
                             remove attached repo worktrees
  wo repos                   show GitHub repo context help
  wo --intent <id> \"<goal>\"  create work
  wo ctx                     print context status
  wo install-shell           install folder switching

Options:
  -i, --intent <id>          intent for new work
  --machine                  emit shell switch signals
  -h, --help                 show this help

Env:
  WORKON_ROOT=/path          override the default ~/.workon root
",
    )?;
    Ok(())
}

pub(crate) fn write_repos_help(writer: &mut dyn Write) -> Result<()> {
    writer.write_all(
        b"wo repos - attach GitHub repositories to a Work as git worktrees

Usage:
  wo repos list <work-query>
      List GitHub repositories attached to the matching Work.

  wo repos add <work-query> <owner/repo>...
      Attach one or more GitHub repositories to the matching Work.
      Repositories are cached as bare repos, then checked out under:
      <work>/repos/<owner>__<repo>

  wo repos remove [--force] <work-query> <owner/repo>...
      Remove one or more attached repository worktrees.
      Branches and bare repo caches are kept.
      Use --force only when you accept losing dirty worktree changes.

TUI:
  wo
      Highlight a Work, press /r, use the left GitHub catalog and right selected panel,
      press ! to arm force removal, then press enter to apply pending changes.

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
