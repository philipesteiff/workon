use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use workon::{App, Command, CommandOutput, WorkonError};

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn create_work_writes_agent_files_and_metadata() {
    let root = temp_root("create_work_writes_agent_files_and_metadata");
    let app = App::new(root.path().to_path_buf());

    let output = app
        .execute(Command::CreateWork {
            goal: "As SE, I want to answer a technical question for my manager that needs investigation across one or more repositories.".to_string(),
            intent_id: "investigate".to_string(),
        })
        .expect("create work should succeed");

    let CommandOutput::WorkCreated(created) = output else {
        panic!("expected WorkCreated output");
    };

    assert_eq!(created.intent_id, "investigate");
    assert!(created.path.ends_with("answer-technical-question-manager"));
    assert!(created.path.join("AGENTS.md").is_file());
    assert!(created.path.join("CLAUDE.md").is_file());
    assert!(created.path.join("workon.meta").is_file());

    let agents =
        fs::read_to_string(created.path.join("AGENTS.md")).expect("AGENTS.md should be readable");
    assert!(agents.contains("Goal"));
    assert!(agents.contains("answer a technical question"));
    assert!(agents.contains("evidence-skill"));
    assert!(agents.contains("slack"));
    assert!(agents.contains("repos/"));
    assert!(agents.contains("AGENTS.md is the source"));
    assert!(agents.contains("Treat Preferred Skills and Preferred MCPs as suggestions"));
    assert!(agents.contains("Record decisions and evidence"));
    assert!(agents.contains("Before acting"));
    assert!(agents.contains("Next"));
    assert!(!agents.contains("This file is a projection for AGENTS"));

    let claude =
        fs::read_to_string(created.path.join("CLAUDE.md")).expect("CLAUDE.md should be readable");
    assert!(claude.contains("AGENTS.md is the source"));
    assert!(claude.contains("This file is a projection for CLAUDE"));
}

#[test]
fn create_work_makes_slug_unique_when_title_repeats() {
    let root = temp_root("create_work_makes_slug_unique_when_title_repeats");
    let app = App::new(root.path().to_path_buf());

    let first = create_investigate(&app, "Answer billing question");
    let second = create_investigate(&app, "Answer billing question");

    assert!(first.path.ends_with("answer-billing-question"));
    assert!(second.path.ends_with("answer-billing-question-2"));
}

#[test]
fn list_and_open_work_use_existing_state() {
    let root = temp_root("list_and_open_work_use_existing_state");
    let app = App::new(root.path().to_path_buf());
    create_investigate(&app, "Investigate billing timeout");
    create_investigate(&app, "Prepare design document");

    let CommandOutput::WorkList(list) = app
        .execute(Command::ListWorks)
        .expect("list works should succeed")
    else {
        panic!("expected WorkList output");
    };
    assert_eq!(list.works.len(), 2);
    assert_eq!(list.works[0].title, "Investigate billing timeout");

    let CommandOutput::WorkOpened(opened) = app
        .execute(Command::OpenWork {
            query: "billing".to_string(),
        })
        .expect("open work should succeed")
    else {
        panic!("expected WorkOpened output");
    };
    assert_eq!(opened.title, "Investigate billing timeout");
}

#[test]
fn archive_work_moves_it_out_of_active_list() {
    let root = temp_root("archive_work_moves_it_out_of_active_list");
    let app = App::new(root.path().to_path_buf());
    let archived = create_investigate(&app, "Investigate billing timeout");
    let active = create_investigate(&app, "Prepare design document");

    let CommandOutput::WorkArchived(output) = app
        .execute(Command::ArchiveWork {
            query: "billing".to_string(),
        })
        .expect("archive should succeed")
    else {
        panic!("expected WorkArchived output");
    };

    assert_eq!(output.title, archived.title);
    assert_eq!(output.slug, archived.slug);
    assert_eq!(output.path, archived.path);
    assert!(output.archive_path.ends_with("investigate-billing-timeout"));
    assert!(!archived.path.exists());
    assert!(output.archive_path.is_dir());

    let CommandOutput::WorkList(list) = app
        .execute(Command::ListWorks)
        .expect("list works should succeed")
    else {
        panic!("expected WorkList output");
    };
    assert_eq!(list.works.len(), 1);
    assert_eq!(list.works[0].title, active.title);

    let error = app
        .execute(Command::OpenWork {
            query: archived.slug,
        })
        .expect_err("archived work should not open as active");
    assert!(matches!(error, WorkonError::WorkNotFound { .. }));
}

