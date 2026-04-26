use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::RepositoryWorkspace;
use crate::shared::error::Result;

const REPOSITORY_WORKSPACES_FILE: &str = "repo-workspaces.json";

pub(crate) trait RepoWorkspaceStore {
    fn read(&self) -> Result<Vec<RepositoryWorkspace>>;
    fn write(&self, workspaces: &[RepositoryWorkspace]) -> Result<()>;
}

pub(crate) struct JsonRepoWorkspaceStore<'a> {
    root: &'a Path,
}

impl<'a> JsonRepoWorkspaceStore<'a> {
    pub(crate) fn new(root: &'a Path) -> Self {
        Self { root }
    }

    fn path(&self) -> PathBuf {
        self.root.join(".workon").join(REPOSITORY_WORKSPACES_FILE)
    }
}

impl RepoWorkspaceStore for JsonRepoWorkspaceStore<'_> {
    fn read(&self) -> Result<Vec<RepositoryWorkspace>> {
        let path = self.path();
        if !path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(path)?;
        let paths: Vec<PathBuf> = serde_json::from_str(&content).map_err(|error| {
            crate::shared::error::WorkonError::RepositoryContext {
                message: format!("invalid repository workspace config: {error}"),
            }
        })?;
        let mut workspaces = paths
            .into_iter()
            .map(|path| RepositoryWorkspace { path })
            .collect::<Vec<_>>();
        workspaces.sort_by(|left, right| left.path.cmp(&right.path));
        workspaces.dedup_by(|left, right| left.path == right.path);
        Ok(workspaces)
    }

    fn write(&self, workspaces: &[RepositoryWorkspace]) -> Result<()> {
        let mut paths = workspaces
            .iter()
            .map(|workspace| workspace.path.clone())
            .collect::<Vec<_>>();
        paths.sort();
        paths.dedup();
        if let Some(parent) = self.path().parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(&paths).map_err(|error| {
            crate::shared::error::WorkonError::RepositoryContext {
                message: format!("could not write repository workspace config: {error}"),
            }
        })?;
        fs::write(self.path(), format!("{content}\n"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use crate::domain::RepositoryWorkspace;

    use super::{JsonRepoWorkspaceStore, RepoWorkspaceStore};

    #[test]
    fn reads_missing_workspaces_as_empty() {
        let root = temp_root("repo_workspaces_missing");
        let store = JsonRepoWorkspaceStore::new(root.path());

        let workspaces = store.read().expect("missing should read empty");

        assert!(workspaces.is_empty());
    }

    #[test]
    fn writes_sorted_deduped_workspaces() {
        let root = temp_root("repo_workspaces_write");
        let store = JsonRepoWorkspaceStore::new(root.path());

        store
            .write(&[
                RepositoryWorkspace {
                    path: "/tmp/b".into(),
                },
                RepositoryWorkspace {
                    path: "/tmp/a".into(),
                },
                RepositoryWorkspace {
                    path: "/tmp/a".into(),
                },
            ])
            .expect("workspaces should write");

        let workspaces = store.read().expect("workspaces should read");
        assert_eq!(
            workspaces,
            vec![
                RepositoryWorkspace {
                    path: "/tmp/a".into()
                },
                RepositoryWorkspace {
                    path: "/tmp/b".into()
                }
            ]
        );
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
