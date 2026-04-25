use crossterm::event::{KeyEvent, KeyModifiers};

pub(super) fn is_plain_character(key: KeyEvent) -> bool {
    !key.modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
}

pub(super) fn is_shortcut_character(key: KeyEvent) -> bool {
    key.modifiers == KeyModifiers::NONE
}