#[test]
fn archive_work_uses_open_matching_errors() {
    let root = temp_root("archive_work_uses_open_matching_errors");
    let app = App::new(root.path().to_path_buf());
    create_investigate(&app, "Answer billing question");
    create_investigate(&app, "Answer billing question");

    let error = app
        .execute(Command::ArchiveWork {
            query: "billing".to_string(),
        })
        .expect_err("ambiguous archive query should fail");

    assert!(matches!(error, WorkonError::AmbiguousWork { .. }));
}

#[test]
fn archive_work_uses_suffixed_destination_when_archive_exists() {
    let root = temp_root("archive_work_uses_suffixed_destination_when_archive_exists");
    let app = App::new(root.path().to_path_buf());
    let work = create_investigate(&app, "Answer billing question");
    let existing_archive = root.path().join(".workon/archive/answer-billing-question");
    fs::create_dir_all(&existing_archive).expect("existing archive should be created");

    let CommandOutput::WorkArchived(output) = app
        .execute(Command::ArchiveWork {
            query: work.slug.clone(),
        })
        .expect("archive should succeed")
    else {
        panic!("expected WorkArchived output");
    };

    assert_eq!(output.slug, work.slug);
    assert!(output.archive_path.ends_with("answer-billing-question-2"));
    assert!(output.archive_path.is_dir());
    assert!(existing_archive.is_dir());
}

#[test]
fn ambiguous_work_error_includes_slug_and_title() {
    let root = temp_root("ambiguous_work_error_includes_slug_and_title");
    let app = App::new(root.path().to_path_buf());
    create_investigate(&app, "Answer billing question");
    create_investigate(&app, "Answer billing question");

    let error = app
        .execute(Command::OpenWork {
            query: "billing".to_string(),
        })
        .expect_err("ambiguous query should fail");

    let WorkonError::AmbiguousWork { matches, .. } = error else {
        panic!("expected ambiguous work error");
    };

    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].slug, "answer-billing-question");
    assert_eq!(matches[0].title, "Answer billing question");
    assert_eq!(matches[1].slug, "answer-billing-question-2");
    assert_eq!(matches[1].title, "Answer billing question");
}

#[test]
fn context_command_is_explicitly_tbd_but_uses_command_layer() {
    let root = temp_root("context_command_is_explicitly_tbd_but_uses_command_layer");
    let app = App::new(root.path().to_path_buf());

    let CommandOutput::Context(context) = app
        .execute(Command::Context)
        .expect("context command should succeed")
    else {
        panic!("expected Context output");
    };

    assert_eq!(context.status, "TBD");
    assert!(context.message.contains("intent"));
    assert!(context.message.contains("skills"));
    assert!(context.message.contains("MCPs"));
    assert!(context.message.contains("repos"));
}

#[test]
fn unknown_intent_returns_typed_error() {
    let root = temp_root("unknown_intent_returns_typed_error");
    let app = App::new(root.path().to_path_buf());

    let error = app
        .execute(Command::CreateWork {
            goal: "Answer billing question".to_string(),
            intent_id: "missing".to_string(),
        })
        .expect_err("unknown intent should fail");

    assert!(matches!(error, WorkonError::UnknownIntent { .. }));
}

