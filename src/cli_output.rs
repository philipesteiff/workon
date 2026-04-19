use std::io::Write;
use std::path::Path;

use crate::app::CommandOutput;
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
            writeln!(writer, "from: {}", work.path.display())?;
            writeln!(writer, "to: {}", work.archive_path.display())?;
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
            writeln!(writer, "intent selected: {}", work.intent_id)?;
            writeln!(writer, "work created: {}", work.title)?;
            writeln!(writer, "folder created: {}", work.path.display())?;
            writeln!(writer, "created: AGENTS.md")?;
            writeln!(writer, "created: CLAUDE.md")?;
            render_switch_signal(writer, &work.path, &work.title, root, machine)?;
        }
        CommandOutput::WorkList(list) => {
            if list.works.is_empty() {
                writeln!(writer, "no work yet")?;
            } else {
                for work in &list.works {
                    writeln!(writer, "{}\t{}", work.slug, work.title)?;
                }
            }
        }
        CommandOutput::WorkOpened(work) => {
            writeln!(writer, "work opened: {}", work.title)?;
            writeln!(writer, "path: {}", work.path.display())?;
            render_switch_signal(writer, &work.path, &work.title, root, machine)?;
        }
    }
    Ok(())
}

pub(crate) fn write_help(writer: &mut dyn Write) -> Result<()> {
    writeln!(
        writer,
        "wo\n\
         wo <work-query>\n\
         wo archive <work-query>\n\
         wo --intent <intent-id> \"<goal>\"\n\
         wo ctx\n\
         wo install-shell"
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
        writeln!(writer, "__WORKON_CD={}", path.display())?;
        writeln!(writer, "__WORKON_ROOT={}", root.display())?;
        writeln!(writer, "__WORKON_TITLE={title}")?;
    }
    Ok(())
}
