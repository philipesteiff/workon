use std::path::PathBuf;

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::prelude::{Frame, Line, Span, Stylize};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Clear, List, ListItem, ListState, Paragraph, StatefulWidget, Wrap};

use crate::domain::WorkSummary;
use crate::slug::{slugify, title_from_goal};

use super::animation::{AnimationTarget, RenderRegions};
use super::components::{
    key, label_value, panel_block, panel_block_with_title, render_popup, section, status_badge,
    top_border,
};
use super::state::{Toast, ToastKind, TraceKind, TuiMode, TuiState};
use super::theme;

const CONTROL_PANEL_MAX_WIDTH: u16 = 160;
const CONTROL_PANEL_SIDE_MARGIN: u16 = 2;

#[cfg(test)]
pub(super) fn render(frame: &mut Frame<'_>, state: &TuiState) {
    render_inner(frame, state, None);
}

pub(super) fn render_for_animation(frame: &mut Frame<'_>, state: &TuiState) -> RenderRegions {
    let mut regions = RenderRegions::default();
    render_inner(frame, state, Some(&mut regions));
    regions
}

fn render_inner(frame: &mut Frame<'_>, state: &TuiState, mut regions: Option<&mut RenderRegions>) {
    let area = frame.area();
    let dialog = control_panel_rect(area, state);
    let block = panel_block("WORKON // CONTROL", true);
    let inner = block.inner(dialog);
    frame.render_widget(Clear, dialog);
    frame.render_widget(block, dialog);

    let [workspace, footer] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(2)]).areas(inner);

    render_workspace(frame, workspace, state, &mut regions);
    mark_region(&mut regions, AnimationTarget::FooterStatus, footer);
    render_footer(frame, footer, state);
    render_overlay(frame, area, state, &mut regions);
}

fn render_workspace(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &TuiState,
    regions: &mut Option<&mut RenderRegions>,
) {
    if area.height == 0 {
        return;
    }

    let trace_height = if !state.trace_visible || area.height < 14 || area.width < 76 {
        0
    } else if area.height < 22 {
        3
    } else {
        5
    };

    let (main, trace) = if trace_height == 0 {
        (area, None)
    } else {
        let [main, trace] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(trace_height)]).areas(area);
        (main, Some(trace))
    };

    render_main_panels(frame, main, state, regions);

    if let Some(trace) = trace {
        render_trace(frame, trace, state, regions);
    }
}

fn render_main_panels(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &TuiState,
    regions: &mut Option<&mut RenderRegions>,
) {
    if area.width < 86 {
        let [queue, diagnostic] =
            Layout::vertical([Constraint::Percentage(50), Constraint::Fill(1)]).areas(area);
        render_work_queue(frame, queue, state, regions);
        render_diagnostic(frame, diagnostic, state, regions);
    } else {
        let [queue, diagnostic] =
            Layout::horizontal([Constraint::Percentage(44), Constraint::Fill(1)]).areas(area);
        render_work_queue(frame, queue, state, regions);
        render_diagnostic(frame, diagnostic, state, regions);
    }
}

fn render_work_queue(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &TuiState,
    regions: &mut Option<&mut RenderRegions>,
) {
    mark_region(regions, AnimationTarget::WorkQueue, area);
    let works = state.filtered_works();
    let block = panel_block_with_title(
        work_queue_title(state.total_count()),
        matches!(state.mode, TuiMode::List | TuiMode::Search),
    );

    if works.is_empty() {
        let message = if state.filter.is_empty() {
            "No active Work. Press n to create Work.".to_string()
        } else {
            format!("No active Work matches \"{}\".", state.filter)
        };
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(status_badge("WARN", theme::style_status_warn())),
                Line::from(message),
                Line::from(""),
                Line::from("WORK QUEUE awaiting operator input.".dim()),
            ])
            .block(block)
            .wrap(Wrap { trim: true }),
            area,
        );
        return;
    }

    let selected = state.selected.min(works.len().saturating_sub(1));
    let items = works
        .iter()
        .enumerate()
        .map(|(index, work)| work_item(work, state.is_active_work(work), index == selected))
        .collect::<Vec<_>>();

    let mut list_state = ListState::default();
    list_state.select(Some(selected));
    let list = List::new(items)
        .block(block)
        .highlight_symbol("")
        .highlight_style(theme::style_selected_row_highlight());
    StatefulWidget::render(list, area, frame.buffer_mut(), &mut list_state);
}

fn work_queue_title(count: usize) -> Line<'static> {
    Line::from(vec![
        Span::raw(" "),
        Span::styled("WORK QUEUE", theme::style_panel_title()),
        Span::styled(" (", theme::style_panel_title()),
        Span::styled(count.to_string(), theme::style_command()),
        Span::styled(") ", theme::style_panel_title()),
    ])
}

fn work_item(work: &WorkSummary, active: bool, selected: bool) -> ListItem<'static> {
    let mut status = Vec::new();
    status.push(row_anchor(active, selected));
    status.push(if active {
        status_badge("CURRENT", theme::style_current_badge())
    } else {
        status_badge("ACTIVE", theme::style_active_badge())
    });
    let primary_style = work_primary_style(active);
    let secondary_style = work_secondary_style(active);
    status.extend([
        Span::raw(" "),
        Span::styled(work.title.clone(), primary_style),
    ]);

    let item = ListItem::new(vec![
        Line::from(status),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("intent ", secondary_style),
            Span::styled(work.intent_id.clone(), work_meta_value_style(active)),
        ]),
    ]);

    item
}