#[test]
fn install_shell_is_a_command_layer_feature() {
    let _guard = ENV_LOCK.lock().expect("env lock should not be poisoned");
    let home = temp_root("install_shell_is_a_command_layer_feature_home");
    let root = temp_root("install_shell_is_a_command_layer_feature_root");
    let previous_home = std::env::var_os("HOME");
    std::env::set_var("HOME", home.path());

    let app = App::new(root.path().to_path_buf());
    let output = app
        .execute(Command::InstallShell)
        .expect("install shell should succeed");

    restore_env("HOME", previous_home);

    let CommandOutput::ShellInstalled {
        label,
        script_path,
        zshrc_path,
    } = output
    else {
        panic!("expected ShellInstalled output");
    };

    assert_eq!(label, "shell integration installed");
    assert!(script_path.ends_with(".workon/shell/zsh/wo.zsh"));
    assert!(zshrc_path.ends_with(".zshrc"));
    assert!(script_path.is_file());
    assert!(zshrc_path.is_file());
}

#[test]
fn install_dev_shell_is_a_command_layer_feature() {
    let _guard = ENV_LOCK.lock().expect("env lock should not be poisoned");
    let home = temp_root("install_dev_shell_is_a_command_layer_feature_home");
    let root = temp_root("install_dev_shell_is_a_command_layer_feature_root");
    let manifest_path = root.path().join("Cargo.toml");
    let previous_home = std::env::var_os("HOME");
    std::env::set_var("HOME", home.path());

    let app = App::new(root.path().to_path_buf());
    let output = app
        .execute(Command::InstallDevShell {
            manifest_path: manifest_path.clone(),
        })
        .expect("install dev shell should succeed");

    restore_env("HOME", previous_home);

    let CommandOutput::ShellInstalled {
        label,
        script_path,
        zshrc_path,
    } = output
    else {
        panic!("expected ShellInstalled output");
    };

    let script = fs::read_to_string(&script_path).expect("script should be readable");

    assert_eq!(label, "dev shell integration installed");
    assert!(script_path.ends_with(".workon/shell/zsh/wo-dev.zsh"));
    assert!(zshrc_path.ends_with(".zshrc"));
    assert!(script.contains(&manifest_path.display().to_string()));
    assert!(script.contains("just()"));
}

#[test]
fn add_repository_workspaces_accepts_multiple_paths() {
    let root = temp_root("add_repository_workspaces_accepts_multiple_paths");
    let app = App::new(root.path().to_path_buf());
    let first = root.path().join("repo-workspace-a");
    let second = root.path().join("repo-workspace-b");

    let CommandOutput::RepositoryWorkspaces(list) = app
        .execute(Command::AddRepositoryWorkspaces {
            paths: vec![first.clone(), second.clone(), first.clone()],
        })
        .expect("repo workspaces should be configured")
    else {
        panic!("expected RepositoryWorkspaces output");
    };

    let paths = list
        .workspaces
        .into_iter()
        .map(|workspace| workspace.path)
        .collect::<Vec<_>>();
    let first = fs::canonicalize(first).expect("first workspace should exist");
    let second = fs::canonicalize(second).expect("second workspace should exist");
    assert_eq!(paths.len(), 2);
    assert!(paths.contains(&first));
    assert!(paths.contains(&second));
}

