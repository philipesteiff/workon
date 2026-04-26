use std::path::{Path, PathBuf};

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::prelude::{Frame, Line, Span, Stylize};
use ratatui::style::Style;
use ratatui::widgets::{Paragraph, Wrap};

use super::animation::{AnimationTarget, RenderRegions};
use super::components::{panel_block_with_title, section, status_badge};
use super::repo_state::{RepoCatalogRow, RepoOperation, RepoPane, RepoStatus};
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

    if area.width >= 96 {
        let [repositories, workspaces] =
            Layout::horizontal([Constraint::Percentage(76), Constraint::Fill(1)]).areas(area);
        render_repo_catalog_panel(frame, repositories, state);
        render_workspace_side_panels(frame, workspaces, state);
    } else {
        let [repositories, workspaces] =
            Layout::vertical([Constraint::Percentage(58), Constraint::Fill(1)]).areas(area);
        render_repo_catalog_panel(frame, repositories, state);
        render_workspace_side_panels(frame, workspaces, state);
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
    let catalog_focused = state.repo.focus == RepoPane::Catalog;
    let title = Line::from(vec![
        Span::raw(" "),
        Span::styled("REPOSITORIES", theme::style_panel_title()),
        Span::raw(" "),
        Span::styled(rows.len().to_string(), theme::style_command()),
        Span::raw(" "),
        Span::styled(focus_label(catalog_focused), theme::style_command()),
    ]);
    let block = panel_block_with_title(title, catalog_focused);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let log_height = if inner.height >= 12 { 6 } else { 0 };
    let (catalog_area, log_area) = if log_height == 0 {
        (inner, None)
    } else {
        let [catalog, log] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(log_height)]).areas(inner);
        (catalog, Some(log))
    };

    let mut lines = vec![repo_catalog_context_line(state, rows.is_empty())];

    if !rows.is_empty() {
        let line_budget = catalog_area.height.saturating_sub(lines.len() as u16) as usize;
        let reserve_detail = line_budget > 1;
        let visible_rows = if reserve_detail {
            line_budget.saturating_sub(1)
        } else {
            line_budget
        };
        let start = scroll_start(state.repo.selected_catalog, rows.len(), visible_rows);
        let workspace = state.repo.selected_workspace_path();
        for (index, row) in rows.iter().enumerate().skip(start).take(visible_rows) {
            lines.push(catalog_row(row, state, index, catalog_area.width));
            if reserve_detail && index == state.repo.selected_catalog {
                lines.push(catalog_row_detail(
                    row,
                    catalog_area.width,
                    workspace.as_deref(),
                ));
            }
        }
    }

    frame.render_widget(Paragraph::new(lines), catalog_area);

    if let Some(log_area) = log_area {
        render_repo_log(frame, log_area, state);
    }
}

fn repo_catalog_context_line(state: &TuiState, rows_empty: bool) -> Line<'static> {
    if matches!(state.repo.status, RepoStatus::Loading { .. }) {
        return Line::from("Repository sources are loading.".dim());
    }

    if state.repo.requires_workspace_setup() {
        return Line::from(vec![
            status_badge("REQUIRED", theme::style_status_warn()),
            Span::raw(" "),
            Span::styled(
                "Add a repo workspace path before creating GitHub repos.",
                theme::style_command(),
            ),
        ]);
    }

    if matches!(state.repo.status, RepoStatus::Failed { .. }) {
        return Line::from("GitHub is unavailable; local and attached repos remain usable.".dim());
    }

    if state.repo.force_remove && !state.repo.pending_remove.is_empty() {
        return Line::from(vec![
            status_badge("FORCE", theme::style_status_warn()),
            Span::raw(" "),
            Span::styled(
                "Force removal is armed for marked attached repositories.",
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
            "GitHub create in {}; local link in place.",
            compact_path(&path)
        )),
        None => Line::from("Select GitHub repos to create or local repos to link.".dim()),
    }
}

fn render_workspace_side_panels(frame: &mut Frame<'_>, area: Rect, state: &TuiState) {
    let [add_path, configured_paths] =
        Layout::vertical([Constraint::Length(7), Constraint::Fill(1)]).areas(area);
    render_add_path_panel(frame, add_path, state);
    render_configured_paths_panel(frame, configured_paths, state);
}

