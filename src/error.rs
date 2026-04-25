use std::fmt;

use crate::domain::AmbiguousWorkMatch;

pub type Result<T> = std::result::Result<T, WorkonError>;

#[derive(Debug)]
pub enum WorkonError {
    AmbiguousWork {
        query: String,
        matches: Vec<AmbiguousWorkMatch>,
    },
    EmptyGoal,
    IntentRequired {
        available: Vec<String>,
    },
    Io(std::io::Error),
    MissingArgument {
        message: String,
    },
    UnknownIntent {
        intent_id: String,
        available: Vec<String>,
    },
    WorkNotFound {
        query: String,
    },
}

impl fmt::Display for WorkonError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AmbiguousWork { query, matches } => {
                let options = matches
                    .iter()
                    .map(|work| format!("  wo {}  # {}", work.slug, work.title))
                    .collect::<Vec<_>>()
                    .join("\n");
                write!(
                    formatter,
                    "work query `{query}` matched multiple works.\nUse a slug:\n{options}"
                )
            }
            Self::EmptyGoal => write!(
                formatter,
                "work goal cannot be empty.\nUse: wo --intent <intent-id> \"<goal>\""
            ),
            Self::IntentRequired { available } => write!(
                formatter,
                "intent required for new work.\nUse: wo --intent <intent-id> \"<goal>\"\nAvailable: {}",
                available.join(", ")
            ),
            Self::Io(error) => write!(formatter, "io error: {error}"),
            Self::MissingArgument { message } => write!(formatter, "{message}"),
            Self::UnknownIntent {
                intent_id,
                available,
            } => write!(
                formatter,
                "unknown intent `{intent_id}`.\nAvailable: {}\nUse: wo --intent <intent-id> \"<goal>\"",
                available.join(", ")
            ),
            Self::WorkNotFound { query } => write!(
                formatter,
                "work not found: `{query}`.\nRun `wo list` or create it with `wo --intent <intent-id> \"{query}\"`."
            ),
        }
    }
}

impl std::error::Error for WorkonError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for WorkonError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}