fn row_anchor(active: bool, selected: bool) -> Span<'static> {
    if active {
        Span::styled("@", theme::style_current_anchor())
    } else if selected {
        Span::styled(">>", theme::style_command())
    } else {
        Span::raw("  ")
    }
}

fn work_primary_style(active: bool) -> Style {
    if active {
        theme::style_current_text()
    } else {
        theme::style_work_title()
    }
}

fn work_secondary_style(active: bool) -> Style {
    if active {
        theme::style_current_meta_label()
    } else {
        theme::style_meta_key()
    }
}

fn work_meta_value_style(active: bool) -> Style {
    if active {
        theme::style_current_intent_value()
    } else {
        theme::style_meta_value()
    }
}

fn render_diagnostic(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &TuiState,
    regions: &mut Option<&mut RenderRegions>,
) {
    mark_region(regions, AnimationTarget::DetailPanel, area);
    let block = panel_block("WORK DETAIL", state.detail_visible);

    let Some(work) = state.selected_work() else {
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(status_badge("WARN", theme::style_status_warn())),
                Line::from("No active Work. Press n to create Work."),
                label_value("view", "EMPTY"),
            ])
            .block(block)
            .wrap(Wrap { trim: true }),
            area,
        );
        return;
    };

    let lines = if !state.detail_visible {
        vec![
            work_header(state, work),
            label_value("intent", work.intent_id.clone()),
            label_value("folder", work_folder_label(work)),
            label_value("work state", work_state_label(state, work)),
            label_value("view", diagnostic_state_label(state)),
        ]
    } else if area.height <= 12 {
        vec![
            section(work_section_label(state, work)),
            work_header(state, work),
            label_value("intent", work.intent_id.clone()),
            label_value("folder", work_folder_label(work)),
            label_value("work state", work_state_label(state, work)),
            section("Goal"),
            Line::from(work.goal.clone()),
        ]
    } else {
        vec![
            section(work_section_label(state, work)),
            work_header(state, work),
            Line::from(work.slug.clone().dim()),
            Line::from(""),
            section("Context"),
            label_value("intent", work.intent_id.clone()),
            label_value("folder", work_folder_label(work)),
            label_value("work state", work_state_label(state, work)),
            label_value("view", diagnostic_state_label(state)),
            Line::from(""),
            section("Goal"),
            Line::from(work.goal.clone()),
            label_value("archive", archive_path_for(work).display().to_string()),
        ]
    };

    frame.render_widget(
        Paragraph::new(lines).block(block).wrap(Wrap { trim: true }),
        area,
    );
}

fn render_trace(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &TuiState,
    regions: &mut Option<&mut RenderRegions>,
) {
    mark_region(regions, AnimationTarget::TracePanel, area);
    let block = panel_block("EXECUTION TRACE", false);
    let mut lines = if state.trace.is_empty() {
        vec![
            Line::from(vec![trace_badge(TraceKind::Run), Span::raw(" list loaded")]),
            Line::from(vec![
                status_badge("OK", theme::style_status_ok()),
                Span::raw(" AWAITING INPUT"),
            ]),
        ]
    } else {
        state
            .trace
            .iter()
            .map(|event| {
                Line::from(vec![
                    trace_badge(event.kind),
                    Span::raw(" "),
                    Span::styled(event.message.clone(), theme::style_primary_text()),
                ])
            })
            .collect::<Vec<_>>()
    };

    if let Some(toast) = &state.toast {
        lines.insert(
            0,
            Line::from(vec![
                toast_badge(toast.kind),
                Span::raw(" "),
                Span::styled(toast.title.clone(), theme::style_primary_text()),
            ]),
        );
    }

    frame.render_widget(
        Paragraph::new(lines).block(block).wrap(Wrap { trim: true }),
        area,
    );
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, state: &TuiState) {
    let mut spans = vec![
        status_badge("OPERATOR COMMAND", theme::style_command()),
        Span::raw(" "),
        Span::styled(mode_label(state.mode), mode_style(state.mode)),
        Span::raw("  "),
    ];
    spans.extend(footer_keys(
        area.width,
        state.mode,
        state.trace_visible,
        state.detail_visible,
    ));

    if area.width >= 96 {
        if let Some(toast) = &state.toast {
            spans.extend([
                Span::raw("  "),
                toast_badge(toast.kind),
                Span::raw(" "),
                Span::styled(toast.title.clone(), theme::style_muted_text()),
            ]);
        }
    }

    frame.render_widget(Paragraph::new(Line::from(spans)).block(top_border()), area);
}

