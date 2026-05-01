use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::repo_state::{RepoCatalogRow, RepoPane, RepoStatus};
use super::state::{Toast, TraceKind, TuiAction, TuiMode, TuiState};
use crate::domain::{
    AttachedRepository, AvailableRepository, RepositoryCandidate, RepositoryWorkspace, WorkList,
    WorkSummary,
};

#[test]
fn filters_work_by_title_slug_intent_and_goal() {
    let mut state = TuiState::new(work_list());

    state.push_filter_char('r');
    state.push_filter_char('e');
    state.push_filter_char('v');

    let filtered = state.filtered_works();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].slug, "review-cache-invalidation-pr");
}

#[test]
fn selection_wraps_through_filtered_work() {
    let mut state = TuiState::new(work_list());

    state.move_selection(-1);
    assert_eq!(
        state.selected_work().map(|work| work.slug.as_str()),
        Some("context-command-sketch")
    );

    state.move_selection(1);
    assert_eq!(
        state.selected_work().map(|work| work.slug.as_str()),
        Some("billing-retry-audit")
    );
}

#[test]
fn selects_current_work_when_cwd_is_inside_work_folder() {
    let state = TuiState::new(work_list()).with_current_directory(Some(std::path::Path::new(
        "/tmp/workon/.workon/work/review-cache-invalidation-pr/src",
    )));

    assert_eq!(
        state.selected_work().map(|work| work.slug.as_str()),
        Some("review-cache-invalidation-pr")
    );
    let selected = state
        .selected_work()
        .expect("current work should be selected");
    assert!(state.is_current_work(selected));
    assert_eq!(
        state.current_work_path.as_deref(),
        Some(std::path::Path::new(
            "/tmp/workon/.workon/work/review-cache-invalidation-pr"
        ))
    );
    assert_eq!(state.filtered_count(), 3);
    assert_eq!(state.total_count(), 3);
}

#[test]
fn leaves_all_work_active_when_cwd_is_outside_work_folders() {
    let state = TuiState::new(work_list())
        .with_current_directory(Some(std::path::Path::new("/tmp/workon/elsewhere")));

    assert_eq!(
        state.selected_work().map(|work| work.slug.as_str()),
        Some("billing-retry-audit")
    );
    assert!(state.current_work_path.is_none());
    assert!(state.works.iter().all(|work| !state.is_current_work(work)));
}

#[test]
fn trace_records_newest_operational_events_first_and_caps_history() {
    let mut state = TuiState::new(work_list());

    for index in 0..10 {
        state.push_trace(TraceKind::Run, format!("event {index}"));
    }

    assert_eq!(state.trace.len(), 6);
    assert_eq!(state.trace[0].message, "event 9");
    assert_eq!(state.trace[0].kind, TraceKind::Run);
    assert_eq!(state.trace[5].message, "event 4");
}

#[test]
fn trace_panel_is_hidden_by_default_and_toggles_from_list() {
    let mut state = TuiState::new(work_list());

    assert!(!state.trace_visible);

    let action = state.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL));

    assert_eq!(action, TuiAction::None);
    assert!(state.trace_visible);

    let action = state.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL));

    assert_eq!(action, TuiAction::None);
    assert!(!state.trace_visible);
}

#[test]
fn clears_toast_on_next_meaningful_key_press() {
    let mut state = TuiState::new(work_list());
    state.toast = Some(Toast::info(
        "Work created",
        "/tmp/workon/.workon/work/billing",
    ));

    let action = state.handle_key(key(KeyCode::Down));

    assert_eq!(action, TuiAction::None);
    assert_eq!(state.toast, None);
    assert_eq!(
        state.selected_work().map(|work| work.slug.as_str()),
        Some("review-cache-invalidation-pr")
    );
}

#[test]
fn colon_starts_filter_like_other_printable_characters() {
    let mut state = TuiState::new(work_list());

    let action = state.handle_key(key(KeyCode::Char(':')));

    assert_eq!(action, TuiAction::None);
    assert_eq!(state.mode, TuiMode::Search);
    assert_eq!(state.filter, ":");
    assert_eq!(state.trace[0].message, "filter :");
}

#[test]
fn direct_typing_starts_filtering_from_list_mode() {
    let mut state = TuiState::new(work_list());
    state.move_selection(1);

    let mut action = TuiAction::None;
    for character in "billing".chars() {
        action = state.handle_key(key(KeyCode::Char(character)));
    }

    assert_eq!(action, TuiAction::None);
    assert_eq!(state.mode, TuiMode::Search);
    assert_eq!(state.filter, "billing");
    assert_eq!(state.filtered_count(), 1);
    assert_eq!(
        state.selected_work().map(|work| work.slug.as_str()),
        Some("billing-retry-audit")
    );
}

#[test]
fn bare_printable_shortcuts_append_to_filter() {
    for character in ['j', 'k', 'n', 'a', 'q', '?', ' '] {
        let mut state = TuiState::new(work_list());

        let action = state.handle_key(key(KeyCode::Char(character)));

        assert_eq!(action, TuiAction::None);
        assert_eq!(state.mode, TuiMode::Search);
        assert_eq!(state.filter, character.to_string());
    }
}

