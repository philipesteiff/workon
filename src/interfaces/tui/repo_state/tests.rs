use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::domain::{AttachedRepository, AvailableRepository, RepositoryCandidate};

use super::{RepoCatalogRow, RepoPane, RepoPickerAction, RepoPickerState};

#[test]
fn repo_picker_applies_pending_adds_and_removes() {
    let mut picker = picker();
    picker.handle_key(key(KeyCode::Char(' ')));
    picker.handle_key(key(KeyCode::Down));
    picker.handle_key(key(KeyCode::Char(' ')));

    assert!(matches!(
        picker.handle_key(key(KeyCode::Enter)),
        RepoPickerAction::Apply {
            add,
            remove,
            force_remove: false,
            ..
        } if add == ["openai/api"] && remove == ["openai/workon"]
    ));
}

#[test]
fn repo_picker_arms_force_only_for_pending_removals() {
    let mut picker = picker();
    picker.handle_key(key(KeyCode::Char(' ')));
    picker.handle_key(key(KeyCode::Char('!')));

    assert!(picker.force_remove);
    assert!(picker.pending_remove.contains("openai/workon"));
}

#[test]
fn repo_picker_reports_missing_force_selection_as_notification() {
    let mut picker = picker();

    let action = picker.handle_key(key(KeyCode::Char('!')));

    assert!(matches!(action, RepoPickerAction::Notify(_)));
}

#[test]
fn repo_picker_keeps_attached_repositories_in_catalog_when_sources_are_empty() {
    let mut picker = RepoPickerState::default();
    picker.enter_context(
        "billing-retry-audit".to_string(),
        "Billing retry audit".to_string(),
        Vec::new(),
        Vec::new(),
        attached_repositories(),
        vec![crate::domain::RepositoryWorkspace {
            path: "/tmp/repos".into(),
        }],
    );

    let rows = picker.catalog_rows();

    assert_eq!(rows.len(), attached_repositories().len());
    assert!(rows.iter().any(|row| row.name() == "openai/workon"));
}

#[test]
fn repo_picker_sorts_attached_then_local_then_github_rows() {
    let mut picker = picker();
    picker.candidates = vec![RepositoryCandidate {
        name_with_owner: "openai/local-tool".to_string(),
        branch: "feature/workon".to_string(),
        path: "/tmp/repos/local-tool".into(),
        url: "https://github.com/openai/local-tool".to_string(),
    }];

    let names = picker
        .catalog_rows()
        .into_iter()
        .map(|row| match row {
            RepoCatalogRow::GitHub(repository) => {
                format!("github:{}", repository.name_with_owner)
            }
            RepoCatalogRow::Local(candidate) => {
                format!("local:{}", candidate.name_with_owner)
            }
            RepoCatalogRow::PendingLocal { path, .. } => {
                format!("pending:{}", path.display())
            }
            RepoCatalogRow::Attached(repository) => {
                format!("attached:{}", repository.name_with_owner)
            }
        })
        .collect::<Vec<_>>();

    assert_eq!(
        names,
        vec![
            "github:openai/workon".to_string(),
            "local:openai/local-tool".to_string(),
            "github:openai/api".to_string(),
        ]
    );
}

#[test]
fn repo_picker_includes_pending_local_candidate_rows() {
    let mut picker = picker();
    picker.pending_candidate_paths = vec![crate::domain::RepositoryCandidatePath {
        path: "/tmp/repos/pending-api".into(),
        cached: None,
    }];

    let rows = picker.catalog_rows();

    assert!(rows.iter().any(|row| matches!(
        row,
        RepoCatalogRow::PendingLocal { path, failed: false }
            if path.ends_with("pending-api")
    )));
}

#[test]
fn repo_picker_focus_cycles_between_repository_workspace_panels() {
    let mut picker = picker();

    picker.handle_key(key(KeyCode::Right));
    assert_eq!(picker.focus, RepoPane::AddPath);

    picker.handle_key(key(KeyCode::Right));
    assert_eq!(picker.focus, RepoPane::ConfiguredPaths);

    picker.handle_key(key(KeyCode::Right));
    assert_eq!(picker.focus, RepoPane::Catalog);
}

fn picker() -> RepoPickerState {
    let mut picker = RepoPickerState::default();
    picker.enter_context(
        "billing-retry-audit".to_string(),
        "Billing retry audit".to_string(),
        available_repositories(),
        Vec::new(),
        attached_repositories(),
        vec![crate::domain::RepositoryWorkspace {
            path: "/tmp/repos".into(),
        }],
    );
    picker
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
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
            default_branch: "main".to_string(),
            url: "https://github.com/openai/workon".to_string(),
            ssh_url: "git@github.com:openai/workon.git".to_string(),
        },
    ]
}

fn attached_repositories() -> Vec<AttachedRepository> {
    vec![AttachedRepository {
        name_with_owner: "openai/workon".to_string(),
        branch: "workon/billing-retry-audit".to_string(),
        path: "/tmp/workon/.workon/work/billing-retry-audit/repos/openai__workon".into(),
        default_branch: "main".to_string(),
        url: "https://github.com/openai/workon".to_string(),
    }]
}
