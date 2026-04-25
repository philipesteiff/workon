use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::prelude::{Frame, Line, Span, Stylize};
use ratatui::widgets::{Paragraph, Wrap};

use super::animation::{AnimationTarget, RenderRegions};
use super::components::{panel_block_with_title, section, status_badge};
use super::repo_state::{RepoOperation, RepoPane, RepoSelectionState, RepoStatus};
use super::state::{TraceKind, TuiState};
use super::theme;

pub(super) fn render_repo_context(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &TuiState,
    regions: &mut Option<&mut RenderRegions>,
) {
    if let Some(regions) = regions.as_deref_mut() {
        regions.set(AnimationTarget::WorkQueue, area);
    }

    if area.width < 72 {
        let [catalog, selected] =
            Layout::vertical([Constraint::Percentage(55), Constraint::Fill(1)]).areas(area);
        render_repo_catalog_panel(frame, catalog, state);
        render_repo_selected_panel(frame, selected, state);
    } else {
        let [catalog, selected] =
            Layout::horizontal([Constraint::Percentage(60), Constraint::Fill(1)]).areas(area);
        render_repo_catalog_panel(frame, catalog, state);
        render_repo_selected_panel(frame, selected, state);
    }
}

pub(super) fn repo_activity_message(
    action: RepoOperation,
    current: usize,
    total: usize,
    repository: &str,
) -> String {
    match action {
        RepoOperation::Add => format!("{current}/{total} Cloning {repository}"),
        RepoOperation::Remove => format!("{current}/{total} Removing {repository}"),
        RepoOperation::Refresh => format!("{current}/{total} Refreshing {repository}"),
    }
}

fn repo_status_line(state: &TuiState) -> Option<Line<'static>> {
    Some(match &state.repo.status {
        RepoStatus::Ready if state.pending_repo_change_count() == 0 => return None,
        RepoStatus::Ready => {
            if state.repo.force_remove && !state.repo.pending_remove.is_empty() {
                Line::from(vec![
                    status_badge("FORCE", theme::style_status_warn()),
                    Span::raw(" "),
                    Span::styled(
                        "dirty selected worktrees may be removed",
                        theme::style_command(),
                    ),
                ])
            } else {
                return None;
            }
        }
        RepoStatus::Loading { .. } => Line::from("gh repo list --no-archived".dim()),
        RepoStatus::Applying { .. } => return None,
        RepoStatus::Failed { message } => Line::from(vec![
            status_badge("ERR", theme::style_status_error()),
            Span::raw(" "),
            Span::styled(message.clone(), theme::style_primary_text()),
        ]),
    })
}

fn render_repo_catalog_panel(frame: &mut Frame<'_>, area: Rect, state: &TuiState) {
    let repositories = state.filtered_available_repositories();
    let title = Line::from(vec![
        Span::raw(" "),
        Span::styled("GITHUB REPOSITORIES", theme::style_panel_title()),
        Span::raw(" "),
        Span::styled(repositories.len().to_string(), theme::style_command()),
        Span::raw(" "),
        Span::styled(
            focus_label(state.repo.focus == RepoPane::Catalog),
            theme::style_command(),
        ),
    ]);
    let block = panel_block_with_title(title, state.repo.focus == RepoPane::Catalog);
    let mut lines = vec![Line::from(
        "space selects repos; ! arms force remove; enter applies changes".dim(),
    )];

    if let Some(status) = repo_status_line(state) {
        lines.push(status);
    }

    if !state.repo.filter.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("find ", theme::style_command()),
            Span::styled(state.repo.filter.clone(), theme::style_primary_text()),
        ]));
    }

    if matches!(state.repo.status, RepoStatus::Loading { .. }) {
        lines.push(Line::from("Waiting for gh repository catalog.".dim()));
    } else if repositories.is_empty() {
        lines.push(Line::from("No GitHub repositories match.".dim()));
    } else {
        lines.extend(repositories.iter().enumerate().map(|(index, repository)| {
            catalog_repo_row(
                &repository.name_with_owner,
                &format!("default {}", repository.default_branch),
                state.is_repository_selected(&repository.name_with_owner),
                state.repo.pending_add.contains(&repository.name_with_owner),
                state
                    .repo
                    .pending_remove
                    .contains(&repository.name_with_owner),
                state.repo.force_remove,
                index == state.repo.selected_catalog && state.repo.focus == RepoPane::Catalog,
            )
        }));
    }

    frame.render_widget(
        Paragraph::new(lines).block(block).wrap(Wrap { trim: true }),
        area,
    );
}