#[test]
fn slash_enters_leader_without_changing_filter_or_selection() {
    let mut state = TuiState::new(work_list());
    state.move_selection(1);
    let selected_before = state.selected_work().map(|work| work.slug.clone());

    let action = state.handle_key(key(KeyCode::Char('/')));

    assert_eq!(action, TuiAction::None);
    assert_eq!(state.mode, TuiMode::Leader);
    assert_eq!(state.filter, "");
    assert_eq!(state.filtered_count(), 3);
    assert_eq!(
        state.selected_work().map(|work| work.slug.clone()),
        selected_before
    );
}

#[test]
fn enter_in_filter_switches_highlighted_match() {
    let mut state = TuiState::new(work_list());
    for character in "context".chars() {
        state.handle_key(key(KeyCode::Char(character)));
    }

    let action = state.handle_key(key(KeyCode::Enter));

    assert_eq!(
        action,
        TuiAction::Switch("context-command-sketch".to_string())
    );
}

#[test]
fn search_navigation_switches_directly_from_filtered_queue() {
    let mut state = TuiState::new(work_list());
    state.handle_key(key(KeyCode::Char('i')));
    state.handle_key(key(KeyCode::Char('n')));
    state.handle_key(key(KeyCode::Char('v')));
    state.handle_key(key(KeyCode::Down));

    let action = state.handle_key(key(KeyCode::Enter));

    assert_eq!(
        action,
        TuiAction::Switch("review-cache-invalidation-pr".to_string())
    );
}

#[test]
fn slash_leader_commands_route_actions() {
    let mut state = TuiState::new(work_list());
    state.handle_key(key(KeyCode::Char('/')));
    assert_eq!(state.handle_key(key(KeyCode::Char('n'))), TuiAction::None);
    assert_eq!(state.mode, TuiMode::Create);

    let mut archive_state = TuiState::new(work_list());
    for character in "review".chars() {
        archive_state.handle_key(key(KeyCode::Char(character)));
    }
    archive_state.handle_key(key(KeyCode::Char('/')));
    assert_eq!(
        archive_state.handle_key(key(KeyCode::Char('a'))),
        TuiAction::None
    );
    assert_eq!(archive_state.mode, TuiMode::Archive);
    assert_eq!(
        archive_state.handle_key(key(KeyCode::Enter)),
        TuiAction::Archive("review-cache-invalidation-pr".to_string())
    );

    let mut help_state = TuiState::new(work_list());
    help_state.handle_key(key(KeyCode::Char('/')));
    assert_eq!(
        help_state.handle_key(key(KeyCode::Char('?'))),
        TuiAction::None
    );
    assert_eq!(help_state.mode, TuiMode::Help);

    let mut quit_state = TuiState::new(work_list());
    quit_state.handle_key(key(KeyCode::Char('/')));
    assert_eq!(
        quit_state.handle_key(key(KeyCode::Char('q'))),
        TuiAction::Quit
    );
}

#[test]
fn slash_leader_opens_repo_context_for_highlighted_work() {
    let mut state = TuiState::new(work_list());
    state.move_selection(1);

    state.handle_key(key(KeyCode::Char('/')));
    let action = state.handle_key(key(KeyCode::Char('r')));

    assert_eq!(
        action,
        TuiAction::OpenRepos("review-cache-invalidation-pr".to_string())
    );
    assert_eq!(state.mode, TuiMode::Repos);
    assert_eq!(state.repo.focus, RepoPane::Catalog);
    assert_eq!(
        state.repo.status,
        RepoStatus::Loading {
            message: "Loading: attached, workspaces, local, GitHub".to_string()
        }
    );
}

#[test]
fn slash_i_opens_intent_context_for_highlighted_work() {
    let mut state = TuiState::new(work_list()).with_intents(intent_list());
    state.move_selection(1);

    state.handle_key(key(KeyCode::Char('/')));
    let action = state.handle_key(key(KeyCode::Char('i')));

    assert_eq!(action, TuiAction::None);
    assert_eq!(state.mode, TuiMode::Intents);
    assert_eq!(state.intent_work_slug, "review-cache-invalidation-pr");
    assert_eq!(state.intent_work_title, "Review cache invalidation PR");
    assert_eq!(state.intent_current_id, "review-pr");
    assert_eq!(state.selected_intent_id().as_deref(), Some("review-pr"));
}

#[test]
fn intent_context_filters_and_switches_highlighted_intent() {
    let mut state = TuiState::new(work_list()).with_intents(intent_list());
    state.move_selection(1);
    state.handle_key(key(KeyCode::Char('/')));
    state.handle_key(key(KeyCode::Char('i')));

    for character in "brain".chars() {
        state.handle_key(key(KeyCode::Char(character)));
    }

    assert_eq!(state.intent_filter, "brain");
    assert_eq!(state.selected_intent_id().as_deref(), Some("brainstorm"));
    assert_eq!(
        state.handle_key(key(KeyCode::Enter)),
        TuiAction::SwitchIntent {
            work_slug: "review-cache-invalidation-pr".to_string(),
            intent_id: "brainstorm".to_string(),
        }
    );
}