fn render_add_path_panel(frame: &mut Frame<'_>, area: Rect, state: &TuiState) {
    let focused = state.repo.focus == RepoPane::AddPath;
    let title = Line::from(vec![
        Span::raw(" "),
        Span::styled("ADD PATH", theme::style_panel_title()),
        Span::raw(" "),
        if state.repo.requires_workspace_setup() {
            status_badge("REQUIRED", theme::style_status_warn())
        } else {
            Span::raw("")
        },
        Span::raw(" "),
        Span::styled(focus_label(focused), theme::style_command()),
    ]);
    let block = panel_block_with_title(title, focused);
    let lines = vec![
        Line::from("one or more paths, comma-separated".dim()),
        Line::from(""),
        Line::from(vec![
            Span::styled("paths ", theme::style_command()),
            Span::styled(workspace_input_label(state), workspace_input_style(state)),
        ]),
    ];
    frame.render_widget(
        Paragraph::new(lines).block(block).wrap(Wrap { trim: true }),
        area,
    );
}

fn render_configured_paths_panel(frame: &mut Frame<'_>, area: Rect, state: &TuiState) {
    let focused = state.repo.focus == RepoPane::ConfiguredPaths;
    let title = Line::from(vec![
        Span::raw(" "),
        Span::styled("CONFIGURED PATHS", theme::style_panel_title()),
        Span::raw(" "),
        Span::styled(
            state.repo.workspaces.len().to_string(),
            theme::style_command(),
        ),
        Span::raw(" "),
        Span::styled(focus_label(focused), theme::style_command()),
    ]);
    let block = panel_block_with_title(title, focused);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines = if state.repo.workspaces.is_empty() {
        vec![Line::from("No repo workspaces configured.".dim())]
    } else {
        let visible_rows = inner.height as usize;
        let start = scroll_start(
            state.repo.selected_workspace,
            state.repo.workspaces.len(),
            visible_rows,
        );
        state
            .repo
            .workspaces
            .iter()
            .enumerate()
            .skip(start)
            .take(visible_rows)
            .map(|(index, workspace)| {
                workspace_row(state, workspace.path.display().to_string(), index, focused)
            })
            .collect()
    };
    frame.render_widget(Paragraph::new(lines), inner);
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

struct RepoRowRenderData {
    name: String,
    source: &'static str,
    branch: String,
    attached_to_work: bool,
    pending_add: bool,
    pending_remove: bool,
    force_remove: bool,
}

fn catalog_repo_row(data: RepoRowRenderData, selected: bool, width: u16) -> Line<'static> {
    let marker = if data.pending_remove && data.force_remove {
        "[!]"
    } else if data.pending_remove {
        "[-]"
    } else if data.pending_add || data.attached_to_work {
        "[x]"
    } else {
        "[ ]"
    };
    let status = if data.pending_add {
        "ADD"
    } else if data.pending_remove && data.force_remove {
        "FORCE"
    } else if data.pending_remove {
        "REMOVE"
    } else if data.attached_to_work && data.source != "attached" {
        "ATTACHED"
    } else {
        ""
    };
    let anchor = if selected { ">>" } else { "  " };
    let columns = repo_columns(usize::from(width));
    let mut spans = vec![
        Span::styled(anchor.to_string(), theme::style_command()),
        Span::raw(" "),
        Span::styled(marker.to_string(), theme::style_muted_text()),
        Span::raw(" "),
    ];
    push_column(
        &mut spans,
        &data.name,
        columns.name,
        theme::style_primary_text(),
        Truncate::End,
    );
    push_column(
        &mut spans,
        data.source,
        columns.source,
        theme::style_meta_value(),
        Truncate::End,
    );
    push_column(
        &mut spans,
        &data.branch,
        columns.branch,
        theme::style_meta_value(),
        Truncate::End,
    );
    push_column(
        &mut spans,
        status,
        columns.status,
        theme::style_command(),
        Truncate::End,
    );
    Line::from(spans)
}