#[test]
fn add_work_repositories_uses_gh_and_git_worktree_then_updates_context_files() {
    let _guard = ENV_LOCK.lock().expect("env lock should not be poisoned");
    let root = temp_root("add_work_repositories_uses_gh_and_git_worktree");
    let fake_bin = fake_repo_tools(root.path());
    let previous_path = prepend_path(fake_bin.path());
    let previous_log = std::env::var_os("WORKON_FAKE_LOG");
    std::env::set_var("WORKON_FAKE_LOG", root.path().join("tool.log"));

    let app = App::new(root.path().to_path_buf());
    let repo_workspace = configure_repo_workspace(&app, root.path());
    let work = create_investigate(&app, "Investigate repository context");

    let CommandOutput::WorkRepositoriesAdded(change) = app
        .execute(Command::AddWorkRepositories {
            query: work.slug.clone(),
            repositories: vec!["openai/workon".to_string()],
            workspace: None,
        })
        .expect("repo add should succeed")
    else {
        panic!("expected WorkRepositoriesAdded output");
    };

    restore_env("PATH", previous_path);
    restore_env("WORKON_FAKE_LOG", previous_log);

    assert_eq!(change.work.slug, work.slug);
    assert_eq!(change.repositories.len(), 1);
    assert_eq!(change.repositories[0].name_with_owner, "openai/workon");
    assert_eq!(
        change.repositories[0].branch,
        "workon/investigate-repository-context"
    );
    assert!(change.repositories[0].path.ends_with("repos/workon"));
    assert!(work.path.join("repos/workon").is_dir());
    assert!(repo_workspace
        .join("investigate-repository-context/workon")
        .is_dir());

    let metadata =
        fs::read_to_string(work.path.join("workon.repos.json")).expect("metadata should exist");
    assert!(metadata.contains("\"name_with_owner\": \"openai/workon\""));
    assert!(metadata.contains("\"default_branch\": \"main\""));
    assert!(metadata.contains("\"url\": \"https://github.com/openai/workon\""));
    assert!(metadata.contains("\"alias\": \"workon\""));
    assert!(metadata.contains("\"target_path\""));
    assert!(!metadata.contains("\"branch\""));

    let agents =
        fs::read_to_string(work.path.join("AGENTS.md")).expect("AGENTS.md should be readable");
    assert!(agents.contains("- openai/workon"));
    assert!(agents.contains("repos/workon"));

    let log = fs::read_to_string(root.path().join("tool.log")).expect("tool log should exist");
    assert!(log.contains("gh repo view openai/workon"));
    assert!(log.contains("gh repo clone openai/workon"));
    assert!(log.contains("git -C"));
    assert!(log.contains("branch workon/investigate-repository-context main"));
    assert!(log.contains("worktree add"));
    assert!(log.contains("repo-workspace/investigate-repository-context/workon"));
}

#[test]
fn list_work_repositories_reconstructs_live_branch_from_repo_folder() {
    let _guard = ENV_LOCK.lock().expect("env lock should not be poisoned");
    let root = temp_root("list_work_repositories_reconstructs_live_branch");
    let fake_bin = fake_repo_tools(root.path());
    let previous_path = prepend_path(fake_bin.path());
    let previous_log = std::env::var_os("WORKON_FAKE_LOG");
    std::env::set_var("WORKON_FAKE_LOG", root.path().join("tool.log"));

    let app = App::new(root.path().to_path_buf());
    configure_repo_workspace(&app, root.path());
    let work = create_investigate(&app, "Track live repository branch");
    app.execute(Command::AddWorkRepositories {
        query: work.slug.clone(),
        repositories: vec!["openai/workon".to_string()],
        workspace: None,
    })
    .expect("repo add should succeed");
    fs::write(
        work.path.join("repos/workon/.workon-current-branch"),
        "feature/user-branch\n",
    )
    .expect("fake branch marker should update");

    let CommandOutput::WorkRepositories(list) = app
        .execute(Command::ListWorkRepositories {
            query: work.slug.clone(),
        })
        .expect("repo list should succeed")
    else {
        panic!("expected WorkRepositories output");
    };

    restore_env("PATH", previous_path);
    restore_env("WORKON_FAKE_LOG", previous_log);

    assert_eq!(list.repositories.len(), 1);
    assert_eq!(list.repositories[0].name_with_owner, "openai/workon");
    assert_eq!(list.repositories[0].branch, "feature/user-branch");

    let metadata =
        fs::read_to_string(work.path.join("workon.repos.json")).expect("metadata should exist");
    assert!(!metadata.contains("feature/user-branch"));
    assert!(!metadata.contains("\"branch\""));
}