#[test]
fn repo_context_accepts_text_input_while_loading() {
    let mut state = TuiState::new(work_list());
    state.enter_repo_context(
        "billing-retry-audit".to_string(),
        "Billing retry audit".to_string(),
        available_repositories(),
        Vec::new(),
        attached_repositories(),
        repo_workspaces(),
    );
    state.start_repo_loading("Loading repository sources");

    assert_eq!(state.handle_key(key(KeyCode::Char('w'))), TuiAction::None);

    assert_eq!(state.mode, TuiMode::Repos);
    assert_eq!(state.repo.filter, "w");
}

#[test]
fn repo_context_accepts_navigation_while_loading() {
    let mut state = TuiState::new(work_list());
    state.enter_repo_context(
        "billing-retry-audit".to_string(),
        "Billing retry audit".to_string(),
        available_repositories(),
        Vec::new(),
        attached_repositories(),
        repo_workspaces(),
    );
    state.start_repo_loading("Loading repository sources");

    assert_eq!(state.handle_key(key(KeyCode::Down)), TuiAction::None);

    assert_eq!(state.mode, TuiMode::Repos);
    assert_eq!(state.repo.selected_catalog, 1);
}

#[test]
fn repo_context_keeps_github_catalog_visible_with_attached_repositories() {
    let mut state = TuiState::new(work_list());
    state.enter_repo_context(
        "billing-retry-audit".to_string(),
        "Billing retry audit".to_string(),
        available_repositories(),
        Vec::new(),
        attached_repositories(),
        repo_workspaces(),
    );

    assert_eq!(state.mode, TuiMode::Repos);
    let names = state
        .repo
        .catalog_rows()
        .into_iter()
        .map(|repository| repository.name().to_string())
        .collect::<Vec<_>>();

    assert_eq!(
        names,
        vec![
            "openai/api".to_string(),
            "openai/workon".to_string(),
            "openai/api-docs".to_string(),
        ]
    );
}

#[test]
fn repo_context_prefers_local_catalog_row_over_github_duplicate() {
    let mut state = TuiState::new(work_list());
    state.enter_repo_context(
        "billing-retry-audit".to_string(),
        "Billing retry audit".to_string(),
        available_repositories(),
        duplicate_local_candidate(),
        attached_repositories(),
        repo_workspaces(),
    );
    state.repo.filter = "api-docs".to_string();

    let rows = state.repo.catalog_rows();

    assert_eq!(rows.len(), 1);
    assert!(matches!(rows[0], RepoCatalogRow::Local(_)));
    assert_eq!(rows[0].name(), "openai/api-docs");
}

#[test]
fn repo_context_updates_work_detail_repository_index() {
    let mut state = TuiState::new(work_list());
    state.enter_repo_context(
        "billing-retry-audit".to_string(),
        "Billing retry audit".to_string(),
        available_repositories(),
        Vec::new(),
        attached_repositories(),
        repo_workspaces(),
    );

    let selected = state
        .selected_work()
        .expect("billing work should be selected");
    assert_eq!(state.attached_repositories_for(selected).len(), 2);

    let refreshed = vec![attached_repositories()[1].clone()];
    state.update_attached_repositories(refreshed);

    let selected = state
        .selected_work()
        .expect("billing work should still be selected");
    let repositories = state.attached_repositories_for(selected);
    assert_eq!(repositories.len(), 1);
    assert_eq!(repositories[0].name_with_owner, "openai/workon");
}

#[test]
fn repo_context_applies_pending_adds_and_removes_from_repository_catalog() {
    let mut state = TuiState::new(work_list());
    state.enter_repo_context(
        "billing-retry-audit".to_string(),
        "Billing retry audit".to_string(),
        available_repositories(),
        Vec::new(),
        attached_repositories(),
        repo_workspaces(),
    );

    assert_eq!(state.repo.focus, RepoPane::Catalog);
    state.handle_key(key(KeyCode::Char(' ')));
    state.handle_key(key(KeyCode::Down));
    state.handle_key(key(KeyCode::Down));
    state.handle_key(key(KeyCode::Char(' ')));

    assert_eq!(
        state.handle_key(key(KeyCode::Enter)),
        TuiAction::ApplyRepoChanges {
            work_slug: "billing-retry-audit".to_string(),
            add: vec!["openai/api-docs".to_string()],
            link: Vec::new(),
            remove: vec!["openai/api".to_string()],
            force_remove: false,
            workspace: Some(PathBuf::from("/tmp/repos")),
        }
    );
}

