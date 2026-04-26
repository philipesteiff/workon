use ratatui::prelude::{Frame, Line, Span};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use super::theme;

pub(super) fn panel_block(title: &'static str, focused: bool) -> Block<'static> {
    panel_block_with_activity(title, focused, None)
}

pub(super) fn panel_block_with_activity(
    title: &'static str,
    focused: bool,
    activity: Option<TitleActivity>,
) -> Block<'static> {
    let mut spans = vec![
        Span::raw(" "),
        Span::styled(title, theme::style_panel_title()),
    ];
    if let Some(activity) = activity {
        spans.extend([
            Span::raw(" "),
            Span::styled(activity.frame_label(), theme::style_command()),
            Span::raw(" "),
            Span::styled(activity.label, theme::style_command()),
        ]);
    }
    spans.push(Span::raw(" "));
    panel_block_with_title(Line::from(spans), focused)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TitleActivity {
    label: String,
    frame: usize,
}

impl TitleActivity {
    pub(super) fn loading(label: impl Into<String>, frame: usize) -> Self {
        Self {
            label: label.into(),
            frame,
        }
    }

    fn frame_label(&self) -> &'static str {
        const FRAMES: [&str; 8] = [
            "⣾▉▊▋▌▍▎▏▎▍▌▋▊▉⣾ ",
            "⣽▊▋▌▍▎▏▎▍▌▋▊▉▉⣽ ",
            "⣻▋▌▍▎▏▎▍▌▋▊▉▉▊⣻ ",
            "⢿▌▍▎▏▎▍▌▋▊▉▉▊▋⢿ ",
            "⡿▍▎▏▎▍▌▋▊▉▉▊▋▌⡿ ",
            "⣟▎▏▎▍▌▋▊▉▉▊▋▌▍⣟ ",
            "⣯▏▎▍▌▋▊▉▉▊▋▌▍▎⣯ ",
            "⣷▎▍▌▋▊▉▉▊▋▌▍▎▏⣷ ",
        ];
        FRAMES[self.frame % FRAMES.len()]
    }
}

pub(super) fn panel_block_with_title(title: Line<'static>, focused: bool) -> Block<'static> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(if focused {
            theme::style_focused_border()
        } else {
            theme::style_inactive_border()
        })
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

#[cfg(test)]
mod tests {
    use super::TitleActivity;

    #[test]
    fn loading_activity_uses_braille_capped_block_wave_frames() {
        let expected = ["⣾▉▊▋▌▍▎▏▎▍▌▋▊▉⣾ ", "⣽▊▋▌▍▎▏▎▍▌▋▊▉▉⣽ ", "⣷▎▍▌▋▊▉▉▊▋▌▍▎▏⣷ "];

        let actual = [0, 1, 7]
            .map(|frame| TitleActivity::loading("loading repositories", frame).frame_label());

        assert_eq!(actual, expected);
    }

    #[test]
    fn loading_activity_frames_do_not_use_letters_or_numbers() {
        for frame in 0..32 {
            let label = TitleActivity::loading("loading repositories", frame).frame_label();

            assert!(
                label.chars().all(|character| !character.is_alphanumeric()),
                "{label} should be symbol-only"
            );
        }
    }

    #[test]
    fn loading_activity_frames_keep_title_width_stable() {
        let first_width = TitleActivity::loading("loading repositories", 0)
            .frame_label()
            .chars()
            .count();

        for frame in 1..32 {
            assert_eq!(
                TitleActivity::loading("loading repositories", frame)
                    .frame_label()
                    .chars()
                    .count(),
                first_width
            );
        }
    }
}