fn footer_keys(
    width: u16,
    mode: TuiMode,
    trace_visible: bool,
    detail_visible: bool,
) -> Vec<Span<'static>> {
    match mode {
        TuiMode::List if width < 88 => vec![
            key("enter"),
            "open ".dim(),
            key("n"),
            "new ".dim(),
            key("/"),
            "find ".dim(),
            key("?"),
            "help ".dim(),
            key("q"),
            "quit".dim(),
        ],
        TuiMode::List if width < 112 => vec![
            key("enter"),
            "open ".dim(),
            key("n"),
            "new ".dim(),
            key("/"),
            "find ".dim(),
            key("C-d"),
            if detail_visible {
                "less ".dim()
            } else {
                "more ".dim()
            },
            key("?"),
            "help ".dim(),
            key("q"),
            "quit".dim(),
        ],
        TuiMode::List => vec![
            key("enter"),
            "open ".dim(),
            key("n"),
            "new ".dim(),
            key("/"),
            "find ".dim(),
            key("C-d"),
            if detail_visible {
                "hide details ".dim()
            } else {
                "details ".dim()
            },
            key("C-t"),
            if trace_visible {
                "hide trace ".dim()
            } else {
                "trace ".dim()
            },
            key("?"),
            "help ".dim(),
            key("q"),
            "quit".dim(),
        ],
        TuiMode::Search => vec![
            key("enter"),
            "apply ".dim(),
            key("esc"),
            "close ".dim(),
            key("j/k"),
            "move".dim(),
        ],
        TuiMode::Create => vec![
            key("enter"),
            "create ".dim(),
            key("tab/right"),
            "intent ".dim(),
            key("esc"),
            "cancel".dim(),
        ],
        TuiMode::Archive => vec![
            key("y/enter"),
            "archive ".dim(),
            key("esc/n"),
            "cancel".dim(),
        ],
        TuiMode::Help => vec![key("esc/?/q"), "close".dim()],
    }
}

fn render_overlay(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &TuiState,
    regions: &mut Option<&mut RenderRegions>,
) {
    match state.mode {
        TuiMode::Search => render_search(frame, area, state, regions),
        TuiMode::Create => render_create(frame, area, state, regions),
        TuiMode::Archive => render_archive(frame, area, state, regions),
        TuiMode::Help => render_help(frame, area, regions),
        TuiMode::List => {}
    }

    if let Some(toast) = &state.toast {
        render_toast(frame, area, toast, regions);
    }
}

fn render_search(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &TuiState,
    regions: &mut Option<&mut RenderRegions>,
) {
    let popup = top_popup(72, 7, area);
    mark_region(regions, AnimationTarget::Overlay, popup);
    let query = if state.filter.is_empty() {
        " ".to_string()
    } else {
        state.filter.clone()
    };
    let lines = vec![
        Line::from(vec![
            Span::styled("/ ", theme::style_command()),
            Span::styled(
                query,
                theme::style_primary_text().add_modifier(Modifier::BOLD),
            ),
        ]),
        label_value("fields", "title, slug, intent, goal"),
        label_value("matches", match_count_label(state.filtered_count())),
        Line::from(vec![
            key("enter"),
            "apply ".dim(),
            key("esc"),
            "close".dim(),
        ]),
    ];
    render_popup(
        frame,
        popup,
        "SIGNAL FILTER",
        lines,
        theme::style_focused_border(),
    );
}

fn render_create(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &TuiState,
    regions: &mut Option<&mut RenderRegions>,
) {
    let popup = centered_rect(78, 56, area);
    mark_region(regions, AnimationTarget::Overlay, popup);
    let mut lines = vec![
        section("WORK INTAKE"),
        label_value("Goal", ""),
        Line::from(if state.create_goal.is_empty() {
            "Type the Work goal.".dim()
        } else {
            Span::styled(state.create_goal.clone(), theme::style_primary_text())
        }),
        Line::from(""),
        section("INTENT"),
    ];
    lines.extend(selected_intent_lines(state));

    if !state.create_goal.trim().is_empty() {
        let title = title_from_goal(&state.create_goal);
        let slug = slugify(&title);
        lines.extend([
            Line::from(""),
            section("SLUG PREVIEW"),
            label_value("title", title),
            label_value("slug", slug),
        ]);
    }

    lines.extend([
        Line::from(""),
        Line::from(vec![
            key("enter"),
            "create ".dim(),
            key("tab/right"),
            "intent ".dim(),
            key("left"),
            "prev ".dim(),
            key("esc"),
            "cancel".dim(),
        ]),
    ]);
    render_popup(
        frame,
        popup,
        "WORK INIT",
        lines,
        theme::style_focused_border(),
    );
}

fn render_archive(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &TuiState,
    regions: &mut Option<&mut RenderRegions>,
) {
    let popup = centered_rect(74, 64, area);
    mark_region(regions, AnimationTarget::Overlay, popup);
    let Some(work) = state.selected_work() else {
        return;
    };
    let archive_path = archive_path_for(work);
    let lines = vec![
        Line::from(vec![
            status_badge("WARN", theme::style_status_warn()),
            Span::raw(" Archive active Work "),
            Span::styled(
                work.title.clone(),
                theme::style_primary_text().add_modifier(Modifier::BOLD),
            ),
            Span::raw("?"),
        ]),
        Line::from(""),
        label_value("from", work.path.display().to_string()),
        label_value("to", archive_path.display().to_string()),
        Line::from(vec![
            Span::styled(format!("{:<15}", "effect"), theme::style_muted_text()),
            Span::styled("Hidden from Active Work.", theme::style_destructive()),
        ]),
        Line::from(""),
        Line::from(vec![
            key("y/enter"),
            "archive ".dim(),
            key("esc/n"),
            "cancel".dim(),
        ]),
    ];
    render_popup(
        frame,
        popup,
        "ARCHIVE ACTIVE WORK",
        lines,
        theme::style_destructive(),
    );
}

