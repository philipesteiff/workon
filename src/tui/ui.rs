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

    let [topbar, command_strip, workspace, footer] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Fill(1),
        Constraint::Length(2),
    ])
    .areas(inner);

    render_topbar(frame, topbar, state);
    render_command_strip(frame, command_strip, state);
    render_workspace(frame, workspace, state);
    render_footer(frame, footer);
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

fn render_command_strip(frame: &mut Frame<'_>, area: Rect, state: &TuiState) {
    let [prompt, command, count] = Layout::horizontal([
        Constraint::Length(5),
        Constraint::Fill(1),
        Constraint::Length(14),
    ])
    .areas(area);

    let filtered_count = state.filtered_indices().len();
    let command_line = match state.mode {
        TuiMode::Search => Line::from(vec![
            "filter ".bold(),
            if state.filter.is_empty() {
                "active Work".dim()
            } else {
                state.filter.clone().into()
            },
        ]),
        TuiMode::Command => Line::from(vec![
            "command ".bold(),
            if state.command.is_empty() {
                "list, create, switch, archive".dim()
            } else {
                state.command.clone().into()
            },
        ]),
        _ => Line::from(vec![
            "work list".bold(),
            " - type to filter, enter to switch".dim(),
        ]),
    };

    frame.render_widget(Paragraph::new(prompt_label(state).green().bold()), prompt);
    frame.render_widget(Paragraph::new(command_line).block(bottom_border()), command);
    frame.render_widget(
        Paragraph::new(format!("{filtered_count} active").dim()).block(bottom_border()),
        count,
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
                Line::from("active".green()),
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
    let title = match state.mode {
        TuiMode::Search => " Filtered Work ",
        _ => " Selected Work ",
    };
    let block = Block::default()
        .title(Line::from(vec![title.bold(), detail_hint(state).dim()]))
        .borders(Borders::ALL)
        .border_style(Style::new().dark_gray());

    let Some(work) = state.selected_work() else {
        frame.render_widget(
            Paragraph::new("No Work selected.\nPress n to create Work.".dim())
                .block(block)
                .wrap(Wrap { trim: true }),
            area,
        );
        return;
    };

    let lines = vec![
        Line::from(work.title.clone().bold()),
        Line::from(work.slug.clone().dim()),
        Line::from(""),
        Line::from("Goal".dim()),
        Line::from(work.goal.clone()),
        Line::from(""),
        Line::from("State".dim()),
        Line::from(vec!["intent        ".dim(), work.intent_id.clone().cyan()]),
        Line::from(vec![
            "folder        ".dim(),
            work.path.display().to_string().into(),
        ]),
        Line::from(vec![
            "switch result ".dim(),
            "return to shell and cd into this folder".into(),
        ]),
        Line::from(""),
        Line::from("Actions".dim()),
        Line::from(vec![
            "enter".green().bold(),
            " switch   ".dim(),
            "n".green().bold(),
            " create   ".dim(),
            "a".green().bold(),
            " archive   ".dim(),
            "/".green().bold(),
            " filter".dim(),
        ]),
        Line::from(""),
        Line::from("Trail".dim()),
        Line::from(vec![
            "created      ".dim(),
            "goal and intent written".into(),
        ]),
        Line::from(vec![
            "files        ".dim(),
            "AGENTS.md and CLAUDE.md in the Work folder".into(),
        ]),
        Line::from(vec![
            "next         ".dim(),
            "switch into the Work folder".into(),
        ]),
    ];

    frame.render_widget(
        Paragraph::new(lines).block(block).wrap(Wrap { trim: true }),
        area,
    );
}

fn render_footer(frame: &mut Frame<'_>, area: Rect) {
    let line = Line::from(vec![
        key("enter"),
        " switch ".dim(),
        key("n"),
        " create ".dim(),
        key("a"),
        " archive ".dim(),
        key("/"),
        " filter ".dim(),
        key(":"),
        " command ".dim(),
        key("?"),
        " keys ".dim(),
        key("q"),
        " quit".dim(),
    ]);
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
    let popup = top_popup(70, 5, area);
    let lines = vec![
        Line::from(vec![
            "/ ".green().bold(),
            if state.filter.is_empty() {
                " ".into()
            } else {
                state.filter.clone().bold()
            },
        ]),
        Line::from("filter active Work by title, slug, intent, or goal".dim()),
    ];
    render_popup(frame, popup, " Filter Work ", lines, Style::new().green());
}

fn render_command(frame: &mut Frame<'_>, area: Rect, state: &TuiState) {
    let popup = top_popup(70, 5, area);
    let lines = vec![
        Line::from(vec![
            ": ".green().bold(),
            if state.command.is_empty() {
                " ".into()
            } else {
                state.command.clone().bold()
            },
        ]),
        Line::from("try list, create, switch, archive".dim()),
    ];
    render_popup(frame, popup, " Command ", lines, Style::new().green());
}

fn render_create(frame: &mut Frame<'_>, area: Rect, state: &TuiState) {
    let popup = centered_rect(76, 52, area);
    let title = title_from_goal(&state.create_goal);
    let slug = slugify(&title);
    let lines = vec![
        Line::from("Goal".dim()),
        Line::from(if state.create_goal.is_empty() {
            "Answer why billing retry alerts spiked after the queue rollout.".dim()
        } else {
            state.create_goal.clone().into()
        }),
        Line::from(""),
        Line::from("Intent".dim()),
        selected_intent_line(state),
        Line::from(""),
        Line::from("Slug preview".dim()),
        Line::from(slug),
        Line::from(""),
        Line::from(vec![
            key("enter"),
            " create ".dim(),
            key("tab"),
            " intent ".dim(),
            key("esc"),
            " cancel".dim(),
        ]),
    ];
    render_popup(frame, popup, " Create Work ", lines, Style::new().green());
}

fn render_archive(frame: &mut Frame<'_>, area: Rect, state: &TuiState) {
    let popup = centered_rect(70, 36, area);
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
        Line::from(vec![
            "effect ".dim(),
            "Hidden from the active Work list.".red(),
        ]),
        Line::from(""),
        Line::from(vec![
            key("y"),
            " archive ".dim(),
            key("esc"),
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
    render_popup(frame, popup, " Status ", lines, style);
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
    format!(" {value} ").bold().green()
}

fn help_line(key_value: &'static str, label: &'static str) -> Line<'static> {
    Line::from(vec![key(key_value), " ".into(), label.dim()])
}

fn selected_intent_line(state: &TuiState) -> Line<'static> {
    if state.intents.is_empty() {
        return Line::from("investigate".cyan());
    }

    let spans = state
        .intents
        .iter()
        .enumerate()
        .flat_map(|(index, (intent, _))| {
            let span = if index == state.create_intent {
                intent.clone().cyan().bold()
            } else {
                intent.clone().dim()
            };
            [span, "  ".into()]
        })
        .collect::<Vec<_>>();
    Line::from(spans)
}

fn detail_hint(state: &TuiState) -> Span<'static> {
    if state.selected_work().is_some() {
        " enter switches ".dim()
    } else {
        " n creates ".dim()
    }
}

fn prompt_label(state: &TuiState) -> &'static str {
    match state.mode {
        TuiMode::Search => "/",
        TuiMode::Command => ":",
        _ => "wo",
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

fn top_popup(percent_x: u16, height: u16, area: Rect) -> Rect {
    let width = area.width.saturating_mul(percent_x).saturating_div(100);
    let width = width.clamp(30, area.width);
    Rect::new(area.x + (area.width - width) / 2, area.y + 2, width, height)
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
    use crate::tui::state::TuiState;

    #[test]
    fn renders_work_list_and_selected_detail() {
        let mut terminal =
            Terminal::new(TestBackend::new(120, 36)).expect("test terminal should initialize");
        let state = TuiState::new(work_list());

        terminal
            .draw(|frame| render(frame, &state))
            .expect("render should succeed");

        let content = format!("{}", terminal.backend());
        assert!(content.contains("wo"));
        assert!(content.contains("work list"));
        assert!(content.contains("Active Work"));
        assert!(content.contains("Selected Work"));
        assert!(content.contains("Billing retry audit"));
        assert!(content.contains("Goal"));
        assert!(content.contains("Actions"));
        assert!(content.contains("enter switches"));
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
}