fn render_repo_selected_panel(frame: &mut Frame<'_>, area: Rect, state: &TuiState) {
    let rows = state.selected_repository_rows();
    let title = Line::from(vec![
        Span::raw(" "),
        Span::styled("SELECTED FOR WORK", theme::style_panel_title()),
        Span::raw(" "),
        Span::styled(rows.len().to_string(), theme::style_command()),
        Span::raw(" "),
        Span::styled(
            focus_label(state.repo.focus == RepoPane::Selected),
            theme::style_command(),
        ),
    ]);
    let block = panel_block_with_title(title, state.repo.focus == RepoPane::Selected);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let log_height = if inner.height >= 12 { 6 } else { 0 };
    let (selected_area, log_area) = if log_height == 0 {
        (inner, None)
    } else {
        let [selected, log] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(log_height)]).areas(inner);
        (selected, Some(log))
    };

    let lines = if rows.is_empty() {
        vec![Line::from("No repositories selected for this Work.".dim())]
    } else {
        rows.iter()
            .enumerate()
            .map(|(index, repository)| {
                selected_repo_row(
                    &repository.name_with_owner,
                    &repository.meta,
                    repository.state,
                    index == state.repo.selected_work && state.repo.focus == RepoPane::Selected,
                )
            })
            .collect::<Vec<_>>()
    };

    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: true }),
        selected_area,
    );

    if let Some(log_area) = log_area {
        render_repo_log(frame, log_area, state);
    }
}

fn render_repo_log(frame: &mut Frame<'_>, area: Rect, state: &TuiState) {
    let mut lines = vec![section("OPERATION LOG")];
    if state.repo.logs.is_empty() {
        lines.push(Line::from("No repository operations yet.".dim()));
    } else {
        lines.extend(state.repo.logs.iter().rev().map(|event| {
            Line::from(vec![
                trace_badge(event.kind),
                Span::raw(" "),
                Span::styled(event.message.clone(), theme::style_muted_text()),
            ])
        }));
    }

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
}

fn catalog_repo_row(
    name: &str,
    meta: &str,
    selected_for_work: bool,
    pending_add: bool,
    pending_remove: bool,
    force_remove: bool,
    selected: bool,
) -> Line<'static> {
    let marker = if pending_remove && force_remove {
        "[!]"
    } else if pending_remove {
        "[-]"
    } else if pending_add || selected_for_work {
        "[x]"
    } else {
        "[ ]"
    };
    let status = if pending_add {
        "ADD"
    } else if pending_remove && force_remove {
        "FORCE"
    } else if pending_remove {
        "REMOVE"
    } else if selected_for_work {
        "SELECTED"
    } else {
        ""
    };
    let anchor = if selected { ">>" } else { "  " };
    Line::from(vec![
        Span::styled(anchor.to_string(), theme::style_command()),
        Span::raw(" "),
        Span::styled(marker.to_string(), theme::style_muted_text()),
        Span::raw(" "),
        Span::styled(name.to_string(), theme::style_primary_text()),
        Span::raw("  "),
        Span::styled(meta.to_string(), theme::style_meta_value()),
        Span::raw(" "),
        Span::styled(status.to_string(), theme::style_command()),
    ])
}

fn selected_repo_row(
    name: &str,
    meta: &str,
    state: RepoSelectionState,
    selected: bool,
) -> Line<'static> {
    let (marker, status) = match state {
        RepoSelectionState::Attached => ("[x]", "ATTACHED"),
        RepoSelectionState::PendingAdd => ("[+]", "ADD"),
        RepoSelectionState::PendingRemove => ("[-]", "REMOVE"),
        RepoSelectionState::PendingForceRemove => ("[!]", "FORCE"),
    };
    let anchor = if selected { ">>" } else { "  " };
    Line::from(vec![
        Span::styled(anchor.to_string(), theme::style_command()),
        Span::raw(" "),
        Span::styled(marker.to_string(), theme::style_muted_text()),
        Span::raw(" "),
        Span::styled(name.to_string(), theme::style_primary_text()),
        Span::raw("  "),
        Span::styled(meta.to_string(), theme::style_meta_value()),
        Span::raw(" "),
        Span::styled(status.to_string(), theme::style_command()),
    ])
}

fn trace_badge(kind: TraceKind) -> Span<'static> {
    match kind {
        TraceKind::Run => status_badge("RUN", theme::style_status_run()),
        TraceKind::Sync => status_badge("SYNC", theme::style_status_ok()),
        TraceKind::Signal => status_badge("SIGNAL", theme::style_command()),
        TraceKind::Warn => status_badge("WARN", theme::style_status_warn()),
        TraceKind::Err => status_badge("ERR", theme::style_status_error()),
    }
}

fn focus_label(focused: bool) -> &'static str {
    if focused {
        "FOCUS"
    } else {
        ""
    }
}
