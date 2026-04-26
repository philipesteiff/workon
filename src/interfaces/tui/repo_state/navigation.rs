use super::{RepoPane, RepoPickerState};

impl RepoPickerState {
    pub(in crate::interfaces::tui::repo_state) fn next_focus(&mut self) {
        self.shift_focus(1);
    }

    pub(in crate::interfaces::tui::repo_state) fn previous_focus(&mut self) {
        self.shift_focus(-1);
    }

    pub(in crate::interfaces::tui::repo_state) fn move_selection(&mut self, delta: isize) {
        let count = self.current_count();
        if count == 0 {
            self.set_current_selection(0);
            return;
        }

        let count = count as isize;
        let selected = (self.current_selection() as isize + delta).rem_euclid(count) as usize;
        self.set_current_selection(selected);
    }

    pub(in crate::interfaces::tui::repo_state) fn clamp_selection(&mut self) {
        let catalog_count = self.catalog_rows().len();
        if catalog_count == 0 {
            self.selected_catalog = 0;
        } else if self.selected_catalog >= catalog_count {
            self.selected_catalog = catalog_count - 1;
        }

        let workspace_count = self.workspaces.len();
        if workspace_count == 0 {
            self.selected_workspace = 0;
        } else if self.selected_workspace >= workspace_count {
            self.selected_workspace = workspace_count - 1;
        }
    }

    fn shift_focus(&mut self, delta: isize) {
        let order = focus_order();
        let current = order
            .iter()
            .position(|pane| *pane == self.focus)
            .unwrap_or_default() as isize;
        let next = (current + delta).rem_euclid(order.len() as isize) as usize;
        self.focus = order[next];
    }

    fn current_selection(&self) -> usize {
        match self.focus {
            RepoPane::Catalog => self.selected_catalog,
            RepoPane::AddPath => 0,
            RepoPane::ConfiguredPaths => self.selected_workspace,
        }
    }

    fn set_current_selection(&mut self, selected: usize) {
        match self.focus {
            RepoPane::Catalog => self.selected_catalog = selected,
            RepoPane::AddPath => {}
            RepoPane::ConfiguredPaths => self.selected_workspace = selected,
        }
    }

    fn current_count(&self) -> usize {
        match self.focus {
            RepoPane::Catalog => self.catalog_rows().len(),
            RepoPane::AddPath => 0,
            RepoPane::ConfiguredPaths => self.workspaces.len(),
        }
    }
}

fn focus_order() -> [RepoPane; 3] {
    [
        RepoPane::Catalog,
        RepoPane::AddPath,
        RepoPane::ConfiguredPaths,
    ]
}