fn render_help(frame: &mut Frame<'_>, area: Rect, regions: &mut Option<&mut RenderRegions>) {
    let popup = centered_rect(66, 52, area);
    mark_region(regions, AnimationTarget::Overlay, popup);
    let lines = vec![
        help_line("j/down", "next Work"),
        help_line("k/up", "previous Work"),
        help_line("/", "signal filter"),
        help_line("enter", "switch to highlighted Work"),
        help_line("n", "work init"),
        help_line("a", "archive highlighted Work"),
        help_line("esc", "close panel"),
    ];
    render_popup(
        frame,
        popup,
        "KEY INDEX",
        lines,
        theme::style_focused_border(),
    );
}

fn render_toast(
    frame: &mut Frame<'_>,
    area: Rect,
    toast: &Toast,
    regions: &mut Option<&mut RenderRegions>,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let width = area.width.min(58).max(area.width.min(30));
    let height = area.height.min(5);
    let x = area.x + area.width.saturating_sub(width + 2);
    let y = area.y + area.height.saturating_sub(height + 2);
    let popup = Rect::new(x, y, width, height);
    mark_region(regions, AnimationTarget::Toast, popup);
    let style = match toast.kind {
        ToastKind::Info => theme::style_status_ok(),
        ToastKind::Error => theme::style_status_error(),
    };
    let lines = vec![
        Line::from(vec![
            toast_badge(toast.kind),
            Span::raw(" "),
            toast.title.clone().bold(),
        ]),
        Line::from(toast.message.clone().dim()),
    ];
    render_popup(frame, popup, "LAST EVENT", lines, style);
}

fn mark_region(regions: &mut Option<&mut RenderRegions>, target: AnimationTarget, area: Rect) {
    if let Some(regions) = regions.as_deref_mut() {
        regions.set(target, area);
    }
}

fn selected_intent_lines(state: &TuiState) -> Vec<Line<'static>> {
    match state.intents.get(state.create_intent) {
        Some((intent, summary)) => vec![
            label_value("selected", selected_intent_label(state, intent)),
            label_value("available", intent_catalog_label(state)),
            Line::from(summary.clone().dim()),
        ],
        None => vec![
            label_value("selected", "1/1 investigate"),
            label_value("available", "investigate"),
        ],
    }
}

fn selected_intent_label(state: &TuiState, intent: &str) -> String {
    format!(
        "{}/{} {}",
        state.create_intent + 1,
        state.intents.len(),
        intent
    )
}

fn intent_catalog_label(state: &TuiState) -> String {
    if state.intents.is_empty() {
        return "investigate".to_string();
    }

    state
        .intents
        .iter()
        .map(|(intent, _)| intent.as_str())
        .collect::<Vec<_>>()
        .join(" | ")
}

fn work_header(state: &TuiState, work: &WorkSummary) -> Line<'static> {
    let mut spans = if state.is_active_work(work) {
        vec![status_badge("CURRENT", theme::style_current_badge())]
    } else {
        vec![status_badge("ACTIVE", theme::style_active_badge())]
    };
    spans.extend([
        Span::raw(" "),
        Span::styled(work.title.clone(), theme::style_primary_text()),
    ]);
    Line::from(spans)
}

fn work_section_label(state: &TuiState, work: &WorkSummary) -> &'static str {
    if state.is_active_work(work) {
        "CURRENT WORK"
    } else {
        "ACTIVE WORK"
    }
}

fn work_state_label(state: &TuiState, work: &WorkSummary) -> &'static str {
    if state.is_active_work(work) {
        "CURRENT"
    } else {
        "ACTIVE"
    }
}

fn help_line(key_value: &'static str, label: &'static str) -> Line<'static> {
    Line::from(vec![key(key_value), Span::raw(" "), label.dim()])
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

fn toast_badge(kind: ToastKind) -> Span<'static> {
    match kind {
        ToastKind::Info => status_badge("OK", theme::style_status_ok()),
        ToastKind::Error => status_badge("ERR", theme::style_status_error()),
    }
}

fn control_panel_rect(area: Rect, state: &TuiState) -> Rect {
    if area.width < 44 || area.height < 12 {
        return area;
    }

    let width = area
        .width
        .saturating_sub(CONTROL_PANEL_SIDE_MARGIN)
        .min(CONTROL_PANEL_MAX_WIDTH);
    let base_height = if state.detail_visible { 28 } else { 24 };
    let trace_extra = if state.trace_visible { 5 } else { 0 };
    let height = (base_height + trace_extra).min(area.height.saturating_sub(2));

    centered_area(area, width, height)
}

