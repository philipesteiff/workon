use std::path::PathBuf;

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::prelude::{Frame, Line, Span, Stylize};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Clear, List, ListItem, ListState, Paragraph, StatefulWidget, Wrap};

use crate::domain::work::naming::{slugify, title_from_goal};
use crate::domain::{AttachedRepository, WorkSummary};

use super::animation::{AnimationTarget, RenderRegions};
use super::components::{
    key, label_value, panel_block, panel_block_with_activity, panel_block_with_title, render_popup,
    section, status_badge, top_border,
};
use super::control_panel::{self, TraceVisibility};
use super::repo_state::RepoPane;
use super::repo_ui::render_repo_context;
use super::state::{Toast, ToastKind, TraceKind, TuiMode, TuiState};
use super::theme;

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
    let block = panel_block_with_activity(
        control_panel::title(state.mode),
        true,
        control_panel::title_activity(state.mode, &state.repo.status, state.repo.activity_frame),
    );
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

    if state.mode == TuiMode::Repos {
        render_repo_context(frame, area, &state.repo, regions);
        return;
    }

    if state.mode == TuiMode::Intents {
        render_intent_context(frame, area, state, regions);
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
        work_queue_title(state),
        matches!(
            state.mode,
            TuiMode::List | TuiMode::Search | TuiMode::Leader
        ),
    );

    if works.is_empty() {
        let message = if state.filter.is_empty() {
            "No active Work. Press /n to create Work.".to_string()
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
        .map(|(index, work)| work_item(work, state.is_current_work(work), index == selected))
        .collect::<Vec<_>>();

    let mut list_state = ListState::default();
    list_state.select(Some(selected));
    let list = List::new(items)
        .block(block)
        .highlight_symbol("")
        .highlight_style(theme::style_selected_row_highlight());
    StatefulWidget::render(list, area, frame.buffer_mut(), &mut list_state);
}

fn work_queue_title(state: &TuiState) -> Line<'static> {
    let mut spans = vec![
        Span::raw(" "),
        Span::styled("WORK QUEUE", theme::style_panel_title()),
        Span::styled(" (", theme::style_panel_title()),
        Span::styled(state.total_count().to_string(), theme::style_command()),
        Span::styled(") ", theme::style_panel_title()),
    ];

    if !state.filter.is_empty() {
        spans.extend([
            Span::styled("find ", theme::style_command()),
            Span::styled(state.filter.clone(), theme::style_primary_text()),
            Span::raw(" "),
            Span::styled(
                match_count_label(state.filtered_count()),
                theme::style_command(),
            ),
            Span::raw(" "),
        ]);
    }

    Line::from(spans)
}

fn work_item(work: &WorkSummary, current: bool, selected: bool) -> ListItem<'static> {
    let mut status = Vec::new();
    status.push(row_anchor(current, selected));
    status.push(if current {
        status_badge("CURRENT", theme::style_current_badge())
    } else {
        status_badge("ACTIVE", theme::style_active_badge())
    });
    let primary_style = work_primary_style(current);
    let secondary_style = work_secondary_style(current);
    status.extend([
        Span::raw(" "),
        Span::styled(work.title.clone(), primary_style),
    ]);

    let item = ListItem::new(vec![
        Line::from(status),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("intent ", secondary_style),
            Span::styled(work.intent_id.clone(), work_meta_value_style(current)),
        ]),
    ]);

    item
}

fn row_anchor(current: bool, selected: bool) -> Span<'static> {
    if current {
        Span::styled("@", theme::style_current_anchor())
    } else if selected {
        Span::styled(">>", theme::style_command())
    } else {
        Span::raw("  ")
    }
}

fn work_primary_style(current: bool) -> Style {
    if current {
        theme::style_current_text()
    } else {
        theme::style_work_title()
    }
}

fn work_secondary_style(current: bool) -> Style {
    if current {
        theme::style_current_meta_label()
    } else {
        theme::style_meta_key()
    }
}

fn work_meta_value_style(current: bool) -> Style {
    if current {
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
    let block = panel_block("WORK DETAIL", false);

    let Some(work) = state.selected_work() else {
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(status_badge("WARN", theme::style_status_warn())),
                Line::from("No active Work. Press /n to create Work."),
            ])
            .block(block)
            .wrap(Wrap { trim: true }),
            area,
        );
        return;
    };

    let lines = detail_lines(state, work);

    frame.render_widget(
        Paragraph::new(lines).block(block).wrap(Wrap { trim: true }),
        area,
    );
}

fn detail_lines(state: &TuiState, work: &WorkSummary) -> Vec<Line<'static>> {
    let mut lines = vec![
        work_header(state, work),
        section("Goal"),
        Line::from(work.goal.clone()),
        section("Context"),
        label_value("intent", work.intent_id.clone()),
        label_value("folder", work_folder_label(work)),
    ];
    lines.extend(repository_context_lines(
        state.attached_repositories_for(work),
        state.repository_index_loading,
    ));
    lines
}