#[test]
fn repo_context_can_arm_force_remove_for_pending_removals() {
    let mut state = TuiState::new(work_list());
    state.enter_repo_context(
        "billing-retry-audit".to_string(),
        "Billing retry audit".to_string(),
        available_repositories(),
        Vec::new(),
        attached_repositories(),
        repo_workspaces(),
    );
    state.handle_key(key(KeyCode::Char(' ')));
    state.handle_key(key(KeyCode::Char('!')));

    assert!(state.repo.force_remove);
    assert_eq!(
        state.handle_key(key(KeyCode::Enter)),
        TuiAction::ApplyRepoChanges {
            work_slug: "billing-retry-audit".to_string(),
            add: Vec::new(),
            link: Vec::new(),
            remove: vec!["openai/api".to_string()],
            force_remove: true,
            workspace: None,
        }
    );
}

#[test]
fn repo_context_focuses_add_path_when_setup_is_required() {
    let mut state = TuiState::new(work_list());
    state.enter_repo_context(
        "billing-retry-audit".to_string(),
        "Billing retry audit".to_string(),
        available_repositories(),
        Vec::new(),
        attached_repositories(),
        Vec::new(),
    );

    assert_eq!(state.repo.focus, RepoPane::AddPath);

    for character in "/tmp/repo-space".chars() {
        state.handle_key(key(KeyCode::Char(character)));
    }

    assert_eq!(
        state.handle_key(key(KeyCode::Enter)),
        TuiAction::AddRepoWorkspaces {
            work_slug: "billing-retry-audit".to_string(),
            paths: vec![PathBuf::from("/tmp/repo-space")],
        }
    );
}

#[test]
fn repo_context_setup_accepts_multiple_workspace_paths() {
    let mut state = TuiState::new(work_list());
    state.enter_repo_context(
        "billing-retry-audit".to_string(),
        "Billing retry audit".to_string(),
        available_repositories(),
        Vec::new(),
        attached_repositories(),
        Vec::new(),
    );

    assert_eq!(state.repo.focus, RepoPane::AddPath);

    for character in "/tmp/repo-a, /tmp/repo-b".chars() {
        state.handle_key(key(KeyCode::Char(character)));
    }

    assert_eq!(
        state.handle_key(key(KeyCode::Enter)),
        TuiAction::AddRepoWorkspaces {
            work_slug: "billing-retry-audit".to_string(),
            paths: vec![PathBuf::from("/tmp/repo-a"), PathBuf::from("/tmp/repo-b")],
        }
    );
}

#[test]
fn repo_context_uses_workspace_after_required_setup_completes() {
    let mut state = TuiState::new(work_list());
    state.enter_repo_context(
        "billing-retry-audit".to_string(),
        "Billing retry audit".to_string(),
        available_repositories(),
        Vec::new(),
        attached_repositories(),
        Vec::new(),
    );
    state.update_repo_workspaces_from_load(repo_workspaces());
    state.handle_key(key(KeyCode::Down));
    state.handle_key(key(KeyCode::Down));
    state.handle_key(key(KeyCode::Char(' ')));

    assert_eq!(
        state.handle_key(key(KeyCode::Enter)),
        TuiAction::ApplyRepoChanges {
            work_slug: "billing-retry-audit".to_string(),
            add: vec!["openai/api-docs".to_string()],
            link: Vec::new(),
            remove: Vec::new(),
            force_remove: false,
            workspace: Some(PathBuf::from("/tmp/repos")),
        }
    );
}

#[test]
fn repo_context_treats_w_as_filter_text() {
    let mut state = TuiState::new(work_list());
    state.enter_repo_context(
        "billing-retry-audit".to_string(),
        "Billing retry audit".to_string(),
        available_repositories(),
        Vec::new(),
        attached_repositories(),
        repo_workspaces(),
    );

    assert_eq!(state.handle_key(key(KeyCode::Char('w'))), TuiAction::None);

    assert_eq!(state.repo.filter, "w");
    assert_eq!(
        state.repo.selected_workspace_path(),
        Some(PathBuf::from("/tmp/repos"))
    );
}

#[test]
fn repo_context_selects_creation_workspace_without_blocking_filter_text() {
    let mut state = TuiState::new(work_list());
    state.enter_repo_context(
        "billing-retry-audit".to_string(),
        "Billing retry audit".to_string(),
        available_repositories(),
        Vec::new(),
        attached_repositories(),
        multiple_repo_workspaces(),
    );

    assert_eq!(state.handle_key(key(KeyCode::Tab)), TuiAction::None);
    assert_eq!(state.repo.focus, RepoPane::AddPath);
    assert_eq!(state.handle_key(key(KeyCode::Tab)), TuiAction::None);
    assert_eq!(state.repo.focus, RepoPane::ConfiguredPaths);
    assert_eq!(state.handle_key(key(KeyCode::Down)), TuiAction::None);
    assert_eq!(
        state.repo.selected_workspace_path(),
        Some(PathBuf::from("/tmp/client-repos"))
    );

    assert_eq!(state.handle_key(key(KeyCode::Left)), TuiAction::None);
    assert_eq!(state.handle_key(key(KeyCode::Left)), TuiAction::None);
    assert_eq!(state.handle_key(key(KeyCode::Char('w'))), TuiAction::None);
    assert_eq!(state.repo.focus, RepoPane::Catalog);
    assert_eq!(state.repo.filter, "w");
}