#[test]
fn add_work_repositories_does_not_duplicate_existing_repository_context() {
    let _guard = ENV_LOCK.lock().expect("env lock should not be poisoned");
    let root = temp_root("add_work_repositories_does_not_duplicate");
    let fake_bin = fake_repo_tools(root.path());
    let previous_path = prepend_path(fake_bin.path());
    let previous_log = std::env::var_os("WORKON_FAKE_LOG");
    std::env::set_var("WORKON_FAKE_LOG", root.path().join("tool.log"));

    let app = App::new(root.path().to_path_buf());
    configure_repo_workspace(&app, root.path());
    let work = create_investigate(&app, "Investigate duplicate repo context");
    app.execute(Command::AddWorkRepositories {
        query: work.slug.clone(),
        repositories: vec!["openai/workon".to_string()],
        workspace: None,
    })
    .expect("first repo add should succeed");

    let CommandOutput::WorkRepositoriesAdded(change) = app
        .execute(Command::AddWorkRepositories {
            query: work.slug.clone(),
            repositories: vec!["openai/workon".to_string()],
            workspace: None,
        })
        .expect("duplicate repo add should succeed")
    else {
        panic!("expected WorkRepositoriesAdded output");
    };

    restore_env("PATH", previous_path);
    restore_env("WORKON_FAKE_LOG", previous_log);

    assert_eq!(change.repositories.len(), 0);
    let metadata =
        fs::read_to_string(work.path.join("workon.repos.json")).expect("metadata should exist");
    assert_eq!(
        metadata
            .matches("\"name_with_owner\": \"openai/workon\"")
            .count(),
        1
    );
}

#[test]
fn add_work_repositories_persists_successes_before_later_batch_failure() {
    let _guard = ENV_LOCK.lock().expect("env lock should not be poisoned");
    let root = temp_root("add_work_repositories_persists_partial_success");
    let fake_bin = fake_repo_tools(root.path());
    let previous_path = prepend_path(fake_bin.path());
    let previous_log = std::env::var_os("WORKON_FAKE_LOG");
    let previous_switch_failure = std::env::var_os("WORKON_FAKE_GIT_WORKTREE_ADD_FAIL_REPO");
    std::env::set_var("WORKON_FAKE_LOG", root.path().join("tool.log"));
    std::env::set_var("WORKON_FAKE_GIT_WORKTREE_ADD_FAIL_REPO", "broken");

    let app = App::new(root.path().to_path_buf());
    configure_repo_workspace(&app, root.path());
    let work = create_investigate(&app, "Partial add repository context");

    let error = app
        .execute(Command::AddWorkRepositories {
            query: work.slug.clone(),
            repositories: vec!["openai/workon".to_string(), "openai/broken".to_string()],
            workspace: None,
        })
        .expect_err("second repo add should fail");

    restore_env("PATH", previous_path);
    restore_env("WORKON_FAKE_LOG", previous_log);
    restore_env(
        "WORKON_FAKE_GIT_WORKTREE_ADD_FAIL_REPO",
        previous_switch_failure,
    );

    assert!(error.to_string().contains("Cannot add worktree"));
    let metadata =
        fs::read_to_string(work.path.join("workon.repos.json")).expect("metadata should exist");
    assert!(metadata.contains("\"name_with_owner\": \"openai/workon\""));
    assert!(!metadata.contains("\"name_with_owner\": \"openai/broken\""));

    let agents =
        fs::read_to_string(work.path.join("AGENTS.md")).expect("AGENTS.md should be readable");
    assert!(agents.contains("- openai/workon"));
    assert!(!agents.contains("- openai/broken"));
}

