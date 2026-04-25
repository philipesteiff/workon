use std::path::PathBuf;

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::prelude::{Frame, Line, Span, Stylize};
use ratatui::style::Style;
use ratatui::widgets::{
    Block, Borders, Clear, List, ListItem, ListState, Paragraph, StatefulWidget, Wrap,
};

use crate::domain::WorkSummary;
use crate::slug::{slugify, title_from_goal};

use super::state::{Toast, ToastKind, TuiMode, TuiState};

pub(super) fn render(frame: &mut Frame<'_>, state: &TuiState) {
    let area = frame.area();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().dark_gray());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let [topbar, workspace, footer] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Fill(1),
        Constraint::Length(2),
    ])
    .areas(inner);

    render_topbar(frame, topbar, state);
    render_workspace(frame, workspace, state);
    render_footer(frame, footer, state);
    render_overlay(frame, area, state);
}

fn render_topbar(frame: &mut Frame<'_>, area: Rect, state: &TuiState) {
    let [brand, path, mode] = Layout::horizontal([
        Constraint::Length(14),
        Constraint::Fill(1),
        Constraint::Length(16),
    ])
    .areas(area);

    frame.render_widget(
        Paragraph::new(Line::from(vec![" wo ".bold().on_green(), " work".bold()])),
        brand,
    );
    frame.render_widget(
        Paragraph::new(root_label(state).dim()).block(bottom_border()),
        path,
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            "mode ".dim(),
            mode_label(state.mode).green().bold(),
        ]))
        .block(bottom_border()),
        mode,
    );
}

fn render_workspace(frame: &mut Frame<'_>, area: Rect, state: &TuiState) {
    if area.width < 86 {
        let [list, detail] =
            Layout::vertical([Constraint::Percentage(45), Constraint::Fill(1)]).areas(area);
        render_work_list(frame, list, state);
        render_detail(frame, detail, state);
    } else {
        let [list, detail] =
            Layout::horizontal([Constraint::Percentage(42), Constraint::Fill(1)]).areas(area);
        render_work_list(frame, list, state);
        render_detail(frame, detail, state);
    }
}

fn render_work_list(frame: &mut Frame<'_>, area: Rect, state: &TuiState) {
    let works = state.filtered_works();
    let title = Line::from(vec![
        " Active Work ".bold(),
        format!("{} ", works.len()).dim(),
    ]);
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::new().dark_gray());

    if works.is_empty() {
        let message = if state.filter.is_empty() {
            "No active Work. Press n to create Work.".to_string()
        } else {
            format!("No active Work matches \"{}\".", state.filter)
        };
        frame.render_widget(
            Paragraph::new(message.dim())
                .block(block)
                .wrap(Wrap { trim: true }),
            area,
        );
        return;
    }

    let items = works
        .iter()
        .map(|work| {
            ListItem::new(vec![
                Line::from(work.title.clone().bold()),
                Line::from(vec![
                    work.intent_id.clone().cyan(),
                    "  ".into(),
                    work.slug.clone().dim(),
                ]),
            ])
        })
        .collect::<Vec<_>>();

    let mut list_state = ListState::default();
    list_state.select(Some(state.selected.min(items.len().saturating_sub(1))));
    let list = List::new(items)
        .block(block)
        .highlight_symbol("> ")
        .highlight_style(Style::new().green().bold());
    StatefulWidget::render(list, area, frame.buffer_mut(), &mut list_state);
}

fn render_detail(frame: &mut Frame<'_>, area: Rect, state: &TuiState) {
    let block = Block::default()
        .title(" Selected Work ".bold())
        .borders(Borders::ALL)
        .border_style(Style::new().dark_gray());

    let Some(work) = state.selected_work() else {
        frame.render_widget(
            Paragraph::new("No Work selected. Press n to create Work.".dim())
                .block(block)
                .wrap(Wrap { trim: true }),
            area,
        );
        return;
    };

    let lines = vec![
        Line::from(work.title.clone().bold()),
        Line::from(work.slug.clone().dim()),
        Line::from("Goal".dim()),
        Line::from(work.goal.clone()),
        Line::from("Context".dim()),
        Line::from(vec!["intent        ".dim(), work.intent_id.clone().cyan()]),
        Line::from(vec!["folder        ".dim(), work_folder_label(work).into()]),
    ];

    frame.render_widget(
        Paragraph::new(lines).block(block).wrap(Wrap { trim: true }),
        area,
    );
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, state: &TuiState) {
    let line = match state.mode {
        TuiMode::List => Line::from(vec![
            key("enter"),
            " switch ".dim(),
            key("n"),
            " create ".dim(),
            key("/"),
            " filter ".dim(),
            key("?"),
            " keys ".dim(),
            key("q"),
            " quit".dim(),
        ]),
        TuiMode::Search => Line::from(vec![
            key("enter"),
            " apply ".dim(),
            key("esc"),
            " close ".dim(),
            key("j/k"),
            " move".dim(),
        ]),
        TuiMode::Command => Line::from(vec![
            key("enter"),
            " run ".dim(),
            key("esc"),
            " cancel".dim(),
        ]),
        TuiMode::Create | TuiMode::Archive | TuiMode::Help => Line::from(""),
    };
    frame.render_widget(Paragraph::new(line), area);
}