fn catalog_row(row: &RepoCatalogRow, state: &TuiState, index: usize, width: u16) -> Line<'static> {
    match row {
        RepoCatalogRow::GitHub(repository) => catalog_repo_row(
            RepoRowRenderData {
                name: repository.name_with_owner.clone(),
                source: "github",
                branch: repository.default_branch.clone(),
                attached_to_work: state.is_repository_selected(&repository.name_with_owner),
                pending_add: state.repo.pending_add.contains(&repository.name_with_owner),
                pending_remove: state
                    .repo
                    .pending_remove
                    .contains(&repository.name_with_owner),
                force_remove: state.repo.force_remove,
            },
            index == state.repo.selected_catalog && state.repo.focus == RepoPane::Catalog,
            width,
        ),
        RepoCatalogRow::Local(candidate) => catalog_repo_row(
            RepoRowRenderData {
                name: candidate.name_with_owner.clone(),
                source: "local",
                branch: candidate.branch.clone(),
                attached_to_work: state.is_repository_selected(&candidate.name_with_owner),
                pending_add: state.repo.is_local_candidate_selected(&candidate.path),
                pending_remove: state
                    .repo
                    .pending_remove
                    .contains(&candidate.name_with_owner),
                force_remove: state.repo.force_remove,
            },
            index == state.repo.selected_catalog && state.repo.focus == RepoPane::Catalog,
            width,
        ),
        RepoCatalogRow::Attached(repository) => catalog_repo_row(
            RepoRowRenderData {
                name: repository.name_with_owner.clone(),
                source: "attached",
                branch: repository.branch.clone(),
                attached_to_work: state.is_repository_selected(&repository.name_with_owner),
                pending_add: false,
                pending_remove: state
                    .repo
                    .pending_remove
                    .contains(&repository.name_with_owner),
                force_remove: state.repo.force_remove,
            },
            index == state.repo.selected_catalog && state.repo.focus == RepoPane::Catalog,
            width,
        ),
    }
}

fn catalog_row_detail(row: &RepoCatalogRow, width: u16, workspace: Option<&Path>) -> Line<'static> {
    let detail = match row {
        RepoCatalogRow::GitHub(repository) => DetailFields {
            key: "create",
            value: workspace
                .map(compact_path)
                .unwrap_or_else(|| "workspace not set".to_string()),
            secondary_key: "work",
            secondary_value: workspace.map(|path| {
                row_path(
                    &path.join(
                        repository
                            .name_with_owner
                            .rsplit('/')
                            .next()
                            .unwrap_or("repo"),
                    ),
                    workspace,
                )
            }),
        },
        RepoCatalogRow::Local(candidate) => DetailFields {
            key: "path",
            value: compact_path(&candidate.path),
            secondary_key: "work",
            secondary_value: Some(row_path(&candidate.path, workspace)),
        },
        RepoCatalogRow::Attached(repository) => DetailFields {
            key: "path",
            value: compact_path(&repository.path),
            secondary_key: "work",
            secondary_value: Some(row_path(&repository.path, workspace)),
        },
    };
    let width = usize::from(width);
    let indent = 7.min(width);
    let value_width = width.saturating_sub(indent);

    let mut spans = vec![
        Span::raw(" ".repeat(indent)),
        Span::styled(detail.key.to_string(), theme::style_command()),
        Span::raw(" "),
    ];
    match detail.secondary_value {
        Some(secondary_value) => {
            let fixed_width =
                detail.key.len() + " ".len() + " | ".len() + detail.secondary_key.len() + " ".len();
            let available = value_width.saturating_sub(fixed_width);
            let value_column = detail_value_width(&detail.value, available);
            let secondary_column = available.saturating_sub(value_column);
            spans.push(Span::styled(
                fit_padded(&detail.value, value_column, Truncate::Start),
                theme::style_muted_text(),
            ));
            spans.push(Span::raw(" | "));
            spans.push(Span::styled(
                detail.secondary_key.to_string(),
                theme::style_command(),
            ));
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                fit_text(&secondary_value, secondary_column, Truncate::Start),
                theme::style_muted_text(),
            ));
        }
        None => spans.push(Span::styled(
            fit_text(&detail.value, value_width, Truncate::Start),
            theme::style_muted_text(),
        )),
    }
    Line::from(spans)
}