#[test]
fn repo_context_can_add_more_workspaces_from_add_path_panel() {
    let mut state = TuiState::new(work_list());
    state.enter_repo_context(
        "billing-retry-audit".to_string(),
        "Billing retry audit".to_string(),
        available_repositories(),
        Vec::new(),
        attached_repositories(),
        repo_workspaces(),
    );

    assert_eq!(state.handle_key(key(KeyCode::Tab)), TuiAction::None);
    assert_eq!(state.repo.focus, RepoPane::AddPath);
    for character in "/tmp/more-repos".chars() {
        state.handle_key(key(KeyCode::Char(character)));
    }

    assert_eq!(
        state.handle_key(key(KeyCode::Enter)),
        TuiAction::AddRepoWorkspaces {
            work_slug: "billing-retry-audit".to_string(),
            paths: vec![PathBuf::from("/tmp/more-repos")],
        }
    );
}

#[test]
fn repo_context_shows_loader_after_add_path_submit() {
    let mut state = TuiState::new(work_list());
    state.enter_repo_context(
        "billing-retry-audit".to_string(),
        "Billing retry audit".to_string(),
        available_repositories(),
        Vec::new(),
        attached_repositories(),
        repo_workspaces(),
    );

    assert_eq!(state.handle_key(key(KeyCode::Tab)), TuiAction::None);
    assert_eq!(state.repo.focus, RepoPane::AddPath);
    for character in "/tmp/more-repos".chars() {
        state.handle_key(key(KeyCode::Char(character)));
    }

    assert_eq!(
        state.handle_key(key(KeyCode::Enter)),
        TuiAction::AddRepoWorkspaces {
            work_slug: "billing-retry-audit".to_string(),
            paths: vec![PathBuf::from("/tmp/more-repos")],
        }
    );
    assert_eq!(
        state.repo.status,
        RepoStatus::Loading {
            message: "Adding repository workspace".to_string(),
        }
    );
}

#[test]
fn repo_context_add_path_panel_keeps_repo_context_on_empty_input() {
    let mut state = TuiState::new(work_list());
    state.enter_repo_context(
        "billing-retry-audit".to_string(),
        "Billing retry audit".to_string(),
        available_repositories(),
        Vec::new(),
        attached_repositories(),
        repo_workspaces(),
    );

    state.handle_key(key(KeyCode::Tab));

    assert_eq!(state.handle_key(key(KeyCode::Enter)), TuiAction::None);
    assert_eq!(state.mode, TuiMode::Repos);
    assert_eq!(state.repo.focus, RepoPane::AddPath);
    assert_eq!(state.toast, None);
}

#[test]
fn repo_context_configured_paths_panel_keeps_repo_context_without_toast_when_empty() {
    let mut state = TuiState::new(work_list());
    state.enter_repo_context(
        "billing-retry-audit".to_string(),
        "Billing retry audit".to_string(),
        available_repositories(),
        Vec::new(),
        attached_repositories(),
        Vec::new(),
    );
    state.repo.focus = RepoPane::ConfiguredPaths;

    assert_eq!(state.handle_key(key(KeyCode::Enter)), TuiAction::None);
    assert_eq!(state.mode, TuiMode::Repos);
    assert_eq!(state.repo.focus, RepoPane::ConfiguredPaths);
    assert_eq!(state.toast, None);
}

#[test]
fn repo_context_configured_paths_ignores_text_so_selection_is_stable() {
    let mut state = TuiState::new(work_list());
    state.enter_repo_context(
        "billing-retry-audit".to_string(),
        "Billing retry audit".to_string(),
        available_repositories(),
        Vec::new(),
        attached_repositories(),
        multiple_repo_workspaces(),
    );

    state.handle_key(key(KeyCode::Tab));
    state.handle_key(key(KeyCode::Tab));
    state.handle_key(key(KeyCode::Char('/')));
    state.handle_key(key(KeyCode::Down));

    assert_eq!(state.repo.focus, RepoPane::ConfiguredPaths);
    assert_eq!(
        state.repo.selected_workspace_path(),
        Some(PathBuf::from("/tmp/client-repos"))
    );
    assert!(state.repo.workspace_input().is_empty());
    assert!(state.repo.filter.is_empty());
}

#[test]
fn repo_context_tab_cycles_between_repository_workspace_panels() {
    let mut state = TuiState::new(work_list());
    state.enter_repo_context(
        "billing-retry-audit".to_string(),
        "Billing retry audit".to_string(),
        available_repositories(),
        Vec::new(),
        attached_repositories(),
        multiple_repo_workspaces(),
    );

    assert_eq!(state.repo.focus, RepoPane::Catalog);
    state.handle_key(key(KeyCode::Tab));
    assert_eq!(state.repo.focus, RepoPane::AddPath);
    state.handle_key(key(KeyCode::Tab));
    assert_eq!(state.repo.focus, RepoPane::ConfiguredPaths);
    state.handle_key(key(KeyCode::Tab));
    assert_eq!(state.repo.focus, RepoPane::Catalog);
    state.handle_key(key(KeyCode::BackTab));
    assert_eq!(state.repo.focus, RepoPane::ConfiguredPaths);
}

