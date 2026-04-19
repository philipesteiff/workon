use std::io::Write;
use std::path::Path;

use crate::app::CommandOutput;
use crate::error::Result;

pub(crate) fn render_output(
    output: &CommandOutput,
    writer: &mut dyn Write,
    machine: bool,
) -> Result<()> {
    match output {
        CommandOutput::Context(context) => {
            writeln!(writer, "context: {}", context.status)?;
            writeln!(writer, "{}", context.message)?;
        }
        CommandOutput::WorkCreated(work) => {
            writeln!(writer, "intent selected: {}", work.intent_id)?;
            writeln!(writer, "work created: {}", work.title)?;
            writeln!(writer, "folder created: {}", work.path.display())?;
            writeln!(writer, "created: AGENTS.md")?;
            writeln!(writer, "created: CLAUDE.md")?;
            render_switch_signal(writer, &work.path, &work.title, machine)?;
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
            render_switch_signal(writer, &work.path, &work.title, machine)?;
        }
    }
    Ok(())
}

pub(crate) fn write_help(writer: &mut dyn Write) -> Result<()> {
    writeln!(
        writer,
        "wo\n\
         wo <work-query>\n\
         wo --intent <intent-id> \"<goal>\"\n\
         wo ctx"
    )?;
    Ok(())
}

fn render_switch_signal(
    writer: &mut dyn Write,
    path: &Path,
    title: &str,
    machine: bool,
) -> Result<()> {
    if machine {
        writeln!(writer, "__WORKON_SWITCH_PATH={}", path.display())?;
        writeln!(writer, "__WORKON_SWITCH_TITLE={title}")?;
    }
    Ok(())
}
