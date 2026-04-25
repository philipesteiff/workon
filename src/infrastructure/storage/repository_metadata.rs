use std::fs;
use std::path::Path;

use crate::domain::repository_context::paths::REPOSITORY_METADATA_FILE;
use crate::domain::RepositoryAttachment;
use crate::shared::error::{Result, WorkonError};

pub(crate) trait RepoMetadataStore {
    fn read(&self, work_path: &Path) -> Result<Vec<RepositoryAttachment>>;
    fn write(&self, work_path: &Path, repositories: &[RepositoryAttachment]) -> Result<()>;
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct JsonRepoMetadataStore;

impl RepoMetadataStore for JsonRepoMetadataStore {
    fn read(&self, work_path: &Path) -> Result<Vec<RepositoryAttachment>> {
        let path = work_path.join(REPOSITORY_METADATA_FILE);
        if !path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&path)?;
        serde_json::from_str(&content).map_err(|error| WorkonError::RepositoryContext {
            message: format!("invalid repository metadata at {}: {error}", path.display()),
        })
    }

    fn write(&self, work_path: &Path, repositories: &[RepositoryAttachment]) -> Result<()> {
        let content = serde_json::to_string_pretty(repositories).map_err(|error| {
            WorkonError::RepositoryContext {
                message: format!("could not write repository metadata: {error}"),
            }
        })?;
        fs::write(
            work_path.join(REPOSITORY_METADATA_FILE),
            format!("{content}\n"),
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use crate::domain::RepositoryAttachment;

    use super::{JsonRepoMetadataStore, RepoMetadataStore};

    #[test]
    fn reads_missing_metadata_as_empty_repositories() {
        let root = temp_root("repo_metadata_missing");
        let store = JsonRepoMetadataStore;

        let repositories = store
            .read(root.path())
            .expect("missing metadata should be empty");

        assert!(repositories.is_empty());
    }

    #[test]
    fn writes_pretty_repository_metadata() {
        let root = temp_root("repo_metadata_write");
        let store = JsonRepoMetadataStore;

        store
            .write(root.path(), &[attached_repository()])
            .expect("metadata should write");

        let content =
            fs::read_to_string(root.path().join("workon.repos.json")).expect("metadata readable");
        assert!(content.contains("\"name_with_owner\": \"openai/workon\""));
        assert!(content.contains("\"default_branch\": \"main\""));
        assert!(content.contains("\"url\": \"https://github.com/openai/workon\""));
        assert!(!content.contains("\"branch\""));
        assert!(!content.contains("\"path\""));
        assert!(content.ends_with('\n'));
    }

    #[test]
    fn reads_legacy_metadata_while_ignoring_reconstructable_fields() {
        let root = temp_root("repo_metadata_legacy");
        let store = JsonRepoMetadataStore;
        fs::write(
            root.path().join("workon.repos.json"),
            r#"[
  {
    "name_with_owner": "openai/workon",
    "branch": "workon/old",
    "path": "/tmp/work/repos/openai__workon",
    "default_branch": "main",
    "url": "https://github.com/openai/workon"
  }
]
"#,
        )
        .expect("legacy metadata should write");

        let repositories = store
            .read(root.path())
            .expect("legacy metadata should read");

        assert_eq!(repositories, vec![attached_repository()]);
    }

    fn attached_repository() -> RepositoryAttachment {
        RepositoryAttachment {
            name_with_owner: "openai/workon".to_string(),
            default_branch: "main".to_string(),
            url: "https://github.com/openai/workon".to_string(),
        }
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
}