#[test]
fn repo_context_plus_jumps_to_add_path_panel() {
    let mut state = TuiState::new(work_list());
    state.enter_repo_context(
        "billing-retry-audit".to_string(),
        "Billing retry audit".to_string(),
        available_repositories(),
        Vec::new(),
        attached_repositories(),
        repo_workspaces(),
    );

    assert_eq!(state.handle_key(key(KeyCode::Char('+'))), TuiAction::None);
    assert_eq!(state.mode, TuiMode::Repos);
    assert_eq!(state.repo.focus, RepoPane::AddPath);
}

#[test]
fn repo_context_configured_paths_removes_selected_path() {
    let mut state = TuiState::new(work_list());
    state.enter_repo_context(
        "billing-retry-audit".to_string(),
        "Billing retry audit".to_string(),
        available_repositories(),
        Vec::new(),
        attached_repositories(),
        multiple_repo_workspaces(),
    );

    state.handle_key(key(KeyCode::Tab));
    state.handle_key(key(KeyCode::Tab));
    state.handle_key(key(KeyCode::Down));

    assert_eq!(
        state.handle_key(key(KeyCode::Char(' '))),
        TuiAction::RemoveRepoWorkspace {
            work_slug: "billing-retry-audit".to_string(),
            path: PathBuf::from("/tmp/client-repos"),
        }
    );
}

#[test]
fn repo_context_enter_removes_configured_path() {
    let mut state = TuiState::new(work_list());
    state.enter_repo_context(
        "billing-retry-audit".to_string(),
        "Billing retry audit".to_string(),
        available_repositories(),
        Vec::new(),
        attached_repositories(),
        multiple_repo_workspaces(),
    );

    state.handle_key(key(KeyCode::Tab));
    state.handle_key(key(KeyCode::Tab));
    state.handle_key(key(KeyCode::Down));

    assert_eq!(
        state.handle_key(key(KeyCode::Enter)),
        TuiAction::RemoveRepoWorkspace {
            work_slug: "billing-retry-audit".to_string(),
            path: PathBuf::from("/tmp/client-repos"),
        }
    );
}

#[test]
fn repo_context_applies_github_adds_to_selected_workspace() {
    let mut state = TuiState::new(work_list());
    state.enter_repo_context(
        "billing-retry-audit".to_string(),
        "Billing retry audit".to_string(),
        available_repositories(),
        Vec::new(),
        attached_repositories(),
        multiple_repo_workspaces(),
    );

    state.handle_key(key(KeyCode::Tab));
    state.handle_key(key(KeyCode::Tab));
    state.handle_key(key(KeyCode::Down));
    state.handle_key(key(KeyCode::Left));
    state.handle_key(key(KeyCode::Left));
    state.repo.filter = "api-docs".to_string();
    state.handle_key(key(KeyCode::Char(' ')));

    assert_eq!(
        state.handle_key(key(KeyCode::Enter)),
        TuiAction::ApplyRepoChanges {
            work_slug: "billing-retry-audit".to_string(),
            add: vec!["openai/api-docs".to_string()],
            link: Vec::new(),
            remove: Vec::new(),
            force_remove: false,
            workspace: Some(PathBuf::from("/tmp/client-repos")),
        }
    );
}

#[test]
fn repo_context_keeps_github_and_local_selection_for_same_repo_exclusive() {
    let mut state = TuiState::new(work_list());
    state.enter_repo_context(
        "billing-retry-audit".to_string(),
        "Billing retry audit".to_string(),
        available_repositories(),
        duplicate_local_candidate(),
        attached_repositories(),
        repo_workspaces(),
    );
    state.repo.filter = "api-docs".to_string();

    state.handle_key(key(KeyCode::Char(' ')));
    state.handle_key(key(KeyCode::Down));
    state.handle_key(key(KeyCode::Char(' ')));

    assert_eq!(
        state.handle_key(key(KeyCode::Enter)),
        TuiAction::ApplyRepoChanges {
            work_slug: "billing-retry-audit".to_string(),
            add: Vec::new(),
            link: vec![PathBuf::from("/tmp/repos/api-docs")],
            remove: Vec::new(),
            force_remove: false,
            workspace: None,
        }
    );
}

#[test]
fn repo_context_links_selected_local_candidate() {
    let mut state = TuiState::new(work_list());
    state.enter_repo_context(
        "billing-retry-audit".to_string(),
        "Billing retry audit".to_string(),
        available_repositories(),
        local_candidates(),
        attached_repositories(),
        repo_workspaces(),
    );
    state.repo.filter = "local".to_string();

    state.handle_key(key(KeyCode::Char(' ')));

    assert_eq!(
        state.handle_key(key(KeyCode::Enter)),
        TuiAction::ApplyRepoChanges {
            work_slug: "billing-retry-audit".to_string(),
            add: Vec::new(),
            link: vec![PathBuf::from("/tmp/repos/local-tool")],
            remove: Vec::new(),
            force_remove: false,
            workspace: None,
        }
    );
}