fn render_overlay(frame: &mut Frame<'_>, area: Rect, state: &TuiState) {
    match state.mode {
        TuiMode::Search => render_search(frame, area, state),
        TuiMode::Command => render_command(frame, area, state),
        TuiMode::Create => render_create(frame, area, state),
        TuiMode::Archive => render_archive(frame, area, state),
        TuiMode::Help => render_help(frame, area),
        TuiMode::List => {}
    }

    if let Some(toast) = &state.toast {
        render_toast(frame, area, toast);
    }
}

fn render_search(frame: &mut Frame<'_>, area: Rect, state: &TuiState) {
    let popup = top_popup(70, 7, area);
    let lines = vec![
        Line::from(vec![
            "/ ".green().bold(),
            if state.filter.is_empty() {
                " ".into()
            } else {
                state.filter.clone().bold()
            },
        ]),
        Line::from("fields title, slug, intent, goal".dim()),
        Line::from(match_count_label(state.filtered_indices().len()).dim()),
        Line::from(vec![
            key("enter"),
            " apply ".dim(),
            key("esc"),
            " close".dim(),
        ]),
    ];
    render_popup(frame, popup, " Filter Work ", lines, Style::new().green());
}

fn render_command(frame: &mut Frame<'_>, area: Rect, state: &TuiState) {
    let popup = top_popup(70, 6, area);
    let lines = vec![
        Line::from(vec![
            ": ".green().bold(),
            if state.command.is_empty() {
                " ".into()
            } else {
                state.command.clone().bold()
            },
        ]),
        Line::from("commands list, create, switch, archive".dim()),
        Line::from(vec![
            key("enter"),
            " run ".dim(),
            key("esc"),
            " cancel".dim(),
        ]),
    ];
    render_popup(frame, popup, " Command ", lines, Style::new().green());
}

fn render_create(frame: &mut Frame<'_>, area: Rect, state: &TuiState) {
    let popup = centered_rect(76, 52, area);
    let mut lines = vec![
        Line::from("Goal".dim()),
        Line::from(if state.create_goal.is_empty() {
            "Type the Work goal.".dim()
        } else {
            state.create_goal.clone().into()
        }),
        Line::from(""),
        Line::from("Intent".dim()),
    ];
    lines.extend(selected_intent_lines(state));

    if !state.create_goal.trim().is_empty() {
        let title = title_from_goal(&state.create_goal);
        let slug = slugify(&title);
        lines.extend([
            Line::from(""),
            Line::from("Slug preview".dim()),
            Line::from(slug),
        ]);
    }

    lines.extend([
        Line::from(""),
        Line::from(vec![
            key("enter"),
            " create ".dim(),
            key("tab/down"),
            " intent ".dim(),
            key("up"),
            " previous ".dim(),
            key("esc"),
            " cancel".dim(),
        ]),
    ]);
    render_popup(frame, popup, " Create Work ", lines, Style::new().green());
}

fn render_archive(frame: &mut Frame<'_>, area: Rect, state: &TuiState) {
    let popup = centered_rect(70, 62, area);
    let Some(work) = state.selected_work() else {
        return;
    };
    let archive_path = archive_path_for(work);
    let lines = vec![
        Line::from(vec![
            "Archive ".into(),
            work.title.clone().bold(),
            "?".into(),
        ]),
        Line::from(""),
        Line::from(vec![
            "from   ".dim(),
            work.path.display().to_string().into(),
        ]),
        Line::from(vec![
            "to     ".dim(),
            archive_path.display().to_string().into(),
        ]),
        Line::from(vec!["effect ".dim(), "Hidden from Active Work.".red()]),
        Line::from(""),
        Line::from(vec![
            key("y/enter"),
            " archive ".dim(),
            key("esc/n"),
            " cancel".dim(),
        ]),
    ];
    render_popup(frame, popup, " Archive Work ", lines, Style::new().red());
}

fn render_help(frame: &mut Frame<'_>, area: Rect) {
    let popup = centered_rect(62, 48, area);
    let lines = vec![
        help_line("j/down", "next Work"),
        help_line("k/up", "previous Work"),
        help_line("/", "filter like fzf"),
        help_line("enter", "switch to selected Work"),
        help_line("n", "create Work"),
        help_line("a", "archive selected Work"),
        help_line(":", "command line"),
        help_line("esc", "close panel"),
        Line::from(""),
        Line::from("Commands: list, create, switch, archive".dim()),
    ];
    render_popup(frame, popup, " Keys ", lines, Style::new().green());
}

fn render_toast(frame: &mut Frame<'_>, area: Rect, toast: &Toast) {
    let width = area.width.min(58);
    let height = 5;
    let x = area.x + area.width.saturating_sub(width + 2);
    let y = area.y + area.height.saturating_sub(height + 2);
    let popup = Rect::new(x, y, width, height);
    let style = match toast.kind {
        ToastKind::Info => Style::new().green(),
        ToastKind::Error => Style::new().red(),
    };
    let lines = vec![
        Line::from(toast.title.clone().bold()),
        Line::from(toast.message.clone().dim()),
    ];
    render_popup(frame, popup, " Last Result ", lines, style);
}

