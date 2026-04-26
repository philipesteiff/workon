use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::prelude::{Frame, Line, Span, Stylize};
use ratatui::widgets::{Block, Paragraph, Wrap};

use super::animation::{AnimationTarget, RenderRegions};
use super::components::{panel_block_with_title, section, status_badge};
use super::repo_state::{RepoCatalogRow, RepoOperation, RepoPane, RepoSelectionState, RepoStatus};
use super::repo_workspaces;
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

    if state.repo.workspace_dialog_open() {
        repo_workspaces::render(
            frame,
            area,
            &state.repo.workspace_dialog,
            &state.repo.workspaces,
            regions,
        );
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
        RepoOperation::Link => format!("{current}/{total} Linking {repository}"),
        RepoOperation::Remove => format!("{current}/{total} Removing {repository}"),
        RepoOperation::Refresh => format!("{current}/{total} Refreshing {repository}"),
    }
}

fn render_repo_catalog_panel(frame: &mut Frame<'_>, area: Rect, state: &TuiState) {
    let rows = state.repo.catalog_rows();
    let catalog_focused = matches!(state.repo.focus, RepoPane::Catalog | RepoPane::Workspace);
    let title = Line::from(vec![
        Span::raw(" "),
        Span::styled("REPOSITORIES", theme::style_panel_title()),
        Span::raw(" "),
        Span::styled(rows.len().to_string(), theme::style_command()),
        Span::raw(" "),
        Span::styled(focus_label(catalog_focused), theme::style_command()),
    ]);
    let block = panel_block_with_title(title, catalog_focused);
    if state.repo.requires_workspace_setup() {
        render_repo_workspace_setup(frame, area, state, block);
        return;
    }

    let mut lines = vec![repo_catalog_context_line(state, rows.is_empty())];

    if !matches!(state.repo.status, RepoStatus::Loading { .. }) && !rows.is_empty() {
        let visible_rows = block.inner(area).height.saturating_sub(lines.len() as u16) as usize;
        let start = scroll_start(state.repo.selected_catalog, rows.len(), visible_rows);
        lines.extend(
            rows.iter()
                .enumerate()
                .skip(start)
                .take(visible_rows)
                .map(|(index, row)| catalog_row(row, state, index)),
        );
    }

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_repo_workspace_setup(
    frame: &mut Frame<'_>,
    area: Rect,
    _state: &TuiState,
    block: Block<'_>,
) {
    let lines = vec![
        Line::from("Configure repo workspaces before creating or linking repositories.".dim()),
        section("REPO WORKSPACE SETUP"),
        Line::from(vec![
            status_badge("REQUIRED", theme::style_status_warn()),
            Span::raw(" "),
            Span::styled(
                "use the workspace dialog to add one or more folders",
                theme::style_primary_text(),
            ),
        ]),
    ];

    frame.render_widget(
        Paragraph::new(lines).block(block).wrap(Wrap { trim: true }),
        area,
    );
}

fn repo_catalog_context_line(state: &TuiState, rows_empty: bool) -> Line<'static> {
    if matches!(state.repo.status, RepoStatus::Loading { .. }) {
        return Line::from("Repository sources are loading.".dim());
    }

    if matches!(state.repo.status, RepoStatus::Failed { .. }) {
        return Line::from("GitHub is unavailable; local and attached repos remain usable.".dim());
    }

    if state.repo.force_remove && !state.repo.pending_remove.is_empty() {
        return Line::from(vec![
            status_badge("FORCE", theme::style_status_warn()),
            Span::raw(" "),
            Span::styled(
                "Force removal is armed for selected attached repositories.",
                theme::style_command(),
            ),
        ]);
    }

    if rows_empty {
        return if state.repo.filter.is_empty() {
            Line::from("No repositories are available from configured sources.".dim())
        } else {
            Line::from(format!("No repositories match `{}`.", state.repo.filter).dim())
        };
    }

    if !state.repo.filter.is_empty() {
        return Line::from(format!("Showing repositories matching `{}`.", state.repo.filter).dim());
    }

    match state.repo.selected_workspace_path() {
        Some(path) => Line::from(format!(
            "GitHub repos create in {}; local repos link in place.",
            path.display()
        )),
        None => Line::from("Select GitHub repos to create or local repos to link.".dim()),
    }
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
        let visible_rows = selected_area.height as usize;
        let start = scroll_start(state.repo.selected_work, rows.len(), visible_rows);
        rows.iter()
            .enumerate()
            .skip(start)
            .take(visible_rows)
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

    frame.render_widget(Paragraph::new(lines), selected_area);

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

fn scroll_start(selected: usize, total: usize, visible_rows: usize) -> usize {
    if total == 0 || visible_rows == 0 || selected < visible_rows {
        return 0;
    }
    selected.saturating_add(1).saturating_sub(visible_rows)
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

fn catalog_row(row: &RepoCatalogRow, state: &TuiState, index: usize) -> Line<'static> {
    match row {
        RepoCatalogRow::GitHub(repository) => catalog_repo_row(
            &repository.name_with_owner,
            &format!("github default {}", repository.default_branch),
            state.is_repository_selected(&repository.name_with_owner),
            state.repo.pending_add.contains(&repository.name_with_owner),
            state
                .repo
                .pending_remove
                .contains(&repository.name_with_owner),
            state.repo.force_remove,
            index == state.repo.selected_catalog && state.repo.focus == RepoPane::Catalog,
        ),
        RepoCatalogRow::Local(candidate) => catalog_repo_row(
            &candidate.name_with_owner,
            &format!("local {} {}", candidate.branch, candidate.path.display()),
            state.is_repository_selected(&candidate.name_with_owner),
            state.repo.is_local_candidate_selected(&candidate.path),
            state
                .repo
                .pending_remove
                .contains(&candidate.name_with_owner),
            state.repo.force_remove,
            index == state.repo.selected_catalog && state.repo.focus == RepoPane::Catalog,
        ),
    }
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
