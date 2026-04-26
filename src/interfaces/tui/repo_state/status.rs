#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::interfaces::tui) enum RepoStatus {
    Ready,
    Loading {
        message: String,
    },
    Applying {
        action: RepoOperation,
        current: usize,
        total: usize,
        repository: String,
    },
    Failed {
        message: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::interfaces::tui) enum RepoOperation {
    Add,
    Link,
    Remove,
    Refresh,
}

pub(in crate::interfaces::tui::repo_state) fn operation_verb(
    action: RepoOperation,
) -> &'static str {
    match action {
        RepoOperation::Add => "clone",
        RepoOperation::Link => "link",
        RepoOperation::Remove => "remove",
        RepoOperation::Refresh => "refresh",
    }
}