fn repository_context_lines(
    repositories: &[AttachedRepository],
    loading: bool,
) -> Vec<Line<'static>> {
    if loading && repositories.is_empty() {
        return vec![label_value("repos", "loading")];
    }

    if repositories.is_empty() {
        return vec![label_value("repos", "none")];
    }

    let mut lines = vec![label_value(
        "repos",
        attached_repository_count_label(repositories.len()),
    )];
    for (index, repository) in repositories.iter().enumerate() {
        lines.extend([
            Line::from(vec![
                Span::styled(format!("  {:02}  ", index + 1), theme::style_command()),
                Span::styled(
                    repository.name_with_owner.clone(),
                    theme::style_primary_text().add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("      branch ", theme::style_muted_text()),
                Span::styled(repository.branch.clone(), theme::style_muted_text()),
            ]),
        ]);
    }
    lines
}

fn attached_repository_count_label(count: usize) -> String {
    if count == 1 {
        "1 attached".to_string()
    } else {
        format!("{count} attached")
    }
}

fn render_intent_context(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &TuiState,
    regions: &mut Option<&mut RenderRegions>,
) {
    mark_region(regions, AnimationTarget::WorkQueue, area);
    let [catalog, detail] = if area.width < 86 {
        Layout::vertical([Constraint::Percentage(54), Constraint::Fill(1)]).areas(area)
    } else {
        Layout::horizontal([Constraint::Percentage(48), Constraint::Fill(1)]).areas(area)
    };
    render_intent_catalog(frame, catalog, state);
    render_intent_detail(frame, detail, state);
}

fn render_intent_catalog(frame: &mut Frame<'_>, area: Rect, state: &TuiState) {
    let indices = state.filtered_intent_indices();
    let title = Line::from(vec![
        Span::raw(" "),
        Span::styled("INTENTS", theme::style_panel_title()),
        Span::raw(" "),
        Span::styled(indices.len().to_string(), theme::style_command()),
        Span::raw(" "),
        if state.intent_filter.is_empty() {
            Span::raw("")
        } else {
            Span::styled(
                format!("find {}", state.intent_filter),
                theme::style_command(),
            )
        },
    ]);
    let block = panel_block_with_title(title, true);
    if indices.is_empty() {
        frame.render_widget(
            Paragraph::new(vec![Line::from("No intents match the filter.".dim())]).block(block),
            area,
        );
        return;
    }

    let selected = state.selected_intent.min(indices.len().saturating_sub(1));
    let items = indices
        .iter()
        .enumerate()
        .filter_map(|(row, index)| state.intents.get(*index).map(|intent| (row, intent)))
        .map(|(row, (id, summary))| {
            let current = id == &state.intent_current_id;
            let selected = row == selected;
            let anchor = if current {
                "@"
            } else if selected {
                ">>"
            } else {
                "  "
            };
            ListItem::new(vec![
                Line::from(vec![
                    Span::styled(anchor.to_string(), theme::style_command()),
                    Span::raw(" "),
                    if current {
                        status_badge("CURRENT", theme::style_current_badge())
                    } else {
                        status_badge("READY", theme::style_active_badge())
                    },
                    Span::raw(" "),
                    Span::styled(id.clone(), theme::style_primary_text()),
                ]),
                Line::from(vec![
                    Span::raw("     "),
                    Span::styled(summary.clone(), theme::style_muted_text()),
                ]),
            ])
        })
        .collect::<Vec<_>>();

    let mut list_state = ListState::default();
    list_state.select(Some(selected));
    let list = List::new(items)
        .block(block)
        .highlight_symbol("")
        .highlight_style(theme::style_selected_row_highlight());
    StatefulWidget::render(list, area, frame.buffer_mut(), &mut list_state);
}

