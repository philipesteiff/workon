use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::prelude::{Frame, Line, Span, Stylize};
use ratatui::style::Style;
use ratatui::widgets::{Paragraph, Wrap};

use super::animation::{AnimationTarget, RenderRegions};
use super::components::{panel_block_with_title, section, status_badge};
use super::repo_rows::{catalog_header_row, catalog_row, catalog_row_detail, compact_path};
use super::repo_state::{RepoPane, RepoPickerState, RepoStatus};
use super::state::TraceKind;
use super::theme;

pub(super) fn render_repo_context(
    frame: &mut Frame<'_>,
    area: Rect,
    repo: &RepoPickerState,
    regions: &mut Option<&mut RenderRegions>,
) {
    if let Some(regions) = regions.as_deref_mut() {
        regions.set(AnimationTarget::WorkQueue, area);
    }

    if area.width >= 96 {
        let [repositories, workspaces] =
            Layout::horizontal([Constraint::Percentage(76), Constraint::Fill(1)]).areas(area);
        render_repo_catalog_panel(frame, repositories, repo);
        render_workspace_side_panels(frame, workspaces, repo);
    } else {
        let [repositories, workspaces] =
            Layout::vertical([Constraint::Percentage(58), Constraint::Fill(1)]).areas(area);
        render_repo_catalog_panel(frame, repositories, repo);
        render_workspace_side_panels(frame, workspaces, repo);
    }
}

fn render_repo_catalog_panel(frame: &mut Frame<'_>, area: Rect, repo: &RepoPickerState) {
    let rows = repo.catalog_rows();
    let catalog_focused = repo.focus == RepoPane::Catalog;
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

    let mut lines = vec![repo_catalog_context_line(repo, rows.is_empty())];

    if !rows.is_empty() {
        lines.push(catalog_header_row(catalog_area.width));
        let line_budget = catalog_area.height.saturating_sub(lines.len() as u16) as usize;
        let reserve_detail = line_budget > 1;
        let visible_rows = if reserve_detail {
            line_budget.saturating_sub(1)
        } else {
            line_budget
        };
        let start = scroll_start(repo.selected_catalog, rows.len(), visible_rows);
        let workspace = repo.selected_workspace_path();
        for (index, row) in rows.iter().enumerate().skip(start).take(visible_rows) {
            lines.push(catalog_row(row, repo, index, catalog_area.width));
            if reserve_detail && index == repo.selected_catalog {
                lines.push(catalog_row_detail(
                    row,
                    catalog_area.width,
                    workspace.as_deref(),
                    &repo.work_slug,
                ));
            }
        }
    }

    frame.render_widget(Paragraph::new(lines), catalog_area);

    if let Some(log_area) = log_area {
        render_repo_log(frame, log_area, repo);
    }
}

fn repo_catalog_context_line(repo: &RepoPickerState, rows_empty: bool) -> Line<'static> {
    if matches!(repo.status, RepoStatus::Loading { .. }) {
        return Line::from("Repository sources are loading.".dim());
    }

    if repo.requires_workspace_setup() {
        return Line::from(vec![
            status_badge("REQUIRED", theme::style_status_warn()),
            Span::raw(" "),
            Span::styled(
                "Add a repo workspace path before creating remote repo worktrees.",
                theme::style_command(),
            ),
        ]);
    }

    if matches!(repo.status, RepoStatus::Failed { .. }) {
        return Line::from("GitHub is unavailable; local and attached repos remain usable.".dim());
    }

    if repo.force_remove && !repo.pending_remove.is_empty() {
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
        return if repo.filter.is_empty() {
            Line::from("No repositories are available from configured sources.".dim())
        } else {
            Line::from(format!("No repositories match `{}`.", repo.filter).dim())
        };
    }

    if !repo.filter.is_empty() {
        return Line::from(format!("Showing repositories matching `{}`.", repo.filter).dim());
    }

    match repo.selected_workspace_path() {
        Some(path) => Line::from(format!(
            "Remote repo worktrees in {}; local links in place.",
            compact_path(&path)
        )),
        None => Line::from("Select remote repos for worktrees or local repos to link.".dim()),
    }
}

fn render_workspace_side_panels(frame: &mut Frame<'_>, area: Rect, repo: &RepoPickerState) {
    let [add_path, configured_paths] =
        Layout::vertical([Constraint::Length(7), Constraint::Fill(1)]).areas(area);
    render_add_path_panel(frame, add_path, repo);
    render_configured_paths_panel(frame, configured_paths, repo);
}

