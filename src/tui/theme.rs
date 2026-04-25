use ratatui::style::{Color, Modifier, Style};

pub(super) fn style_primary_text() -> Style {
    Style::new().fg(Color::White)
}

pub(super) fn style_muted_text() -> Style {
    Style::new().fg(Color::DarkGray)
}

pub(super) fn style_panel_title() -> Style {
    Style::new().fg(Color::White).add_modifier(Modifier::BOLD)
}

pub(super) fn style_inactive_border() -> Style {
    Style::new().fg(Color::DarkGray)
}

pub(super) fn style_focused_border() -> Style {
    Style::new().fg(Color::Cyan)
}

pub(super) fn style_status_ok() -> Style {
    Style::new().fg(Color::Green).add_modifier(Modifier::BOLD)
}

pub(super) fn style_status_warn() -> Style {
    Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD)
}

pub(super) fn style_status_error() -> Style {
    Style::new().fg(Color::Red).add_modifier(Modifier::BOLD)
}

pub(super) fn style_status_run() -> Style {
    Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD)
}

pub(super) fn style_command() -> Style {
    Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD)
}

pub(super) fn style_key() -> Style {
    Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD)
}

pub(super) fn style_selected() -> Style {
    Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD)
}

pub(super) fn style_active() -> Style {
    Style::new().fg(Color::Green).add_modifier(Modifier::BOLD)
}

pub(super) fn style_destructive() -> Style {
    Style::new().fg(Color::Red).add_modifier(Modifier::BOLD)
}