fn render_intent_detail(frame: &mut Frame<'_>, area: Rect, state: &TuiState) {
    let block = panel_block("INTENT DETAIL", false);
    let mut lines = vec![
        section("WORK"),
        label_value("work", state.intent_work_title.clone()),
        label_value("current", state.intent_current_id.clone()),
        section("SELECTED"),
    ];

    if let Some((id, summary)) = state.selected_intent_summary() {
        lines.extend([
            label_value("intent", id),
            Line::from(summary),
            Line::from(""),
            Line::from(vec![
                key("enter"),
                "switch current Work intent ".dim(),
                key("esc"),
                "back".dim(),
            ]),
        ]);
    } else {
        lines.push(Line::from("No selected intent.".dim()));
    }

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
        state.repo.focus,
        state.repo.requires_workspace_setup(),
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
    repo_focus: RepoPane,
    repo_workspace_setup_required: bool,
) -> Vec<Span<'static>> {
    match mode {
        TuiMode::List if width < 88 => vec![
            key("type"),
            "find ".dim(),
            key("enter"),
            "open ".dim(),
            key("/"),
            "cmd ".dim(),
            key("esc"),
            "clear/quit".dim(),
        ],
        TuiMode::List if width < 112 => vec![
            key("type"),
            "find ".dim(),
            key("enter"),
            "open ".dim(),
            key("up/down"),
            "move ".dim(),
            key("/"),
            "commands ".dim(),
            key("esc"),
            "clear/quit".dim(),
        ],
        TuiMode::List | TuiMode::Search => vec![
            key("type"),
            "find ".dim(),
            key("enter"),
            "open ".dim(),
            key("up/down"),
            "move ".dim(),
            key("/"),
            "commands ".dim(),
            key("C-t"),
            if trace_visible {
                "hide trace ".dim()
            } else {
                "trace ".dim()
            },
            key("esc"),
            "clear/quit".dim(),
        ],
        TuiMode::Leader => vec![
            key("/n"),
            "new ".dim(),
            key("/a"),
            "archive ".dim(),
            key("/i"),
            "intent ".dim(),
            key("/r"),
            "repos ".dim(),
            key("/?"),
            "help ".dim(),
            key("/q"),
            "quit ".dim(),
            key("esc"),
            "cancel".dim(),
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
        TuiMode::Intents => vec![
            key("type"),
            "find ".dim(),
            key("up/down"),
            "intent ".dim(),
            key("enter"),
            "switch ".dim(),
            key("esc"),
            "back".dim(),
        ],
        TuiMode::Repos if repo_focus == RepoPane::AddPath => vec![
            key("tab"),
            "panel ".dim(),
            key("type"),
            "path ".dim(),
            key("backspace"),
            "edit ".dim(),
            key("enter"),
            "add ".dim(),
            key("esc"),
            "back".dim(),
        ],
        TuiMode::Repos if repo_focus == RepoPane::ConfiguredPaths => vec![
            key("tab"),
            "panel ".dim(),
            key("up/down"),
            "path ".dim(),
            key("space"),
            "remove ".dim(),
            key("enter"),
            "remove ".dim(),
            key("esc"),
            "back".dim(),
        ],
        TuiMode::Repos => {
            let mut spans = vec![key("tab"), "panel ".dim(), key("type"), "find ".dim()];
            if repo_workspace_setup_required {
                spans.extend([key("enter"), "apply ".dim(), key("esc"), "back".dim()]);
            } else {
                spans.extend([
                    key("space"),
                    "select ".dim(),
                    key("!"),
                    "force ".dim(),
                    key("enter"),
                    "apply ".dim(),
                    key("esc"),
                    "back".dim(),
                ]);
            }
            spans
        }
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
        TuiMode::Create => render_create(frame, area, state, regions),
        TuiMode::Archive => render_archive(frame, area, state, regions),
        TuiMode::Help => render_help(frame, area, regions),
        TuiMode::List | TuiMode::Search | TuiMode::Leader | TuiMode::Intents | TuiMode::Repos => {}
    }

    if let Some(toast) = &state.toast {
        render_toast(frame, area, toast, regions);
    }
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
        help_line("up/down", "move Work selection"),
        help_line("type", "filter Work queue"),
        help_line("/", "open command leader"),
        help_line("enter", "switch to highlighted Work"),
        help_line("/n", "work init"),
        help_line("/a", "archive highlighted Work"),
        help_line("/r", "repository context for highlighted Work"),
        help_line("/?", "help"),
        help_line("//", "insert slash in filter"),
        help_line("/q", "quit"),
        help_line("esc", "clear filter or close panel"),
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
    let mut spans = if state.is_current_work(work) {
        vec![status_badge("CURRENT", theme::style_current_badge())]
    } else {
        vec![status_badge("ACTIVE", theme::style_active_badge())]
    };
    spans.extend([
        Span::raw(" "),
        Span::styled(
            work.title.clone(),
            work_primary_style(state.is_current_work(work)),
        ),
    ]);
    Line::from(spans)
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
    control_panel::rect(area, TraceVisibility::from(state.trace_visible))
}

fn mode_style(mode: TuiMode) -> Style {
    match mode {
        TuiMode::Archive => theme::style_status_warn(),
        TuiMode::Create
        | TuiMode::Search
        | TuiMode::Leader
        | TuiMode::Intents
        | TuiMode::Repos
        | TuiMode::Help => theme::style_command(),
        TuiMode::List => theme::style_status_ok(),
    }
}

