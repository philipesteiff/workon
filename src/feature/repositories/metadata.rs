use std::fs;
use std::path::Path;

use crate::domain::AttachedRepository;
use crate::error::{Result, WorkonError};

use super::paths::REPOSITORY_METADATA_FILE;

pub(super) trait RepoMetadataStore {
    fn read(&self, work_path: &Path) -> Result<Vec<AttachedRepository>>;
    fn write(&self, work_path: &Path, repositories: &[AttachedRepository]) -> Result<()>;
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct JsonRepoMetadataStore;

impl RepoMetadataStore for JsonRepoMetadataStore {
    fn read(&self, work_path: &Path) -> Result<Vec<AttachedRepository>> {
        let path = work_path.join(REPOSITORY_METADATA_FILE);
        if !path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&path)?;
        serde_json::from_str(&content).map_err(|error| WorkonError::RepositoryContext {
            message: format!("invalid repository metadata at {}: {error}", path.display()),
        })
    }

    fn write(&self, work_path: &Path, repositories: &[AttachedRepository]) -> Result<()> {
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

    use crate::domain::AttachedRepository;

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
        assert!(content.ends_with('\n'));
    }

    fn attached_repository() -> AttachedRepository {
        AttachedRepository {
            name_with_owner: "openai/workon".to_string(),
            branch: "workon/test".to_string(),
            path: PathBuf::from("/tmp/work/repos/openai__workon"),
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