#[test]
fn remove_work_repositories_unlinks_symlink_and_updates_context_files() {
    let _guard = ENV_LOCK.lock().expect("env lock should not be poisoned");
    let root = temp_root("remove_work_repositories_unlinks_symlink");
    let fake_bin = fake_repo_tools(root.path());
    let previous_path = prepend_path(fake_bin.path());
    let previous_log = std::env::var_os("WORKON_FAKE_LOG");
    std::env::set_var("WORKON_FAKE_LOG", root.path().join("tool.log"));

    let app = App::new(root.path().to_path_buf());
    let repo_workspace = configure_repo_workspace(&app, root.path());
    let work = create_investigate(&app, "Remove repository context");
    app.execute(Command::AddWorkRepositories {
        query: work.slug.clone(),
        repositories: vec!["openai/workon".to_string()],
        workspace: None,
    })
    .expect("repo add should succeed");

    let CommandOutput::WorkRepositoriesRemoved(change) = app
        .execute(Command::RemoveWorkRepositories {
            query: work.slug.clone(),
            repositories: vec!["openai/workon".to_string()],
            force: false,
        })
        .expect("repo remove should succeed")
    else {
        panic!("expected WorkRepositoriesRemoved output");
    };

    restore_env("PATH", previous_path);
    restore_env("WORKON_FAKE_LOG", previous_log);

    assert_eq!(change.repositories.len(), 1);
    assert_eq!(change.repositories[0].name_with_owner, "openai/workon");

    let metadata =
        fs::read_to_string(work.path.join("workon.repos.json")).expect("metadata should exist");
    assert!(!metadata.contains("openai/workon"));

    let agents =
        fs::read_to_string(work.path.join("AGENTS.md")).expect("AGENTS.md should be readable");
    assert!(!agents.contains("- openai/workon"));
    assert!(agents.contains("No GitHub repositories attached yet."));

    let log = fs::read_to_string(root.path().join("tool.log")).expect("tool log should exist");
    assert!(!log.contains("worktree remove"));
    assert!(!work.path.join("repos/workon").exists());
    assert!(repo_workspace
        .join("remove-repository-context/workon")
        .is_dir());
}

#[test]
fn remove_work_repositories_fails_when_repository_is_not_attached() {
    let root = temp_root("remove_work_repositories_missing_attached_repo");
    let app = App::new(root.path().to_path_buf());
    let work = create_investigate(&app, "Missing repository context");

    let error = app
        .execute(Command::RemoveWorkRepositories {
            query: work.slug.clone(),
            repositories: vec!["openai/workon".to_string()],
            force: false,
        })
        .expect_err("missing attached repo should fail");

    assert!(error
        .to_string()
        .contains("repository `openai/workon` is not attached"));
}

#[test]
fn remove_work_repositories_persists_successes_before_later_batch_failure() {
    let _guard = ENV_LOCK.lock().expect("env lock should not be poisoned");
    let root = temp_root("remove_work_repositories_persists_partial_success");
    let fake_bin = fake_repo_tools(root.path());
    let previous_path = prepend_path(fake_bin.path());
    let previous_log = std::env::var_os("WORKON_FAKE_LOG");
    std::env::set_var("WORKON_FAKE_LOG", root.path().join("tool.log"));

    let app = App::new(root.path().to_path_buf());
    configure_repo_workspace(&app, root.path());
    let work = create_investigate(&app, "Partial remove repository context");
    app.execute(Command::AddWorkRepositories {
        query: work.slug.clone(),
        repositories: vec!["openai/workon".to_string(), "openai/api".to_string()],
        workspace: None,
    })
    .expect("repo add should succeed");

    app.execute(Command::RemoveWorkRepositories {
        query: work.slug.clone(),
        repositories: vec!["openai/workon".to_string(), "openai/api".to_string()],
        force: false,
    })
    .expect("repo link removals should not depend on git worktree dirtiness");

    restore_env("PATH", previous_path);
    restore_env("WORKON_FAKE_LOG", previous_log);

    let metadata =
        fs::read_to_string(work.path.join("workon.repos.json")).expect("metadata should exist");
    assert!(!metadata.contains("\"name_with_owner\": \"openai/workon\""));
    assert!(!metadata.contains("\"name_with_owner\": \"openai/api\""));

    let agents =
        fs::read_to_string(work.path.join("AGENTS.md")).expect("AGENTS.md should be readable");
    assert!(!agents.contains("- openai/workon"));
    assert!(!agents.contains("- openai/api"));
}