#[test]
fn slash_leader_can_insert_literal_slash_and_cancel() {
    let mut state = TuiState::new(work_list());
    state.handle_key(key(KeyCode::Char('b')));
    state.handle_key(key(KeyCode::Char('/')));

    let action = state.handle_key(key(KeyCode::Esc));

    assert_eq!(action, TuiAction::None);
    assert_eq!(state.mode, TuiMode::Search);
    assert_eq!(state.filter, "b");

    state.handle_key(key(KeyCode::Char('/')));
    let action = state.handle_key(key(KeyCode::Char('/')));

    assert_eq!(action, TuiAction::None);
    assert_eq!(state.mode, TuiMode::Search);
    assert_eq!(state.filter, "b/");
}

#[test]
fn escape_clears_filter_and_restores_previous_selection() {
    let mut state = TuiState::new(work_list());
    state.move_selection(1);
    let selected_before = state.selected_work().map(|work| work.slug.clone());
    for character in "context".chars() {
        state.handle_key(key(KeyCode::Char(character)));
    }

    let action = state.handle_key(key(KeyCode::Esc));

    assert_eq!(action, TuiAction::None);
    assert_eq!(state.mode, TuiMode::List);
    assert_eq!(state.filter, "");
    assert_eq!(
        state.selected_work().map(|work| work.slug.clone()),
        selected_before
    );
}

#[test]
fn clearing_filter_context_allows_next_filter_to_restore_current_selection() {
    let mut state = TuiState::new(work_list());
    state.move_selection(1);
    for character in "context".chars() {
        state.handle_key(key(KeyCode::Char(character)));
    }

    state.clear_filter_context();
    state.select_slug("billing-retry-audit");
    state.close_panel();
    for character in "review".chars() {
        state.handle_key(key(KeyCode::Char(character)));
    }

    let action = state.handle_key(key(KeyCode::Esc));

    assert_eq!(action, TuiAction::None);
    assert_eq!(state.mode, TuiMode::List);
    assert_eq!(state.filter, "");
    assert_eq!(
        state.selected_work().map(|work| work.slug.as_str()),
        Some("billing-retry-audit")
    );
}

#[test]
fn enter_on_empty_filter_match_does_not_switch_or_close_filter() {
    let mut state = TuiState::new(work_list());
    for character in "missing".chars() {
        state.handle_key(key(KeyCode::Char(character)));
    }

    let action = state.handle_key(key(KeyCode::Enter));

    assert_eq!(action, TuiAction::None);
    assert_eq!(state.mode, TuiMode::Search);
    assert_eq!(state.filter, "missing");
}

#[test]
fn backspace_edits_filter_and_resets_selection() {
    let mut state = TuiState::new(work_list());
    for character in "review".chars() {
        state.handle_key(key(KeyCode::Char(character)));
    }
    state.handle_key(key(KeyCode::Down));

    let action = state.handle_key(key(KeyCode::Backspace));

    assert_eq!(action, TuiAction::None);
    assert_eq!(state.mode, TuiMode::Search);
    assert_eq!(state.filter, "revie");
    assert_eq!(state.selected, 0);
}

#[test]
fn escape_with_empty_filter_quits() {
    let mut state = TuiState::new(work_list());

    assert_eq!(state.handle_key(key(KeyCode::Esc)), TuiAction::Quit);
}

#[test]
fn modified_list_shortcuts_are_ignored() {
    for (code, modifiers) in [
        (KeyCode::Char('q'), KeyModifiers::ALT),
        (KeyCode::Char('j'), KeyModifiers::CONTROL),
        (KeyCode::Char('k'), KeyModifiers::ALT),
        (KeyCode::Char('n'), KeyModifiers::ALT),
        (KeyCode::Char('a'), KeyModifiers::CONTROL),
        (KeyCode::Char('?'), KeyModifiers::ALT),
        (KeyCode::Char('/'), KeyModifiers::CONTROL),
    ] {
        let mut state = TuiState::new(work_list());

        let action = state.handle_key(KeyEvent::new(code, modifiers));

        assert_eq!(action, TuiAction::None);
        assert_eq!(state.mode, TuiMode::List);
        assert_eq!(state.selected, 0);
        assert!(state.trace.is_empty());
    }
}

#[test]
fn archive_confirm_accepts_enter_and_y() {
    let mut enter_state = TuiState::new(work_list());
    enter_state.mode = TuiMode::Archive;
    let enter_action = enter_state.handle_key(key(KeyCode::Enter));

    let mut y_state = TuiState::new(work_list());
    y_state.mode = TuiMode::Archive;
    let y_action = y_state.handle_key(key(KeyCode::Char('y')));

    assert_eq!(
        enter_action,
        TuiAction::Archive("billing-retry-audit".to_string())
    );
    assert_eq!(y_action, enter_action);
}

