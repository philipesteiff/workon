use ratatui::prelude::{Frame, Line, Span};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use super::theme;

pub(super) fn panel_block(title: &'static str, focused: bool) -> Block<'static> {
    Block::default()
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(title, theme::style_panel_title()),
            Span::raw(" "),
        ]))
        .borders(Borders::ALL)
        .border_style(if focused {
            theme::style_focused_border()
        } else {
            theme::style_inactive_border()
        })
}

pub(super) fn bottom_border() -> Block<'static> {
    Block::default()
        .borders(Borders::BOTTOM)
        .border_style(theme::style_inactive_border())
}

pub(super) fn top_border() -> Block<'static> {
    Block::default()
        .borders(Borders::TOP)
        .border_style(theme::style_inactive_border())
}

pub(super) fn status_badge(label: impl Into<String>, style: Style) -> Span<'static> {
    Span::styled(
        format!(" {} ", label.into()),
        style.add_modifier(Modifier::BOLD),
    )
}

pub(super) fn key(value: &'static str) -> Span<'static> {
    Span::styled(format!(" {value} "), theme::style_key())
}

pub(super) fn label_value(label: &'static str, value: impl Into<String>) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<15}"), theme::style_muted_text()),
        Span::styled(value.into(), theme::style_primary_text()),
    ])
}

pub(super) fn section(label: &'static str) -> Line<'static> {
    Line::from(vec![
        Span::styled(":: ", theme::style_muted_text()),
        Span::styled(label, theme::style_panel_title()),
    ])
}

pub(super) fn render_popup(
    frame: &mut Frame<'_>,
    area: ratatui::layout::Rect,
    title: &'static str,
    lines: Vec<Line<'static>>,
    border_style: Style,
) {
    frame.render_widget(Clear, area);
    let block = Block::default()
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(title, theme::style_panel_title()),
            Span::raw(" "),
        ]))
        .borders(Borders::ALL)
        .border_style(border_style);
    frame.render_widget(
        Paragraph::new(lines).block(block).wrap(Wrap { trim: true }),
        area,
    );
}