#[test]
fn remove_work_repositories_accepts_force_without_deleting_checkout() {
    let _guard = ENV_LOCK.lock().expect("env lock should not be poisoned");
    let root = temp_root("remove_work_repositories_can_force");
    let fake_bin = fake_repo_tools(root.path());
    let previous_path = prepend_path(fake_bin.path());
    let previous_log = std::env::var_os("WORKON_FAKE_LOG");
    std::env::set_var("WORKON_FAKE_LOG", root.path().join("tool.log"));

    let app = App::new(root.path().to_path_buf());
    configure_repo_workspace(&app, root.path());
    let work = create_investigate(&app, "Force remove repository context");
    app.execute(Command::AddWorkRepositories {
        query: work.slug.clone(),
        repositories: vec!["openai/workon".to_string()],
        workspace: None,
    })
    .expect("repo add should succeed");

    app.execute(Command::RemoveWorkRepositories {
        query: work.slug.clone(),
        repositories: vec!["openai/workon".to_string()],
        force: true,
    })
    .expect("force repo remove should succeed");

    restore_env("PATH", previous_path);
    restore_env("WORKON_FAKE_LOG", previous_log);

    let log = fs::read_to_string(root.path().join("tool.log")).expect("tool log should exist");
    assert!(!log.contains("worktree remove --force"));
    assert!(!work.path.join("repos/workon").exists());
}

#[test]
fn remove_work_repositories_keeps_checkout_when_unlinking() {
    let _guard = ENV_LOCK.lock().expect("env lock should not be poisoned");
    let root = temp_root("remove_work_repositories_keeps_metadata_on_failure");
    let fake_bin = fake_repo_tools(root.path());
    let previous_path = prepend_path(fake_bin.path());
    let previous_log = std::env::var_os("WORKON_FAKE_LOG");
    std::env::set_var("WORKON_FAKE_LOG", root.path().join("tool.log"));

    let app = App::new(root.path().to_path_buf());
    let repo_workspace = configure_repo_workspace(&app, root.path());
    let work = create_investigate(&app, "Dirty repository context");
    app.execute(Command::AddWorkRepositories {
        query: work.slug.clone(),
        repositories: vec!["openai/workon".to_string()],
        workspace: None,
    })
    .expect("repo add should succeed");

    app.execute(Command::RemoveWorkRepositories {
        query: work.slug.clone(),
        repositories: vec!["openai/workon".to_string()],
        force: false,
    })
    .expect("repo remove should unlink without deleting dirty worktree");

    restore_env("PATH", previous_path);
    restore_env("WORKON_FAKE_LOG", previous_log);

    let metadata =
        fs::read_to_string(work.path.join("workon.repos.json")).expect("metadata should exist");
    assert!(!metadata.contains("openai/workon"));
    assert!(repo_workspace
        .join("dirty-repository-context/workon")
        .is_dir());
    assert!(!work.path.join("repos/workon").exists());
}

fn create_investigate(app: &App, goal: &str) -> workon::CreatedWork {
    let CommandOutput::WorkCreated(work) = app
        .execute(Command::CreateWork {
            goal: goal.to_string(),
            intent_id: "investigate".to_string(),
        })
        .expect("create work should succeed")
    else {
        panic!("expected WorkCreated output");
    };
    work
}

