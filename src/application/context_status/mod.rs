use crate::application::CommandOutput;
use crate::domain::ContextStatus;
use crate::shared::error::Result;

pub(crate) fn execute() -> Result<CommandOutput> {
    Ok(CommandOutput::Context(ContextStatus {
        status: "TBD".to_string(),
        message:
            "Context editing will change intent, skills, MCPs, and repos for the current work."
                .to_string(),
    }))
}