fn mode_label(mode: TuiMode) -> &'static str {
    match mode {
        TuiMode::List => "LIST",
        TuiMode::Search => "FILTER",
        TuiMode::Leader => "COMMAND",
        TuiMode::Create => "CREATE",
        TuiMode::Archive => "ARCHIVE",
        TuiMode::Intents => "INTENT",
        TuiMode::Repos => "REPOS",
        TuiMode::Help => "HELP",
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

    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::style::{Color, Modifier};
    use ratatui::Terminal;

    use super::{control_panel_rect, render, render_for_animation};
    use crate::domain::{
        AttachedRepository, AvailableRepository, RepositoryCandidate, RepositoryWorkspace,
        WorkList, WorkSummary,
    };
    use crate::interfaces::tui::animation::AnimationTarget;
    use crate::interfaces::tui::repo_state::RepoPane;
    use crate::interfaces::tui::state::{Toast, TuiMode, TuiState};

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
        assert!(!content.contains(".workon/archive"));
        assert!(content.contains("Goal"));
        assert!(content.contains("repos"));
        assert!(content.contains("none"));
        assert!(
            content.find("Goal").expect("goal section should render")
                < content
                    .find("Context")
                    .expect("context section should render")
        );
        assert!(!content.contains("ALL ACTIVE"));
        assert!(!content.contains("ACTIVE WORK"));
        assert!(!content.contains("work state"));
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
        assert_eq!(selected_active_title.fg, Color::Rgb(255, 140, 32));
        assert_eq!(selected_active_title.bg, Color::Rgb(92, 58, 32));
        assert!(selected_active_title.modifier.contains(Modifier::BOLD));
        assert_eq!(target_meta_key.fg, Color::Rgb(104, 72, 40));
        assert_eq!(target_meta_value.fg, Color::Rgb(255, 176, 64));
        assert_eq!(target_meta_value.bg, Color::Rgb(92, 58, 32));
        assert_eq!(active_badge.fg, Color::Rgb(190, 130, 70));
        assert_eq!(active_title.fg, Color::Rgb(255, 140, 32));
        assert!(active_title.modifier.contains(Modifier::BOLD));
        assert_eq!(active_meta_key.fg, Color::Rgb(104, 72, 40));
        assert_eq!(active_meta_value.fg, Color::Rgb(255, 176, 64));
        assert!(!active_meta_value.modifier.contains(Modifier::BOLD));
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
        assert!(!content.contains("ACTIVE WORK"));
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
        assert!(!content.contains("ACTIVE WORK"));
    }

    #[test]
    fn uses_more_horizontal_room_on_wide_terminals() {
        let state = TuiState::new(work_list());
        let panel = control_panel_rect(Rect::new(0, 0, 160, 36), &state);

        assert_eq!(panel.width, 158);
        assert_eq!(panel.height, 28);
        assert_eq!(panel.x, 1);
    }

    #[test]
    fn control_panel_shell_can_be_computed_without_full_tui_state() {
        let panel = crate::interfaces::tui::control_panel::rect(
            Rect::new(0, 0, 160, 36),
            crate::interfaces::tui::control_panel::TraceVisibility::Hidden,
        );

        assert_eq!(panel.width, 158);
        assert_eq!(panel.height, 28);
        assert_eq!(panel.x, 1);
        assert_eq!(
            crate::interfaces::tui::control_panel::title(TuiMode::Repos),
            "WORKON // CONTROL // REPO"
        );
    }

    #[test]
    fn renders_execution_trace_only_when_toggled_visible() {
        let mut state = TuiState::new(work_list());
        state.push_trace(
            crate::interfaces::tui::state::TraceKind::Run,
            "manual trace check",
        );

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
    fn renders_detail_panel_without_show_hide_toggle() {
        let state = TuiState::new(work_list());

        let content = render_content(&state, 120, 36);

        assert!(content.contains("Goal"));
        assert!(!content.contains(".workon/archive"));
        assert!(!content.contains("archive        "));
        assert!(!content.contains("C-d details"));
        assert!(!content.contains("C-d hide details"));
    }

    #[test]
    fn renders_attached_repositories_in_work_detail_context() {
        let state = TuiState::new(work_list()).with_attached_repositories(
            [("billing-retry-audit".to_string(), attached_repositories())]
                .into_iter()
                .collect(),
        );

        let content = render_content(&state, 120, 36);

        assert!(content.contains("Context"));
        assert!(content.contains("repos          1 attached"));
        assert!(content.contains("01  openai/workon"));
        assert!(content.contains("branch workon/billing-retry-audit"));
        assert!(content.contains("openai/workon"));
        assert!(content.contains("workon/billing-retry-audit"));
        assert!(!content.contains("repo           openai/workon workon/billing-retry-audit"));
        assert!(!content.contains("view           ALL ACTIVE"));
    }

    #[test]
    fn renders_direct_filter_and_leader_shortcuts_in_list_footer() {
        let full = render_content(&TuiState::new(work_list()), 120, 36);
        assert!(full.contains("type find"));
        assert!(full.contains("enter open"));
        assert!(full.contains("up/down move"));
        assert!(full.contains("/ commands"));
        assert!(full.contains("esc clear/quit"));
        assert!(!full.contains("/ find"));
        assert!(!full.contains("a archive"));

        let compact = render_content(&TuiState::new(work_list()), 80, 24);
        assert!(compact.contains("type find"));
        assert!(compact.contains("/ cmd"));
    }

    #[test]
    fn renders_inline_queue_filter_with_query_and_count() {
        let mut state = TuiState::new(work_list());
        state.mode = TuiMode::Search;
        state.filter = "billing".to_string();

        let content = render_content(&state, 120, 36);

        assert!(!content.contains("SIGNAL FILTER"));
        assert!(content.contains("WORK QUEUE"));
        assert!(content.contains("find billing"));
        assert!(content.contains("1 match"));
        assert!(content.contains("enter open"));
        assert!(content.contains("esc clear/quit"));
    }

    #[test]
    fn renders_leader_mode_inline_without_overlay() {
        let mut state = TuiState::new(work_list());
        state.mode = TuiMode::Leader;

        let content = render_content(&state, 120, 36);
        let (_, regions) = render_content_for_animation(&state, 120, 36);

        assert!(content.contains("OPERATOR COMMAND"));
        assert!(content.contains("COMMAND"));
        assert!(content.contains("/n new"));
        assert!(content.contains("/a archive"));
        assert!(content.contains("/? help"));
        assert!(content.contains("/q quit"));
        assert!(content.contains("esc cancel"));
        assert!(!content.contains("WORK INIT"));
        assert!(!content.contains("SIGNAL FILTER"));
        assert!(!regions.has_region(AnimationTarget::Overlay));
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
        assert!(content.contains("up/down"));
        assert!(content.contains("type"));
        assert!(content.contains("/n"));
        assert!(!content.contains("operator command"));
    }

    #[test]
    fn renders_repo_context_view_as_repository_catalog() {
        let mut state = TuiState::new(work_list());
        state.enter_repo_context(
            "billing-retry-audit".to_string(),
            "Billing retry audit".to_string(),
            available_repositories(),
            Vec::new(),
            attached_repositories(),
            repo_workspaces(),
        );

        let attached = render_content(&state, 120, 36);
        assert!(attached.contains("WORKON // CONTROL // REPO"));
        assert!(!attached.contains("REPO CONTEXT"));
        assert!(attached.contains("REPOSITORIES"));
        assert!(attached.contains("ADD PATH"));
        assert!(attached.contains("CONFIGURED PATHS"));
        assert!(!attached.contains("SELECTED FOR WORK"));
        assert!(!attached.contains("REPO WORKSPACES"));
        assert!(!attached.contains("No repositories selected for this Work."));
        assert!(attached.contains("openai/workon"));
        assert!(attached.contains("GitHub worktrees in /tmp/repos; local links in place."));
        assert!(attached.contains("/tmp/repos/billing-retry-audit/workon"));
        assert!(!attached.contains("type filters repos"));
        assert!(!attached.contains("github default"));

        state.repo.filter = "api".to_string();
        let add = render_content(&state, 120, 36);
        assert!(add.contains("Showing repositories matching `api`."));
        assert!(add.contains("openai/api"));
        assert!(add.contains("openai/api-docs"));
        assert!(!add.contains("openai/workon"));
    }

    #[test]
    fn renders_repo_context_workspace_setup_as_inline_side_panels() {
        let mut state = TuiState::new(work_list());
        state.enter_repo_context(
            "billing-retry-audit".to_string(),
            "Billing retry audit".to_string(),
            available_repositories(),
            Vec::new(),
            attached_repositories(),
            Vec::new(),
        );

        let content = render_content(&state, 120, 36);

        assert!(content.contains("REPOSITORIES"));
        assert!(content.contains("ADD PATH"));
        assert!(content.contains("CONFIGURED PATHS"));
        assert!(!content.contains("REPO WORKSPACES"));
        assert!(!content.contains("REPO WORKSPACE SETUP"));
        assert!(content.contains("REQUIRED"));
        assert!(content.contains("Add a repo workspace path before creating GitHub repos."));
        assert!(content.contains("<type folder path>"));
        assert!(!content.contains("SELECTED FOR WORK"));
        assert!(!content.contains("wo repos workspace add"));
        assert!(!content.contains("space selects repos"));
        assert!(!content.contains("type workspace path"));
    }

    #[test]
    fn renders_repo_context_scrolls_catalog_to_selected_repository() {
        let mut state = TuiState::new(work_list());
        state.enter_repo_context(
            "billing-retry-audit".to_string(),
            "Billing retry audit".to_string(),
            many_available_repositories(30),
            Vec::new(),
            attached_repositories(),
            repo_workspaces(),
        );
        state.repo.selected_catalog = 25;

        let content = render_content(&state, 100, 18);

        assert!(content.contains("openai/repo-24"));
        assert!(content.contains(">>"));
        assert!(!content.contains("openai/repo-00"));
    }

    #[test]
    fn renders_repo_context_local_repository_candidates() {
        let mut state = TuiState::new(work_list());
        state.enter_repo_context(
            "billing-retry-audit".to_string(),
            "Billing retry audit".to_string(),
            available_repositories(),
            local_candidates(),
            attached_repositories(),
            repo_workspaces(),
        );

        let content = render_content(&state, 120, 36);

        assert!(content.contains("openai/local-tool"));
        assert!(!content.contains("local feature/workon /tmp/repos/local-tool"));
    }

    #[test]
    fn renders_repo_context_rows_as_compact_columns_with_selected_detail() {
        let home = PathBuf::from(std::env::var("HOME").expect("HOME should be set for tests"));
        let local_path = home.join("Projects/client/local-tool");
        let mut state = TuiState::new(work_list());
        state.enter_repo_context(
            "billing-retry-audit".to_string(),
            "Billing retry audit".to_string(),
            Vec::new(),
            vec![RepositoryCandidate {
                name_with_owner: "openai/local-tool".to_string(),
                branch: "feature/workon".to_string(),
                path: local_path.clone(),
                url: "https://github.com/openai/local-tool".to_string(),
            }],
            Vec::new(),
            vec![RepositoryWorkspace {
                path: home.join("Projects"),
            }],
        );

        let content = render_content(&state, 160, 36);
        let buffer = render_buffer(&state, 160, 36);
        let row = row_containing(&buffer, "openai/local-tool");
        let header_text = row_text(&buffer, row - 1);
        let local_row_text = row_text(&buffer, row);
        let detail_text = row_text(&buffer, row + 1);

        assert!(header_text.contains("REPOSITORY"));
        assert!(header_text.contains("SOURCE"));
        assert!(header_text.contains("BRANCH"));
        assert!(header_text.contains("STATE"));
        assert!(local_row_text.contains("openai/local-tool"));
        assert!(local_row_text.contains("local"));
        assert!(local_row_text.contains("feature/workon"));
        assert!(!local_row_text.contains("./client/local-tool"));
        assert!(!local_row_text.contains("local feature/workon"));
        assert!(detail_text.contains("path "));
        assert!(detail_text.contains("~/Projects/client/local-tool"));
        assert!(detail_text.contains(" | work "));
        assert!(detail_text.contains("./client/local-tool"));
        assert!(!detail_text.contains("remote"));
        assert!(!detail_text.contains("https://github.com/openai/local-tool"));
        assert!(!content.contains(&local_path.display().to_string()));
    }

    #[test]
    fn gives_repository_catalog_more_horizontal_space_than_workspace_panels() {
        let mut state = TuiState::new(work_list());
        state.enter_repo_context(
            "billing-retry-audit".to_string(),
            "Billing retry audit".to_string(),
            available_repositories(),
            local_candidates(),
            attached_repositories(),
            repo_workspaces(),
        );

        let buffer = render_buffer(&state, 160, 36);

        assert!(
            x_containing(&buffer, "ADD PATH") > 116,
            "workspace panels should start after a wider repository catalog"
        );
    }

    #[test]
    fn renders_repo_context_workspace_selector_when_multiple_paths_exist() {
        let mut state = TuiState::new(work_list());
        state.enter_repo_context(
            "billing-retry-audit".to_string(),
            "Billing retry audit".to_string(),
            available_repositories(),
            local_candidates(),
            attached_repositories(),
            multiple_repo_workspaces(),
        );
        state.repo.focus = RepoPane::ConfiguredPaths;
        state.repo.selected_workspace = 1;

        let content = render_content(&state, 160, 36);

        assert!(content.contains("GitHub worktrees in /tmp/client-repos; local links in place."));
        assert!(content.contains("/tmp/client-repos/billing-retry-audit/workon"));
        assert!(content.contains("ADD PATH"));
        assert!(content.contains("CONFIGURED PATHS"));
        assert!(!content.contains("CREATE IN"));
        assert!(content.contains("openai/local-tool"));
    }

    #[test]
    fn renders_repo_context_add_path_panel_input() {
        let mut state = TuiState::new(work_list());
        state.enter_repo_context(
            "billing-retry-audit".to_string(),
            "Billing retry audit".to_string(),
            available_repositories(),
            local_candidates(),
            attached_repositories(),
            repo_workspaces(),
        );
        state
            .repo
            .handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        for character in "/tmp/more-repos".chars() {
            state
                .repo
                .handle_key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE));
        }

        let content = render_content(&state, 120, 36);

        assert!(!content.contains("REPO WORKSPACES"));
        assert!(content.contains("ADD PATH"));
        assert!(content.contains("CONFIGURED PATHS"));
        assert!(content.contains("/tmp/repos"));
        assert!(content.contains("/tmp/more-repos"));
        assert!(content.contains("type path"));
        assert!(content.contains("enter add"));
    }

    #[test]
    fn renders_repo_context_footer_hints_for_each_repo_panel() {
        let mut state = TuiState::new(work_list());
        state.enter_repo_context(
            "billing-retry-audit".to_string(),
            "Billing retry audit".to_string(),
            available_repositories(),
            Vec::new(),
            attached_repositories(),
            multiple_repo_workspaces(),
        );

        let repositories = render_content(&state, 120, 36);
        assert!(repositories.contains("type find"));
        assert!(repositories.contains("space select"));
        assert!(repositories.contains("enter apply"));

        state.repo.focus = RepoPane::AddPath;
        let add_path = render_content(&state, 120, 36);
        assert!(add_path.contains("type path"));
        assert!(add_path.contains("backspace edit"));
        assert!(add_path.contains("enter add"));
        assert!(!add_path.contains("space select"));

        state.repo.focus = RepoPane::ConfiguredPaths;
        let configured_paths = render_content(&state, 120, 36);
        assert!(configured_paths.contains("up/down path"));
        assert!(configured_paths.contains("space remove"));
        assert!(configured_paths.contains("enter remove"));
        assert!(!configured_paths.contains("type find"));
    }

    #[test]
    fn renders_repo_context_loading_state() {
        let mut state = TuiState::new(work_list());
        state.enter_repo_loading(
            "billing-retry-audit".to_string(),
            "Billing retry audit".to_string(),
        );

        let content = render_content(&state, 120, 36);
        let title = content
            .lines()
            .find(|line| line.contains("WORKON // CONTROL // REPO"))
            .expect("control title should render");

        assert!(title.contains("Loading: attached, workspaces, local, GitHub"));
        assert!(content.contains("Loading: attached, workspaces, local, GitHub"));
        assert!(!content.contains("LOAD Loading: attached, workspaces, local, GitHub"));
        assert!(!content.contains("REPO WORKSPACE SETUP"));
        assert!(!content.contains("repo workspace setup required"));
    }

    #[test]
    fn renders_available_repo_rows_while_sources_are_loading() {
        let mut state = TuiState::new(work_list());
        state.enter_repo_context(
            "billing-retry-audit".to_string(),
            "Billing retry audit".to_string(),
            Vec::new(),
            Vec::new(),
            attached_repositories(),
            repo_workspaces(),
        );
        state.start_repo_loading("Loading repository sources");

        let content = render_content(&state, 120, 36);

        assert!(content.contains("Loading repository sources"));
        assert!(content.contains("openai/workon"));
    }

    #[test]
    fn animates_repo_context_loading_title() {
        let mut state = TuiState::new(work_list());
        state.enter_repo_loading(
            "billing-retry-audit".to_string(),
            "Billing retry audit".to_string(),
        );

        let first = render_content(&state, 120, 36);
        state.advance_activity_frame();
        let second = render_content(&state, 120, 36);

        let first_title = first
            .lines()
            .find(|line| line.contains("WORKON // CONTROL // REPO"))
            .expect("control title should render");
        let second_title = second
            .lines()
            .find(|line| line.contains("WORKON // CONTROL // REPO"))
            .expect("control title should render");

        assert_ne!(first_title, second_title);
        assert!(first_title.contains("⣾▉"));
        assert!(second_title.contains("⣽▊"));
        assert!(!first_title.contains("SYNC"));
        assert!(!second_title.contains("SYNC"));
        assert!(!first_title.contains("MAGI"));
        assert!(!second_title.contains("MAGI"));
        assert!(!first_title.contains("[-]"));
        assert!(!second_title.contains("[\\]"));
        assert!(first_title.contains("Loading: attached, workspaces, local, GitHub"));
        assert!(second_title.contains("Loading: attached, workspaces, local, GitHub"));
    }

    #[test]
    fn animates_repo_context_apply_title() {
        let mut state = TuiState::new(work_list());
        state.enter_repo_context(
            "billing-retry-audit".to_string(),
            "Billing retry audit".to_string(),
            available_repositories(),
            Vec::new(),
            attached_repositories(),
            repo_workspaces(),
        );
        state.start_repo_step(
            crate::interfaces::tui::repo_state::RepoOperation::Remove,
            1,
            1,
            "openai/workon",
        );

        let first = render_content(&state, 120, 36);
        state.advance_activity_frame();
        let second = render_content(&state, 120, 36);

        let first_title = first
            .lines()
            .find(|line| line.contains("WORKON // CONTROL // REPO"))
            .expect("control title should render");
        let second_title = second
            .lines()
            .find(|line| line.contains("WORKON // CONTROL // REPO"))
            .expect("control title should render");

        assert_ne!(first_title, second_title);
        assert!(first_title.contains("⣾▉"));
        assert!(second_title.contains("⣽▊"));
        assert!(!first_title.contains("SYNC"));
        assert!(!second_title.contains("SYNC"));
        assert!(first_title.contains("1/1 Removing openai/workon"));
        assert!(second_title.contains("1/1 Removing openai/workon"));
    }

    #[test]
    fn renders_repo_context_progress_and_operation_log() {
        let mut state = TuiState::new(work_list());
        state.enter_repo_context(
            "billing-retry-audit".to_string(),
            "Billing retry audit".to_string(),
            available_repositories(),
            Vec::new(),
            attached_repositories(),
            repo_workspaces(),
        );
        state.start_repo_step(
            crate::interfaces::tui::repo_state::RepoOperation::Add,
            1,
            2,
            "openai/api-docs",
        );
        state.push_repo_log(
            crate::interfaces::tui::state::TraceKind::Sync,
            "attached openai/api-docs",
        );

        let content = render_content(&state, 120, 36);

        assert!(content.contains("1/2"));
        assert!(content.contains("openai/api-docs"));
        assert!(!content.contains("CLONE"));
        assert!(!content.contains("cancel remaining"));
        assert!(content.contains("OPERATION LOG"));
        assert!(content.contains("attached openai/api-docs"));
    }

    #[test]
    fn renders_empty_list_and_empty_filter_result() {
        let empty = render_content(&TuiState::new(WorkList { works: Vec::new() }), 100, 28);
        assert!(empty.contains("No active Work. Press /n to create Work."));

        let mut state = TuiState::new(work_list());
        state.filter = "missing".to_string();
        let filtered = render_content(&state, 100, 28);

        assert!(filtered.contains("No active Work matches \"missing\"."));
        assert!(filtered.contains("No active Work. Press /n to create Work."));
    }

    #[test]
    fn renders_long_goal_and_path_without_losing_context_labels() {
        let state = TuiState::new(WorkList {
            works: vec![WorkSummary {
                title: "Long context review".to_string(),
                slug: "long-context-review".to_string(),
                goal: "Review the rollout decision, compare the operational evidence, preserve the tradeoffs, and identify the smallest next step before implementation.".to_string(),
                intent_id: "review-pr".to_string(),
                path: PathBuf::from("/tmp/workon/.workon/work/long-context-review/with/a/deep/path/that/should/wrap"),
            }],
        });

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
        assert!(!regions.has_region(AnimationTarget::Overlay));
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

    fn available_repositories() -> Vec<AvailableRepository> {
        vec![
            AvailableRepository {
                name_with_owner: "openai/api".to_string(),
                default_branch: "main".to_string(),
                url: "https://github.com/openai/api".to_string(),
                ssh_url: "git@github.com:openai/api.git".to_string(),
            },
            AvailableRepository {
                name_with_owner: "openai/workon".to_string(),
                default_branch: "trunk".to_string(),
                url: "https://github.com/openai/workon".to_string(),
                ssh_url: "git@github.com:openai/workon.git".to_string(),
            },
            AvailableRepository {
                name_with_owner: "openai/api-docs".to_string(),
                default_branch: "main".to_string(),
                url: "https://github.com/openai/api-docs".to_string(),
                ssh_url: "git@github.com:openai/api-docs.git".to_string(),
            },
        ]
    }

    fn many_available_repositories(count: usize) -> Vec<AvailableRepository> {
        (0..count)
            .map(|index| AvailableRepository {
                name_with_owner: format!("openai/repo-{index:02}"),
                default_branch: "main".to_string(),
                url: format!("https://github.com/openai/repo-{index:02}"),
                ssh_url: format!("git@github.com:openai/repo-{index:02}.git"),
            })
            .collect()
    }

    fn local_candidates() -> Vec<RepositoryCandidate> {
        vec![RepositoryCandidate {
            name_with_owner: "openai/local-tool".to_string(),
            branch: "feature/workon".to_string(),
            path: PathBuf::from("/tmp/repos/local-tool"),
            url: "https://github.com/openai/local-tool".to_string(),
        }]
    }

    fn attached_repositories() -> Vec<AttachedRepository> {
        vec![AttachedRepository {
            name_with_owner: "openai/workon".to_string(),
            branch: "workon/billing-retry-audit".to_string(),
            path: PathBuf::from(
                "/tmp/workon/.workon/work/billing-retry-audit/repos/openai__workon",
            ),
            default_branch: "trunk".to_string(),
            url: "https://github.com/openai/workon".to_string(),
        }]
    }

    fn repo_workspaces() -> Vec<RepositoryWorkspace> {
        vec![RepositoryWorkspace {
            path: PathBuf::from("/tmp/repos"),
        }]
    }

    fn multiple_repo_workspaces() -> Vec<RepositoryWorkspace> {
        vec![
            RepositoryWorkspace {
                path: PathBuf::from("/tmp/repos"),
            },
            RepositoryWorkspace {
                path: PathBuf::from("/tmp/client-repos"),
            },
        ]
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
    ) -> (String, crate::interfaces::tui::animation::RenderRegions) {
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
            let row = row_text(buffer, y);
            if row.contains(text) {
                return y;
            }
        }

        panic!("buffer did not contain row text: {text}");
    }

    fn row_text(buffer: &Buffer, y: u16) -> String {
        let mut row = String::new();
        for x in 0..buffer.area.width {
            row.push_str(buffer[(x, y)].symbol());
        }
        row
    }

    fn x_containing(buffer: &Buffer, text: &str) -> u16 {
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                let mut candidate = String::new();
                for end in x..buffer.area.width {
                    candidate.push_str(buffer[(end, y)].symbol());
                    if candidate == text {
                        return x;
                    }
                    if !text.starts_with(&candidate) || candidate.len() >= text.len() {
                        break;
                    }
                }
            }
        }

        panic!("buffer did not contain text: {text}");
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