fn render_add_path_panel(frame: &mut Frame<'_>, area: Rect, repo: &RepoPickerState) {
    let focused = repo.focus == RepoPane::AddPath;
    let title = Line::from(vec![
        Span::raw(" "),
        Span::styled("ADD PATH", theme::style_panel_title()),
        Span::raw(" "),
        if repo.requires_workspace_setup() {
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
            Span::styled(workspace_input_label(repo), workspace_input_style(repo)),
        ]),
    ];
    frame.render_widget(
        Paragraph::new(lines).block(block).wrap(Wrap { trim: true }),
        area,
    );
}

fn render_configured_paths_panel(frame: &mut Frame<'_>, area: Rect, repo: &RepoPickerState) {
    let focused = repo.focus == RepoPane::ConfiguredPaths;
    let title = Line::from(vec![
        Span::raw(" "),
        Span::styled("CONFIGURED PATHS", theme::style_panel_title()),
        Span::raw(" "),
        Span::styled(repo.workspaces.len().to_string(), theme::style_command()),
        Span::raw(" "),
        Span::styled(focus_label(focused), theme::style_command()),
    ]);
    let block = panel_block_with_title(title, focused);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines = if repo.workspaces.is_empty() {
        vec![Line::from("No repo workspaces configured.".dim())]
    } else {
        let visible_rows = inner.height as usize;
        let start = scroll_start(repo.selected_workspace, repo.workspaces.len(), visible_rows);
        repo.workspaces
            .iter()
            .enumerate()
            .skip(start)
            .take(visible_rows)
            .map(|(index, workspace)| {
                workspace_row(repo, workspace.path.display().to_string(), index, focused)
            })
            .collect()
    };
    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_repo_log(frame: &mut Frame<'_>, area: Rect, repo: &RepoPickerState) {
    let mut lines = vec![section("OPERATION LOG")];
    if repo.logs.is_empty() {
        lines.push(Line::from("No repository operations yet.".dim()));
    } else {
        lines.extend(repo.logs.iter().rev().map(|event| {
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

fn workspace_row(
    repo: &RepoPickerState,
    path: String,
    index: usize,
    focused: bool,
) -> Line<'static> {
    let selected = index == repo.selected_workspace;
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

fn workspace_input_label(repo: &RepoPickerState) -> String {
    if repo.workspace_input().is_empty() {
        "<type folder path>".to_string()
    } else {
        repo.workspace_input().to_string()
    }
}

fn workspace_input_style(repo: &RepoPickerState) -> Style {
    if repo.workspace_input().is_empty() {
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

#[cfg(test)]
mod tests {
    use crate::domain::{AttachedRepository, AvailableRepository, RepositoryWorkspace};
    use crate::interfaces::tui::repo_state::{RepoCatalogRow, RepoPickerState};

    use super::catalog_row;

    #[test]
    fn catalog_rows_render_from_repo_picker_state_without_full_tui_state() {
        let mut picker = RepoPickerState::default();
        picker.enter_context(
            "billing-retry-audit".to_string(),
            "Billing retry audit".to_string(),
            vec![AvailableRepository {
                name_with_owner: "openai/api".to_string(),
                default_branch: "main".to_string(),
                url: "https://github.com/openai/api".to_string(),
                ssh_url: "git@github.com:openai/api.git".to_string(),
            }],
            Vec::new(),
            vec![AttachedRepository {
                name_with_owner: "openai/workon".to_string(),
                branch: "workon/billing-retry-audit".to_string(),
                path: "/tmp/workon/.workon/work/billing-retry-audit/repos/openai__workon".into(),
                default_branch: "main".to_string(),
                url: "https://github.com/openai/workon".to_string(),
            }],
            vec![RepositoryWorkspace {
                path: "/tmp/repos".into(),
            }],
        );

        let row = RepoCatalogRow::GitHub(AvailableRepository {
            name_with_owner: "openai/api".to_string(),
            default_branch: "main".to_string(),
            url: "https://github.com/openai/api".to_string(),
            ssh_url: "git@github.com:openai/api.git".to_string(),
        });

        let rendered = catalog_row(&row, &picker, 0, 96);

        assert_eq!(rendered.spans[4].content.trim(), "openai/api");
    }
}