fn render_popup(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &'static str,
    lines: Vec<Line<'_>>,
    border_style: Style,
) {
    frame.render_widget(Clear, area);
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);
    frame.render_widget(
        Paragraph::new(lines).block(block).wrap(Wrap { trim: true }),
        area,
    );
}

fn bottom_border<'a>() -> Block<'a> {
    Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::new().dark_gray())
}

fn key(value: &'static str) -> Span<'static> {
    format!(" {value}").bold().green()
}

fn help_line(key_value: &'static str, label: &'static str) -> Line<'static> {
    Line::from(vec![key(key_value), " ".into(), label.dim()])
}

fn selected_intent_lines(state: &TuiState) -> Vec<Line<'static>> {
    match state.intents.get(state.create_intent) {
        Some((intent, summary)) => vec![
            Line::from(intent.clone().cyan().bold()),
            Line::from(summary.clone().dim()),
        ],
        None => vec![Line::from("investigate".cyan().bold())],
    }
}

fn mode_label(mode: TuiMode) -> &'static str {
    match mode {
        TuiMode::List => "list",
        TuiMode::Search => "search",
        TuiMode::Command => "command",
        TuiMode::Create => "create",
        TuiMode::Archive => "archive",
        TuiMode::Help => "help",
    }
}

fn root_label(state: &TuiState) -> String {
    if state.root.as_os_str().is_empty() {
        ".workon".to_string()
    } else {
        state.root.join(".workon").display().to_string()
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
    let width = width.clamp(30, area.width);
    Rect::new(area.x + (area.width - width) / 2, area.y + 2, width, height)
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
    use ratatui::Terminal;

    use super::render;
    use crate::domain::{WorkList, WorkSummary};
    use crate::tui::state::{Toast, TuiMode, TuiState};

    #[test]
    fn renders_work_list_and_selected_detail() {
        let state = TuiState::new(work_list());
        let content = render_content(&state, 120, 36);

        assert!(content.contains("wo"));
        assert!(content.contains("Active Work"));
        assert!(content.contains("Selected Work"));
        assert!(content.contains("Billing retry audit"));
        assert!(content.contains("Goal"));
        assert!(content.contains("intent"));
        assert!(content.contains("folder"));
        assert!(!content.contains("switch result"));
        assert!(!content.contains("Actions"));
        assert!(!content.contains("Trail"));
        assert!(!content.contains("Filtered Work"));
        assert!(!content.contains("active"));
    }

    #[test]
    fn renders_search_overlay_with_query_fields_and_count() {
        let mut state = TuiState::new(work_list());
        state.mode = TuiMode::Search;
        state.filter = "billing".to_string();

        let content = render_content(&state, 120, 36);

        assert!(content.contains("Filter Work"));
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

        assert!(content.contains("Create Work"));
        assert!(content.contains("Type the Work goal."));
        assert!(content.contains("investigate"));
        assert!(content.contains("Answer a technical question with evidence."));
        assert!(content.contains("enter create"));
        assert!(content.contains("tab/down intent"));
        assert!(!content.contains("Slug preview"));
        assert!(!content.contains("Answer why billing retry alerts spiked"));
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

        assert!(content.contains("Archive Work"));
        assert!(content.contains("Archive Billing retry audit?"));
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

        assert!(content.contains("Keys"));
        assert!(content.contains("j/down"));
        assert!(content.contains("command line"));
        assert!(content.contains("Commands: list, create, switch, archive"));
    }

    #[test]
    fn renders_empty_list_and_empty_filter_result() {
        let empty = render_content(&TuiState::new(WorkList { works: Vec::new() }), 100, 28);
        assert!(empty.contains("No active Work. Press n to create Work."));

        let mut state = TuiState::new(work_list());
        state.filter = "missing".to_string();
        let filtered = render_content(&state, 100, 28);

        assert!(filtered.contains("No active Work matches \"missing\"."));
        assert!(filtered.contains("Selected Work"));
        assert!(filtered.contains("No Work selected. Press n to create Work."));
        assert!(!filtered.contains("Filtered Work"));
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

        assert!(content.contains("Selected Work"));
        assert!(content.contains("Goal"));
        assert!(content.contains("intent"));
        assert!(content.contains("folder"));
    }

    #[test]
    fn renders_core_context_in_default_terminal_size() {
        let state = TuiState::new(work_list());
        let content = render_content(&state, 80, 24);

        assert!(content.contains("Selected Work"));
        assert!(content.contains("Goal"));
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

        assert!(content.contains("Last Result"));
        assert!(content.contains("Work created"));
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

    fn render_content(state: &TuiState, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height))
            .expect("test terminal should initialize");

        terminal
            .draw(|frame| render(frame, state))
            .expect("render should succeed");

        format!("{}", terminal.backend())
    }
}
