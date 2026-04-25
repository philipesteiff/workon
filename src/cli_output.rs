use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use crate::app::CommandOutput;
use crate::domain::WorkList;
use crate::error::Result;

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

    let label = if list.works.len() == 1 {
        "active work"
    } else {
        "active works"
    };
    writeln!(writer, "{} {label}", list.works.len())?;
    writeln!(writer)?;
    for (index, work) in list.works.iter().enumerate() {
        if index > 0 {
            writeln!(writer)?;
        }
        writeln!(writer, "{}. {}", index + 1, work.title)?;
        writeln!(writer, "   intent  {}", work.intent_id)?;
        writeln!(writer, "   slug    {}", work.slug)?;
        writeln!(writer, "   folder  {}", display_folder(&work.path, root))?;
    }

    Ok(())
}

fn display_folder(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

pub(crate) fn write_help(writer: &mut dyn Write) -> Result<()> {
    writeln!(
        writer,
        "wo - start, open, and archive work folders\n\
         \n\
         Usage:\n\
           wo                         list active work\n\
           wo list                    list active work\n\
           wo <work-query>            open matching work\n\
           wo archive <work-query>    archive matching work\n\
           wo --intent <id> \"<goal>\"  create work\n\
           wo ctx                     print context status\n\
           wo install-shell           install folder switching\n\
         \n\
         Options:\n\
           -i, --intent <id>          intent for new work\n\
           --machine                  emit shell switch signals\n\
           -h, --help                 show this help\n\
         \n\
         Env:\n\
           WORKON_ROOT=/path          override the default ~/.workon root"
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
    if machine {
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