fn configure_repo_workspace(app: &App, root: &Path) -> PathBuf {
    let path = root.join("repo-workspace");
    app.execute(Command::AddRepositoryWorkspaces {
        paths: vec![path.clone()],
    })
    .expect("repo workspace should be configured");
    path
}

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn temp_root(name: &str) -> TempRoot {
    let mut path = std::env::temp_dir();
    path.push(format!("workon-test-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("temp root should be created");
    TempRoot { path }
}

fn restore_env(key: &str, value: Option<std::ffi::OsString>) {
    match value {
        Some(value) => std::env::set_var(key, value),
        None => std::env::remove_var(key),
    }
}

fn prepend_path(path: &Path) -> Option<std::ffi::OsString> {
    let previous = std::env::var_os("PATH");
    let mut paths = vec![path.to_path_buf()];
    if let Some(previous) = previous.as_ref() {
        paths.extend(std::env::split_paths(previous));
    }
    let next = std::env::join_paths(paths).expect("PATH should join");
    std::env::set_var("PATH", next);
    previous
}

fn fake_repo_tools(root: &Path) -> TempRoot {
    let fake_bin = temp_root("fake_repo_tools");
    fs::write(fake_bin.path().join("gh"), fake_gh_script()).expect("gh fake should be written");
    fs::write(fake_bin.path().join("git"), fake_git_script()).expect("git fake should be written");

    for name in ["gh", "git"] {
        let path = fake_bin.path().join(name);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(&path)
                .expect("fake tool metadata should read")
                .permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).expect("fake tool should be executable");
        }
    }

    fs::write(root.join("tool.log"), "").expect("tool log should be initialized");
    fake_bin
}

fn fake_gh_script() -> &'static str {
    r#"#!/bin/sh
printf 'gh %s\n' "$*" >> "$WORKON_FAKE_LOG"
if [ "$1" = "repo" ] && [ "$2" = "view" ]; then
  repo="$3"
  printf '{"nameWithOwner":"%s","defaultBranchRef":{"name":"main"},"url":"https://github.com/%s","sshUrl":"git@github.com:%s.git"}\n' "$repo" "$repo" "$repo"
  exit 0
fi
if [ "$1" = "repo" ] && [ "$2" = "clone" ]; then
  mkdir -p "$4"
  exit 0
fi
if [ "$1" = "repo" ] && [ "$2" = "list" ]; then
  printf '[{"nameWithOwner":"openai/workon","defaultBranchRef":{"name":"main"},"url":"https://github.com/openai/workon","sshUrl":"git@github.com:openai/workon.git"}]\n'
  exit 0
fi
exit 2
"#
}

fn fake_git_script() -> &'static str {
    r#"#!/bin/sh
printf 'git %s\n' "$*" >> "$WORKON_FAKE_LOG"
if [ "$1" = "-C" ]; then
  cwd="$2"
  shift 2
fi
if [ "$1" = "fetch" ]; then
  exit 0
fi
if [ "$1" = "branch" ] && [ "$2" = "--list" ]; then
  exit 0
fi
if [ "$1" = "branch" ] && [ "$2" = "--show-current" ]; then
  if [ -n "$cwd" ] && [ -f "$cwd/.workon-current-branch" ]; then
    cat "$cwd/.workon-current-branch"
  fi
  exit 0
fi
if [ "$1" = "branch" ]; then
  exit 0
fi
if [ "$1" = "worktree" ] && [ "$2" = "add" ]; then
  path="$3"
  branch="$4"
  if [ -n "$WORKON_FAKE_GIT_WORKTREE_ADD_FAIL_REPO" ]; then
    case "$path" in
      *"$WORKON_FAKE_GIT_WORKTREE_ADD_FAIL_REPO"*)
        printf 'Cannot add worktree: repository cache is unavailable\n' >&2
        exit 8
        ;;
    esac
  fi
  mkdir -p "$path"
  printf '%s\n' "$branch" > "$path/.workon-current-branch"
  exit 0
fi
exit 0
"#
}
