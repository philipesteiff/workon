use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::prelude::{Frame, Line, Span, Stylize};
use ratatui::widgets::{Clear, Paragraph, Wrap};

use crate::domain::RepositoryWorkspace;

use super::super::animation::{AnimationTarget, RenderRegions};
use super::super::components::panel_block_with_title;
use super::super::theme;
use super::state::{RepoWorkspaceDialogFocus, RepoWorkspaceDialogState};

pub(in crate::interfaces::tui) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &RepoWorkspaceDialogState,
    workspaces: &[RepositoryWorkspace],
    regions: &mut Option<&mut RenderRegions>,
) {
    let popup = centered_rect(86, 62, area);
    if let Some(regions) = regions.as_deref_mut() {
        regions.set(AnimationTarget::Overlay, popup);
    }

    frame.render_widget(Clear, popup);
    let outer = panel_block_with_title(
        Line::from(vec![
            Span::raw(" "),
            Span::styled("REPO WORKSPACES", theme::style_panel_title()),
            Span::raw(" "),
            Span::styled(workspaces.len().to_string(), theme::style_command()),
            Span::raw(" "),
        ]),
        true,
    );
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    let [body, footer] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(inner);
    if body.width < 74 {
        let [paths, add] =
            Layout::vertical([Constraint::Percentage(58), Constraint::Fill(1)]).areas(body);
        render_paths_panel(frame, paths, state, workspaces);
        render_add_panel(frame, add, state);
    } else {
        let [paths, add] =
            Layout::horizontal([Constraint::Percentage(58), Constraint::Fill(1)]).areas(body);
        render_paths_panel(frame, paths, state, workspaces);
        render_add_panel(frame, add, state);
    }
    render_footer(frame, footer);
}

fn render_paths_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &RepoWorkspaceDialogState,
    workspaces: &[RepositoryWorkspace],
) {
    let focused = state.focus() == RepoWorkspaceDialogFocus::List;
    let title = Line::from(vec![
        Span::raw(" "),
        Span::styled("CONFIGURED PATHS", theme::style_panel_title()),
        Span::raw(" "),
        Span::styled(workspaces.len().to_string(), theme::style_command()),
        Span::raw(" "),
        Span::styled(focus_label(focused), theme::style_command()),
    ]);
    let block = panel_block_with_title(title, focused);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines = if workspaces.is_empty() {
        vec![Line::from("No repo workspaces configured.".dim())]
    } else {
        let visible_rows = inner.height as usize;
        let start = scroll_start(state.selected(), workspaces.len(), visible_rows);
        workspaces
            .iter()
            .enumerate()
            .skip(start)
            .take(visible_rows)
            .map(|(index, workspace)| workspace_row(state, workspace, index, focused))
            .collect()
    };
    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_add_panel(frame: &mut Frame<'_>, area: Rect, state: &RepoWorkspaceDialogState) {
    let focused = state.focus() == RepoWorkspaceDialogFocus::Add;
    let title = Line::from(vec![
        Span::raw(" "),
        Span::styled("ADD PATH", theme::style_panel_title()),
        Span::raw(" "),
        Span::styled(focus_label(focused), theme::style_command()),
    ]);
    let block = panel_block_with_title(title, focused);
    let lines = vec![
        Line::from("one or more paths, comma-separated".dim()),
        Line::from(""),
        Line::from(vec![
            Span::styled("paths ", theme::style_command()),
            Span::styled(input_label(state), input_style(state)),
        ]),
    ];
    frame.render_widget(
        Paragraph::new(lines).block(block).wrap(Wrap { trim: true }),
        area,
    );
}

fn render_footer(frame: &mut Frame<'_>, area: Rect) {
    let line = Line::from(vec![
        key("tab"),
        "panel ".dim(),
        key("up/down"),
        "select ".dim(),
        key("space"),
        "remove ".dim(),
        key("enter"),
        "apply ".dim(),
        key("esc"),
        "back".dim(),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn workspace_row(
    state: &RepoWorkspaceDialogState,
    workspace: &RepositoryWorkspace,
    index: usize,
    focused: bool,
) -> Line<'static> {
    let selected = index == state.selected();
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
        Span::styled(workspace.path.display().to_string(), path_style),
    ])
}

fn input_label(state: &RepoWorkspaceDialogState) -> String {
    if state.input().is_empty() {
        "<type folder path>".to_string()
    } else {
        state.input().to_string()
    }
}

fn input_style(state: &RepoWorkspaceDialogState) -> ratatui::style::Style {
    if state.input().is_empty() {
        theme::style_muted_text()
    } else {
        theme::style_meta_value()
    }
}

fn key(value: &'static str) -> Span<'static> {
    Span::styled(format!(" {value} "), theme::style_key())
}

fn focus_label(focused: bool) -> &'static str {
    if focused {
        "FOCUS"
    } else {
        ""
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let [_, center, _] = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .areas(area);

    let [_, center, _] = Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .areas(center);

    center
}

fn scroll_start(selected: usize, total: usize, visible_rows: usize) -> usize {
    if total == 0 || visible_rows == 0 || selected < visible_rows {
        return 0;
    }
    selected.saturating_add(1).saturating_sub(visible_rows)
}
