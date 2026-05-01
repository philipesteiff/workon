use ratatui::layout::{Constraint, Layout, Rect};

use super::components::TitleActivity;
use super::repo_state::{RepoOperation, RepoStatus};
use super::state::TuiMode;

const MAX_WIDTH: u16 = 160;
const SIDE_MARGIN: u16 = 2;
const BASE_HEIGHT: u16 = 28;
const TRACE_EXTRA_HEIGHT: u16 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TraceVisibility {
    Hidden,
    Visible,
}

impl From<bool> for TraceVisibility {
    fn from(visible: bool) -> Self {
        if visible {
            Self::Visible
        } else {
            Self::Hidden
        }
    }
}

pub(super) fn title(mode: TuiMode) -> &'static str {
    if mode == TuiMode::Repos {
        "WORKON // CONTROL // REPO"
    } else {
        "WORKON // CONTROL"
    }
}

pub(super) fn title_activity(
    mode: TuiMode,
    repo_status: &RepoStatus,
    repo_activity_frame: usize,
) -> Option<TitleActivity> {
    if mode != TuiMode::Repos {
        return None;
    }

    match repo_status {
        RepoStatus::Loading { message } => {
            Some(TitleActivity::loading(message.clone(), repo_activity_frame))
        }
        RepoStatus::Applying {
            action,
            current,
            total,
            repository,
        } => Some(TitleActivity::loading(
            repo_activity_message(*action, *current, *total, repository),
            repo_activity_frame,
        )),
        _ => None,
    }
}

pub(super) fn rect(area: Rect, trace_visibility: TraceVisibility) -> Rect {
    if area.width < 44 || area.height < 12 {
        return area;
    }

    let width = area.width.saturating_sub(SIDE_MARGIN).min(MAX_WIDTH);
    let trace_extra = if trace_visibility == TraceVisibility::Visible {
        TRACE_EXTRA_HEIGHT
    } else {
        0
    };
    let height = (BASE_HEIGHT + trace_extra).min(area.height.saturating_sub(2));

    centered_area(area, width, height)
}

fn repo_activity_message(
    action: RepoOperation,
    current: usize,
    total: usize,
    repository: &str,
) -> String {
    match action {
        RepoOperation::Add => format!("{current}/{total} Creating worktree {repository}"),
        RepoOperation::Link => format!("{current}/{total} Linking {repository}"),
        RepoOperation::Remove => format!("{current}/{total} Removing {repository}"),
        RepoOperation::Refresh => format!("{current}/{total} Refreshing {repository}"),
    }
}

fn centered_area(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    let [_, center, _] = Layout::vertical([
        Constraint::Length((area.height - height) / 2),
        Constraint::Length(height),
        Constraint::Fill(1),
    ])
    .areas(area);
    let [_, center, _] = Layout::horizontal([
        Constraint::Length((area.width - width) / 2),
        Constraint::Length(width),
        Constraint::Fill(1),
    ])
    .areas(center);

    center
}

#[cfg(test)]
mod tests {
    use ratatui::layout::Rect;

    use super::{rect, title, title_activity, TraceVisibility};
    use crate::interfaces::tui::repo_state::{RepoOperation, RepoStatus};
    use crate::interfaces::tui::state::TuiMode;

    #[test]
    fn title_is_selected_from_mode_only() {
        assert_eq!(title(TuiMode::Repos), "WORKON // CONTROL // REPO");
        assert_eq!(title(TuiMode::List), "WORKON // CONTROL");
    }

    #[test]
    fn rect_expands_only_for_visible_trace() {
        let hidden = rect(Rect::new(0, 0, 160, 36), TraceVisibility::Hidden);
        let visible = rect(Rect::new(0, 0, 160, 36), TraceVisibility::Visible);

        assert_eq!(hidden.width, 158);
        assert_eq!(hidden.height, 28);
        assert_eq!(hidden.x, 1);
        assert_eq!(visible.height, 33);
    }

    #[test]
    fn title_activity_is_repo_scoped() {
        let loading = RepoStatus::Loading {
            message: "Loading repositories".to_string(),
        };
        let applying = RepoStatus::Applying {
            action: RepoOperation::Remove,
            current: 1,
            total: 2,
            repository: "openai/workon".to_string(),
        };

        assert!(title_activity(TuiMode::List, &loading, 0).is_none());
        assert!(title_activity(TuiMode::Repos, &loading, 0).is_some());
        assert!(format!(
            "{:?}",
            title_activity(TuiMode::Repos, &applying, 0).expect("activity should render")
        )
        .contains("1/2 Removing openai/workon"));
    }
}