struct DetailFields {
    key: &'static str,
    value: String,
    secondary_key: &'static str,
    secondary_value: Option<String>,
}

fn detail_value_width(value: &str, available: usize) -> usize {
    if available == 0 {
        return 0;
    }
    let natural = value.chars().count();
    let preferred = available.saturating_mul(3) / 5;
    natural.min(preferred).max(available.min(16))
}

#[derive(Clone, Copy)]
struct RepoColumns {
    name: usize,
    source: usize,
    branch: usize,
    status: usize,
}

#[derive(Clone, Copy)]
enum Truncate {
    Start,
    End,
}

fn repo_columns(width: usize) -> RepoColumns {
    let source = if width >= 52 { 8 } else { 0 };
    let branch = if width >= 120 {
        18
    } else if width >= 64 {
        14
    } else {
        0
    };
    let status = if width >= 104 { 8 } else { 0 };
    let shown_columns = [source, branch, status]
        .into_iter()
        .filter(|column| *column > 0)
        .count();
    let gaps = shown_columns;
    let fixed = 7 + source + branch + status + gaps;
    let name = width.saturating_sub(fixed).max(12);

    RepoColumns {
        name,
        source,
        branch,
        status,
    }
}

fn push_column(
    spans: &mut Vec<Span<'static>>,
    value: &str,
    width: usize,
    style: Style,
    truncation: Truncate,
) {
    if width == 0 {
        return;
    }
    if spans.len() > 4 {
        spans.push(Span::raw(" "));
    }
    spans.push(Span::styled(fit_padded(value, width, truncation), style));
}

fn fit_padded(value: &str, width: usize, truncation: Truncate) -> String {
    let fitted = fit_text(value, width, truncation);
    format!("{fitted:<width$}")
}

fn fit_text(value: &str, width: usize, truncation: Truncate) -> String {
    if width == 0 {
        return String::new();
    }
    if value.chars().count() <= width {
        return value.to_string();
    }
    if width <= 3 {
        return ".".repeat(width);
    }

    let keep = width - 3;
    match truncation {
        Truncate::Start => {
            let tail = value
                .chars()
                .rev()
                .take(keep)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<String>();
            format!("...{tail}")
        }
        Truncate::End => {
            let head = value.chars().take(keep).collect::<String>();
            format!("{head}...")
        }
    }
}

fn row_path(path: &Path, workspace: Option<&Path>) -> String {
    if let Some(workspace) = workspace {
        if let Ok(relative) = path.strip_prefix(workspace) {
            if relative.as_os_str().is_empty() {
                return ".".to_string();
            }
            return format!("./{}", relative.display());
        }
    }
    compact_path(path)
}

fn compact_path(path: &Path) -> String {
    if let Some(home) = home_path() {
        if let Ok(relative) = path.strip_prefix(&home) {
            if relative.as_os_str().is_empty() {
                return "~".to_string();
            }
            return format!("~/{}", relative.display());
        }
    }

    path.display().to_string()
}

fn home_path() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
}

fn workspace_row(state: &TuiState, path: String, index: usize, focused: bool) -> Line<'static> {
    let selected = index == state.repo.selected_workspace;
    let anchor = if selected && focused { ">>" } else { "  " };
    let marker = if selected { "[x]" } else { "[ ]" };
    let path_style = if selected {
        theme::style_primary_text()
    } else {
        theme::style_meta_value()
    };
    Line::from(vec![
        Span::styled(anchor.to_string(), theme::style_command()),
        Span::raw(" "),
        Span::styled(marker.to_string(), theme::style_muted_text()),
        Span::raw(" "),
        Span::styled(path, path_style),
    ])
}

fn workspace_input_label(state: &TuiState) -> String {
    if state.repo.workspace_input().is_empty() {
        "<type folder path>".to_string()
    } else {
        state.repo.workspace_input().to_string()
    }
}

fn workspace_input_style(state: &TuiState) -> ratatui::style::Style {
    if state.repo.workspace_input().is_empty() {
        theme::style_muted_text()
    } else {
        theme::style_meta_value()
    }
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