fn mode_style(mode: TuiMode) -> Style {
    match mode {
        TuiMode::Archive => theme::style_status_warn(),
        TuiMode::Create | TuiMode::Search | TuiMode::Help => theme::style_command(),
        TuiMode::List => theme::style_status_ok(),
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

fn mode_label(mode: TuiMode) -> &'static str {
    match mode {
        TuiMode::List => "LIST",
        TuiMode::Search => "FILTER",
        TuiMode::Create => "CREATE",
        TuiMode::Archive => "ARCHIVE",
        TuiMode::Help => "HELP",
    }
}

fn diagnostic_state_label(state: &TuiState) -> String {
    if state.filter.is_empty() {
        "ALL ACTIVE".to_string()
    } else {
        format!("FILTER {}", state.filter)
    }
}

fn archive_path_for(work: &WorkSummary) -> PathBuf {
    work.path
        .parent()
        .and_then(|work_root| work_root.parent())
        .map(|workon_root| workon_root.join("archive").join(&work.slug))
        .unwrap_or_else(|| PathBuf::from(".workon/archive").join(&work.slug))
}

fn work_folder_label(work: &WorkSummary) -> String {
    work.path
        .file_name()
        .map(|folder| format!("work/{}", folder.to_string_lossy()))
        .unwrap_or_else(|| work.path.display().to_string())
}

fn top_popup(percent_x: u16, height: u16, area: Rect) -> Rect {
    let width = area.width.saturating_mul(percent_x).saturating_div(100);
    let width = width.clamp(30.min(area.width), area.width);
    Rect::new(
        area.x + (area.width - width) / 2,
        area.y + 2,
        width,
        height.min(area.height),
    )
}

fn match_count_label(count: usize) -> String {
    if count == 1 {
        "1 match".to_string()
    } else {
        format!("{count} matches")
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::style::{Color, Modifier};
    use ratatui::Terminal;

    use super::{control_panel_rect, render, render_for_animation};
    use crate::domain::{WorkList, WorkSummary};
    use crate::tui::animation::AnimationTarget;
    use crate::tui::state::{Toast, TuiMode, TuiState};

    #[test]
    fn renders_work_list_and_detail() {
        let state = TuiState::new(work_list()).with_current_directory(Some(std::path::Path::new(
            "/tmp/workon/.workon/work/billing-retry-audit",
        )));
        let content = render_content(&state, 120, 36);

        assert!(content.contains("WORKON // CONTROL"));
        assert!(!content.contains("SYSTEM ONLINE"));
        assert!(!content.contains("ROOT "));
        assert!(content.contains("WORK QUEUE (1)"));
        assert!(!content.contains("MODE LIST"));
        assert!(!content.contains("SIGNAL AWAITING INPUT"));
        assert!(!content.contains("WORKS 1"));
        assert!(!content.contains("TASKS "));
        assert!(!content.contains("WORKS 1/1"));
        assert!(!content.contains("SELECT "));
        assert!(!content.contains("EXECUTION TRACE"));
        assert!(content.contains("OPERATOR COMMAND"));
        assert!(content.contains("WORK DETAIL"));
        assert!(content.contains("CURRENT"));
        assert!(content.contains("Billing retry audit"));
        assert!(content.contains("intent investigate"));
        assert!(!content.contains("slug billing-retry-audit"));
        assert!(content.contains("folder"));
        assert!(!content.contains("archive"));
        assert!(!content.contains("Goal"));
        assert!(!first_lines(&content, 4).contains("WORKON // CONTROL"));
        assert!(content.contains("@ CURRENT"));
        assert!(!content.contains(">> CURRENT"));

        let buffer = render_buffer(&state, 120, 36);
        let queue_title_row = row_containing(&buffer, "WORK QUEUE (1)");
        let queue_count = cell_at_text(&buffer, queue_title_row, "1");
        assert_eq!(queue_count.fg, Color::Rgb(255, 140, 32));
    }

    #[test]
    fn renders_active_and_current_queue_rows_as_distinct_work_states() {
        let mut state = TuiState::new(multi_work_list()).with_current_directory(Some(
            std::path::Path::new("/tmp/workon/.workon/work/billing-retry-audit"),
        ));
        state.move_selection(1);

        let content = render_content(&state, 120, 36);
        let buffer = render_buffer(&state, 120, 36);
        let current_row = row_containing(&buffer, "@ CURRENT");
        let active_row = row_containing(&buffer, ">> ACTIVE");
        let other_active_row = row_containing(&buffer, "Context command sketch");
        let other_active_meta_row = row_containing(&buffer, "intent brainstorm");

        assert!(content.contains("CURRENT"));
        assert!(content.contains("ACTIVE"));
        assert!(content.contains("Billing retry audit"));
        assert!(content.contains("Review cache invalidation PR"));
        assert!(content.contains("Context command sketch"));
        assert!(content.contains(">> ACTIVE"));
        assert!(content.contains("@ CURRENT"));
        assert!(!content.contains("TARGET"));
        assert!(!content.contains("READY"));
        assert!(!content.contains("STANDBY"));
        assert!(content.contains("intent review-pr"));
        assert!(content.contains("intent brainstorm"));
        assert!(!content.contains("slug review-cache"));
        assert!(!content.contains("slug context-command"));
        assert!(row_has_bg(&buffer, active_row, Color::Rgb(92, 58, 32)));
        assert!(!row_has_modifier(
            &buffer,
            current_row,
            Modifier::UNDERLINED
        ));

        let current_anchor = cell_at_text(&buffer, current_row, "@");
        let current_badge = cell_at_text(&buffer, current_row, "CURRENT");
        let current_title = cell_at_text(&buffer, current_row, "Billing retry audit");
        let current_meta_row = row_containing(&buffer, "intent investigate");
        let current_meta_value = cell_at_text(&buffer, current_meta_row, "investigate");
        let selected_active_badge = cell_at_text(&buffer, active_row, "ACTIVE");
        let selected_active_title =
            cell_at_text(&buffer, active_row, "Review cache invalidation PR");
        let target_meta_row = row_containing(&buffer, "intent review-pr");
        let target_meta_key = cell_at_text(&buffer, target_meta_row, "intent ");
        let target_meta_value = cell_at_text(&buffer, target_meta_row, "review-pr");
        let active_badge = cell_at_text(&buffer, other_active_row, "ACTIVE");
        let active_title = cell_at_text(&buffer, other_active_row, "Context command sketch");
        let active_meta_key = cell_at_text(&buffer, other_active_meta_row, "intent ");
        let active_meta_value = cell_at_text(&buffer, other_active_meta_row, "brainstorm");
        assert_eq!(current_anchor.fg, Color::Rgb(220, 180, 84));
        assert_eq!(current_anchor.bg, Color::Reset);
        assert!(current_anchor.modifier.contains(Modifier::BOLD));
        assert_eq!(current_badge.fg, Color::Rgb(220, 180, 84));
        assert_eq!(current_badge.bg, Color::Reset);
        assert!(current_badge.modifier.contains(Modifier::BOLD));
        assert_eq!(current_title.fg, Color::Rgb(220, 180, 84));
        assert_eq!(current_title.bg, Color::Reset);
        assert_eq!(current_meta_value.fg, Color::Rgb(220, 180, 84));
        assert_eq!(current_meta_value.bg, Color::Reset);
        assert_eq!(selected_active_badge.fg, Color::Rgb(190, 130, 70));
        assert_eq!(selected_active_badge.bg, Color::Rgb(92, 58, 32));
        assert_eq!(selected_active_title.fg, Color::Rgb(255, 176, 64));
        assert_eq!(selected_active_title.bg, Color::Rgb(92, 58, 32));
        assert_eq!(target_meta_key.fg, Color::Rgb(104, 72, 40));
        assert_eq!(target_meta_value.fg, Color::Rgb(255, 140, 32));
        assert_eq!(target_meta_value.bg, Color::Rgb(92, 58, 32));
        assert_eq!(active_badge.fg, Color::Rgb(190, 130, 70));
        assert_eq!(active_title.fg, Color::Rgb(255, 176, 64));
        assert!(active_title.modifier.contains(Modifier::BOLD));
        assert_eq!(active_meta_key.fg, Color::Rgb(104, 72, 40));
        assert_eq!(active_meta_value.fg, Color::Rgb(255, 140, 32));
        assert!(active_meta_value.modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn renders_detail_as_active_when_not_current_work() {
        let mut state = TuiState::new(multi_work_list()).with_current_directory(Some(
            std::path::Path::new("/tmp/workon/.workon/work/billing-retry-audit"),
        ));
        state.move_selection(1);

        let content = render_content(&state, 120, 36);

        assert!(content.contains("WORK DETAIL"));
        assert!(content.contains("ACTIVE"));
        assert!(!content.contains("TARGET"));
        assert!(!content.contains("READY"));
        assert!(!content.contains("STANDBY"));
    }

    #[test]
    fn renders_all_work_as_active_when_cwd_is_outside_work_folders() {
        let state = TuiState::new(multi_work_list())
            .with_current_directory(Some(std::path::Path::new("/tmp/workon/elsewhere")));

        let content = render_content(&state, 120, 36);

        assert!(content.contains(">> ACTIVE"));
        assert!(!content.contains("@ CURRENT"));
        assert!(!content.contains("CURRENT WORK"));
    }

    #[test]
    fn uses_more_horizontal_room_on_wide_terminals() {
        let state = TuiState::new(work_list());
        let panel = control_panel_rect(Rect::new(0, 0, 160, 36), &state);

        assert_eq!(panel.width, 158);
        assert_eq!(panel.height, 24);
        assert_eq!(panel.x, 1);
    }

    #[test]
    fn renders_execution_trace_only_when_toggled_visible() {
        let mut state = TuiState::new(work_list());
        state.push_trace(crate::tui::state::TraceKind::Run, "manual trace check");

        let hidden = render_content(&state, 120, 36);
        assert!(!hidden.contains("EXECUTION TRACE"));
        assert!(!hidden.contains("manual trace check"));
        assert!(hidden.contains("C-t trace"));

        state.trace_visible = true;
        let visible = render_content(&state, 120, 36);

        assert!(visible.contains("EXECUTION TRACE"));
        assert!(visible.contains("manual trace check"));
        assert!(visible.contains("C-t hide trace"));
    }

    #[test]
    fn renders_detail_panel_only_when_toggled_visible() {
        let mut state = TuiState::new(work_list());

        let compact = render_content(&state, 120, 36);
        assert!(!compact.contains("Goal"));
        assert!(!compact.contains("archive"));
        assert!(compact.contains("C-d details"));

        state.detail_visible = true;
        let expanded = render_content(&state, 120, 36);

        assert!(expanded.contains("Goal"));
        assert!(expanded.contains("archive"));
        assert!(expanded.contains("C-d hide details"));
    }

    #[test]
    fn renders_search_overlay_with_query_fields_and_count() {
        let mut state = TuiState::new(work_list());
        state.mode = TuiMode::Search;
        state.filter = "billing".to_string();

        let content = render_content(&state, 120, 36);

        assert!(content.contains("SIGNAL FILTER"));
        assert!(content.contains("/ billing"));
        assert!(content.contains("title, slug, intent, goal"));
        assert!(content.contains("1 match"));
        assert!(content.contains("enter apply"));
        assert!(content.contains("esc close"));
    }

    #[test]
    fn renders_create_overlay_with_neutral_prompt_intent_summary_and_empty_slug_preview() {
        let mut state = TuiState::new(work_list()).with_intents(vec![(
            "investigate".to_string(),
            "Answer a technical question with evidence.".to_string(),
        )]);
        state.mode = TuiMode::Create;

        let content = render_content(&state, 120, 36);

        assert!(content.contains("WORK INIT"));
        assert!(content.contains("WORK INTAKE"));
        assert!(!content.contains("TASK INIT"));
        assert!(!content.contains("TASK INTAKE"));
        assert!(content.contains("Type the Work goal."));
        assert!(content.contains("selected"));
        assert!(content.contains("1/1 investigate"));
        assert!(content.contains("available"));
        assert!(content.contains("investigate"));
        assert!(content.contains("Answer a technical question with evidence."));
        assert!(content.contains("enter create"));
        assert!(content.contains("tab/right intent"));
        assert!(!content.contains("SLUG PREVIEW"));
        assert!(!content.contains("Answer why billing retry alerts spiked"));
    }

    #[test]
    fn renders_create_overlay_with_compact_available_intents() {
        let mut state = TuiState::new(work_list()).with_intents(vec![
            (
                "investigate".to_string(),
                "Answer a technical question with evidence.".to_string(),
            ),
            (
                "review-pr".to_string(),
                "Review a pull request for regressions.".to_string(),
            ),
            (
                "brainstorm".to_string(),
                "Shape options before implementation.".to_string(),
            ),
        ]);
        state.mode = TuiMode::Create;
        state.create_intent = 1;

        let content = render_content(&state, 120, 36);

        assert!(content.contains("2/3 review-pr"));
        assert!(content.contains("available"));
        assert!(content.contains("investigate | review-pr | brainstorm"));
    }

    #[test]
    fn renders_create_overlay_slug_preview_after_goal_input() {
        let mut state = TuiState::new(work_list()).with_intents(vec![(
            "investigate".to_string(),
            "Answer a technical question with evidence.".to_string(),
        )]);
        state.mode = TuiMode::Create;
        state.create_goal = "Review the alert queue rollout".to_string();

        let content = render_content(&state, 120, 36);

        assert!(content.contains("review-the-alert-queue-rollout"));
    }

    #[test]
    fn renders_archive_overlay_with_matching_confirmation_keys() {
        let mut state = TuiState::new(work_list());
        state.mode = TuiMode::Archive;

        let content = render_content(&state, 120, 36);

        assert!(content.contains("ARCHIVE ACTIVE WORK"));
        assert!(content.contains("WARN"));
        assert!(content.contains("Archive active Work Billing retry audit?"));
        assert!(content.contains("Hidden from Active Work."));
        assert!(content.contains("y/enter archive"));
        assert!(content.contains("esc/n cancel"));
    }

    #[test]
    fn renders_archive_confirmation_keys_in_default_terminal_size() {
        let mut state = TuiState::new(work_list());
        state.mode = TuiMode::Archive;

        let content = render_content(&state, 80, 24);

        assert!(content.contains("Hidden from Active Work."));
        assert!(content.contains("y/enter archive"));
        assert!(content.contains("esc/n cancel"));
    }

    #[test]
    fn renders_help_as_complete_key_reference() {
        let mut state = TuiState::new(work_list());
        state.mode = TuiMode::Help;

        let content = render_content(&state, 120, 36);

        assert!(content.contains("KEY INDEX"));
        assert!(content.contains("j/down"));
        assert!(!content.contains("operator command"));
        assert!(!content.contains("Commands: list, create, switch, archive"));
    }

    #[test]
    fn renders_empty_list_and_empty_filter_result() {
        let empty = render_content(&TuiState::new(WorkList { works: Vec::new() }), 100, 28);
        assert!(empty.contains("No active Work. Press n to create Work."));

        let mut state = TuiState::new(work_list());
        state.filter = "missing".to_string();
        let filtered = render_content(&state, 100, 28);

        assert!(filtered.contains("No active Work matches \"missing\"."));
        assert!(filtered.contains("No active Work. Press n to create Work."));
    }

    #[test]
    fn renders_long_goal_and_path_without_losing_context_labels() {
        let mut state = TuiState::new(WorkList {
            works: vec![WorkSummary {
                title: "Long context review".to_string(),
                slug: "long-context-review".to_string(),
                goal: "Review the rollout decision, compare the operational evidence, preserve the tradeoffs, and identify the smallest next step before implementation.".to_string(),
                intent_id: "review-pr".to_string(),
                path: PathBuf::from("/tmp/workon/.workon/work/long-context-review/with/a/deep/path/that/should/wrap"),
            }],
        });
        state.detail_visible = true;

        let content = render_content(&state, 74, 30);

        assert!(content.contains("Goal"));
        assert!(content.contains("intent"));
        assert!(content.contains("folder"));
    }

    #[test]
    fn renders_core_context_in_default_terminal_size() {
        let state = TuiState::new(work_list());
        let content = render_content(&state, 80, 24);

        assert!(content.contains("WORKON // CONTROL"));
        assert!(content.contains("WORK QUEUE"));
        assert!(content.contains("OPERATOR COMMAND"));
        assert!(content.contains("intent"));
        assert!(content.contains("folder"));
        assert!(content.contains("work/billing-retry-audit"));
        assert!(!content.contains("/tmp/workon/.workon/work/billing-retry-audit"));
    }

    #[test]
    fn renders_toast_as_last_result_only() {
        let mut state = TuiState::new(work_list());
        state.toast = Some(Toast::info(
            "Work created",
            "/tmp/workon/.workon/work/billing",
        ));

        let content = render_content(&state, 120, 36);

        assert!(content.contains("LAST EVENT"));
        assert!(content.contains("Work created"));
    }

    #[test]
    fn animated_render_records_regions_without_changing_static_output() {
        let mut state = TuiState::new(work_list());
        state.mode = TuiMode::Search;
        state.trace_visible = true;
        state.detail_visible = true;
        state.toast = Some(Toast::info(
            "Work created",
            "/tmp/workon/.workon/work/billing",
        ));

        let static_content = render_content(&state, 120, 36);
        let (animated_content, regions) = render_content_for_animation(&state, 120, 36);

        assert_eq!(animated_content, static_content);
        assert!(regions.has_region(AnimationTarget::WorkQueue));
        assert!(regions.has_region(AnimationTarget::DetailPanel));
        assert!(regions.has_region(AnimationTarget::TracePanel));
        assert!(regions.has_region(AnimationTarget::FooterStatus));
        assert!(regions.has_region(AnimationTarget::Overlay));
        assert!(regions.has_region(AnimationTarget::Toast));
    }

    #[test]
    fn renders_mission_control_vocabulary_at_common_sizes() {
        for (width, height) in [(120, 36), (100, 28), (80, 24), (74, 30)] {
            let content = render_content(&TuiState::new(work_list()), width, height);

            assert!(content.contains("WORKON // CONTROL"), "{width}x{height}");
            assert!(content.contains("WORK QUEUE"), "{width}x{height}");
            assert!(content.contains("OPERATOR COMMAND"), "{width}x{height}");
        }
    }

    fn work_list() -> WorkList {
        WorkList {
            works: vec![WorkSummary {
                title: "Billing retry audit".to_string(),
                slug: "billing-retry-audit".to_string(),
                goal: "Find why billing retry alerts spiked after the queue rollout.".to_string(),
                intent_id: "investigate".to_string(),
                path: PathBuf::from("/tmp/workon/.workon/work/billing-retry-audit"),
            }],
        }
    }

    fn multi_work_list() -> WorkList {
        WorkList {
            works: vec![
                WorkSummary {
                    title: "Billing retry audit".to_string(),
                    slug: "billing-retry-audit".to_string(),
                    goal: "Find why billing retry alerts spiked after the queue rollout."
                        .to_string(),
                    intent_id: "investigate".to_string(),
                    path: PathBuf::from("/tmp/workon/.workon/work/billing-retry-audit"),
                },
                WorkSummary {
                    title: "Review cache invalidation PR".to_string(),
                    slug: "review-cache".to_string(),
                    goal: "Review the cache invalidation PR for regressions.".to_string(),
                    intent_id: "review-pr".to_string(),
                    path: PathBuf::from("/tmp/workon/.workon/work/review-cache"),
                },
                WorkSummary {
                    title: "Context command sketch".to_string(),
                    slug: "context-command".to_string(),
                    goal: "Shape the first context surface.".to_string(),
                    intent_id: "brainstorm".to_string(),
                    path: PathBuf::from("/tmp/workon/.workon/work/context-command"),
                },
            ],
        }
    }

    fn render_content(state: &TuiState, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height))
            .expect("test terminal should initialize");

        terminal
            .draw(|frame| render(frame, state))
            .expect("render should succeed");

        format!("{}", terminal.backend())
    }

    fn render_content_for_animation(
        state: &TuiState,
        width: u16,
        height: u16,
    ) -> (String, crate::tui::animation::RenderRegions) {
        let mut terminal = Terminal::new(TestBackend::new(width, height))
            .expect("test terminal should initialize");
        let mut regions = None;

        terminal
            .draw(|frame| {
                regions = Some(render_for_animation(frame, state));
            })
            .expect("render should succeed");

        (
            format!("{}", terminal.backend()),
            regions.expect("animated render should collect regions"),
        )
    }

    fn render_buffer(state: &TuiState, width: u16, height: u16) -> Buffer {
        let mut terminal = Terminal::new(TestBackend::new(width, height))
            .expect("test terminal should initialize");

        terminal
            .draw(|frame| render(frame, state))
            .expect("render should succeed");

        terminal.backend().buffer().clone()
    }

    fn first_lines(content: &str, count: usize) -> String {
        content.lines().take(count).collect::<Vec<_>>().join("\n")
    }

    fn row_containing(buffer: &Buffer, text: &str) -> u16 {
        for y in 0..buffer.area.height {
            let mut row = String::new();
            for x in 0..buffer.area.width {
                row.push_str(buffer[(x, y)].symbol());
            }
            if row.contains(text) {
                return y;
            }
        }

        panic!("buffer did not contain row text: {text}");
    }

    fn row_has_bg(buffer: &Buffer, y: u16, color: Color) -> bool {
        (0..buffer.area.width).any(|x| buffer[(x, y)].bg == color)
    }

    fn row_has_modifier(buffer: &Buffer, y: u16, modifier: Modifier) -> bool {
        (0..buffer.area.width).any(|x| buffer[(x, y)].modifier.contains(modifier))
    }

    fn cell_at_text<'a>(buffer: &'a Buffer, y: u16, text: &str) -> &'a ratatui::buffer::Cell {
        for x in 0..buffer.area.width {
            let mut candidate = String::new();
            for end in x..buffer.area.width {
                candidate.push_str(buffer[(end, y)].symbol());
                if candidate == text {
                    return &buffer[(x, y)];
                }
                if !text.starts_with(&candidate) || candidate.len() >= text.len() {
                    break;
                }
            }
        }

        panic!("row did not contain text: {text}");
    }
}
