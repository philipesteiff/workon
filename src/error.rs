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
                    .map(|work| format!("{} ({})", work.slug, work.title))
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(formatter, "work query `{query}` is ambiguous: {options}")
            }
            Self::EmptyGoal => write!(formatter, "work goal cannot be empty"),
            Self::IntentRequired { available } => write!(
                formatter,
                "intent required. available intents: {}",
                available.join(", ")
            ),
            Self::Io(error) => write!(formatter, "io error: {error}"),
            Self::MissingArgument { message } => write!(formatter, "{message}"),
            Self::UnknownIntent {
                intent_id,
                available,
            } => write!(
                formatter,
                "unknown intent `{intent_id}`. available intents: {}",
                available.join(", ")
            ),
            Self::WorkNotFound { query } => write!(formatter, "work not found for `{query}`"),
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
