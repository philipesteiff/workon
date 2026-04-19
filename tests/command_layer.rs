use std::fs;
use std::path::{Path, PathBuf};

use workon::{App, Command, CommandOutput, WorkonError};

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
