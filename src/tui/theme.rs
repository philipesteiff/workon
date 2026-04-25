use ratatui::style::{Color, Modifier, Style};

const AMBER: Color = Color::Rgb(255, 176, 64);
const ORANGE: Color = Color::Rgb(255, 140, 32);
const TARGET_BG: Color = Color::Rgb(92, 58, 32);
const READY_AMBER: Color = Color::Rgb(190, 130, 70);
const DIM_AMBER: Color = Color::Rgb(160, 104, 48);
const BORDER_AMBER: Color = Color::Rgb(104, 72, 40);
const OK_AMBER: Color = Color::Rgb(220, 180, 84);

pub(super) fn style_primary_text() -> Style {
    Style::new().fg(AMBER)
}

pub(super) fn style_task_title() -> Style {
    Style::new().fg(AMBER).add_modifier(Modifier::BOLD)
}

pub(super) fn style_muted_text() -> Style {
    Style::new().fg(DIM_AMBER)
}

pub(super) fn style_meta_key() -> Style {
    Style::new().fg(BORDER_AMBER)
}

pub(super) fn style_meta_value() -> Style {
    Style::new().fg(ORANGE).add_modifier(Modifier::BOLD)
}

pub(super) fn style_panel_title() -> Style {
    Style::new().fg(AMBER).add_modifier(Modifier::BOLD)
}

pub(super) fn style_inactive_border() -> Style {
    Style::new().fg(BORDER_AMBER)
}

pub(super) fn style_focused_border() -> Style {
    Style::new().fg(ORANGE)
}

pub(super) fn style_status_ok() -> Style {
    Style::new().fg(OK_AMBER).add_modifier(Modifier::BOLD)
}

pub(super) fn style_status_warn() -> Style {
    Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD)
}

pub(super) fn style_status_error() -> Style {
    Style::new().fg(Color::Red).add_modifier(Modifier::BOLD)
}

pub(super) fn style_status_run() -> Style {
    Style::new().fg(ORANGE).add_modifier(Modifier::BOLD)
}

pub(super) fn style_command() -> Style {
    Style::new().fg(ORANGE).add_modifier(Modifier::BOLD)
}

pub(super) fn style_key() -> Style {
    Style::new().fg(ORANGE).add_modifier(Modifier::BOLD)
}

pub(super) fn style_selected() -> Style {
    Style::new().fg(ORANGE).add_modifier(Modifier::BOLD)
}

pub(super) fn style_target_row() -> Style {
    Style::new()
        .fg(AMBER)
        .bg(TARGET_BG)
        .add_modifier(Modifier::BOLD)
}

pub(super) fn style_current_text() -> Style {
    Style::new()
        .fg(OK_AMBER)
        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
}

pub(super) fn style_current_badge() -> Style {
    Style::new()
        .fg(Color::Black)
        .bg(OK_AMBER)
        .add_modifier(Modifier::BOLD)
}

pub(super) fn style_current_meta_label() -> Style {
    Style::new()
        .fg(DIM_AMBER)
        .add_modifier(Modifier::UNDERLINED)
}

pub(super) fn style_current_intent_value() -> Style {
    Style::new().fg(ORANGE).add_modifier(Modifier::BOLD)
}

pub(super) fn style_ready_badge() -> Style {
    Style::new().fg(READY_AMBER).add_modifier(Modifier::BOLD)
}

pub(super) fn style_active() -> Style {
    Style::new().fg(OK_AMBER).add_modifier(Modifier::BOLD)
}

pub(super) fn style_destructive() -> Style {
    Style::new().fg(Color::Red).add_modifier(Modifier::BOLD)
}

#[cfg(test)]
mod tests {
    use ratatui::style::Color;

    use super::{style_command, style_focused_border, style_inactive_border, style_primary_text};

    #[test]
    fn uses_amber_orange_terminal_palette() {
        assert_eq!(style_primary_text().fg, Some(Color::Rgb(255, 176, 64)));
        assert_eq!(style_focused_border().fg, Some(Color::Rgb(255, 140, 32)));
        assert_eq!(style_command().fg, Some(Color::Rgb(255, 140, 32)));
        assert_eq!(style_inactive_border().fg, Some(Color::Rgb(104, 72, 40)));
    }
}