#[test]
fn create_intent_navigation_accepts_horizontal_keys() {
    let mut state = TuiState::new(work_list()).with_intents(vec![
        ("investigate".to_string(), "Investigate".to_string()),
        ("review-pr".to_string(), "Review".to_string()),
    ]);
    state.mode = TuiMode::Create;

    assert_eq!(state.handle_key(key(KeyCode::Right)), TuiAction::None);
    assert_eq!(state.create_intent, 1);

    assert_eq!(state.handle_key(key(KeyCode::Left)), TuiAction::None);
    assert_eq!(state.create_intent, 0);

    assert_eq!(state.handle_key(key(KeyCode::BackTab)), TuiAction::None);
    assert_eq!(state.create_intent, 1);
}

#[test]
fn create_defaults_to_blank_when_intent_list_is_empty() {
    let mut state = TuiState::new(work_list());
    state.mode = TuiMode::Create;
    state.create_goal = "Capture release checklist".to_string();

    assert_eq!(
        state.handle_key(key(KeyCode::Enter)),
        TuiAction::Create {
            goal: "Capture release checklist".to_string(),
            intent_id: "blank".to_string(),
        }
    );
}

#[test]
fn create_defaults_to_blank_when_intent_list_contains_seeded_defaults() {
    let mut state = TuiState::new(work_list()).with_intents(vec![
        (
            "address-pr-comments".to_string(),
            "Address PR Comments".to_string(),
        ),
        ("blank".to_string(), "Blank".to_string()),
        ("investigate".to_string(), "Investigate".to_string()),
    ]);
    state.mode = TuiMode::Create;
    state.create_goal = "Capture release checklist".to_string();

    assert_eq!(
        state.handle_key(key(KeyCode::Enter)),
        TuiAction::Create {
            goal: "Capture release checklist".to_string(),
            intent_id: "blank".to_string(),
        }
    );
}

fn work_list() -> WorkList {
    WorkList {
        works: vec![
            WorkSummary {
                title: "Billing retry audit".to_string(),
                slug: "billing-retry-audit".to_string(),
                goal: "Find why billing retry alerts spiked after the queue rollout.".to_string(),
                intent_id: "investigate".to_string(),
                path: PathBuf::from("/tmp/workon/.workon/work/billing-retry-audit"),
            },
            WorkSummary {
                title: "Review cache invalidation PR".to_string(),
                slug: "review-cache-invalidation-pr".to_string(),
                goal: "Review the cache invalidation PR for regressions.".to_string(),
                intent_id: "review-pr".to_string(),
                path: PathBuf::from("/tmp/workon/.workon/work/review-cache-invalidation-pr"),
            },
            WorkSummary {
                title: "Context command sketch".to_string(),
                slug: "context-command-sketch".to_string(),
                goal: "Shape the first context surface.".to_string(),
                intent_id: "brainstorm".to_string(),
                path: PathBuf::from("/tmp/workon/.workon/work/context-command-sketch"),
            },
        ],
    }
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn intent_list() -> Vec<(String, String)> {
    vec![
        (
            "investigate".to_string(),
            "Answer a technical question with evidence.".to_string(),
        ),
        (
            "review-pr".to_string(),
            "Review a peer pull request from local context.".to_string(),
        ),
        (
            "brainstorm".to_string(),
            "Explore an idea without committing to implementation.".to_string(),
        ),
    ]
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

fn attached_repositories() -> Vec<AttachedRepository> {
    vec![
        AttachedRepository {
            name_with_owner: "openai/api".to_string(),
            branch: "workon/billing-retry-audit".to_string(),
            path: PathBuf::from("/tmp/workon/.workon/work/billing-retry-audit/repos/openai__api"),
            default_branch: "main".to_string(),
            url: "https://github.com/openai/api".to_string(),
        },
        AttachedRepository {
            name_with_owner: "openai/workon".to_string(),
            branch: "workon/billing-retry-audit".to_string(),
            path: PathBuf::from(
                "/tmp/workon/.workon/work/billing-retry-audit/repos/openai__workon",
            ),
            default_branch: "trunk".to_string(),
            url: "https://github.com/openai/workon".to_string(),
        },
    ]
}

fn local_candidates() -> Vec<RepositoryCandidate> {
    vec![RepositoryCandidate {
        name_with_owner: "openai/local-tool".to_string(),
        branch: "feature/workon".to_string(),
        path: PathBuf::from("/tmp/repos/local-tool"),
        url: "https://github.com/openai/local-tool".to_string(),
    }]
}

fn duplicate_local_candidate() -> Vec<RepositoryCandidate> {
    vec![RepositoryCandidate {
        name_with_owner: "openai/api-docs".to_string(),
        branch: "feature/workon".to_string(),
        path: PathBuf::from("/tmp/repos/api-docs"),
        url: "https://github.com/openai/api-docs".to_string(),
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
