#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::fs;
    use std::path::{Path, PathBuf};

    use crate::application::CommandOutput;
    use crate::domain::{
        AttachedRepository, AvailableRepository, RepositoryAttachment, RepositoryCandidate,
        RepositoryWorkspace, WorkSummary,
    };
    use crate::infrastructure::agent_files::RepoContextFileWriter;
    use crate::infrastructure::git_worktree::{
        RepositoryLinker, WorktreeInspector, WorktreeManager,
    };
    use crate::infrastructure::github::GithubClient;
    use crate::infrastructure::storage::{
        RepoMetadataStore, RepoWorkspaceStore, RepositoryCache, WorkStore,
    };
    use crate::shared::error::{Result, WorkonError};

    use super::super::service::{RepositoryContextDeps, RepositoryContextService};

    #[test]
    fn add_does_not_persist_metadata_when_created_worktree_cannot_be_scanned() {
        let root = temp_root("repo_service_add_scan_failure");
        let store = WorkStore::new(root.path().to_path_buf());
        let work = store
            .create("Repository scan failure", "investigate")
            .expect("work should create");
        let github = FakeGithub;
        let cache = FakeCache;
        let worktrees = FakeWorktrees::default();
        let inspector = FakeInspector::new(vec![Vec::new(), Vec::new()]);
        let linker = FakeLinker::default();
        let metadata = FakeMetadata::default();
        let workspaces = FakeWorkspaces::with_root(root.path().join("repo-workspace"));
        let context_files = FakeContextFiles::default();
        let service = RepositoryContextService::new(RepositoryContextDeps {
            store: &store,
            github: &github,
            cache: &cache,
            worktrees: &worktrees,
            inspector: &inspector,
            linker: &linker,
            metadata: &metadata,
            workspaces: &workspaces,
            context_files: &context_files,
        });

        let error = service
            .add(&work.slug, &["openai/workon".to_string()], None)
            .expect_err("missing post-add scan result should fail");

        assert!(error
            .to_string()
            .contains("worktree was created but could not be inspected"));
        assert_eq!(worktrees.switch_count(), 1);
        assert_eq!(metadata.write_count(), 0);
        assert_eq!(context_files.rewrite_count(), 0);
        assert!(metadata.repositories().is_empty());
    }

    #[test]
    fn remove_treats_metadata_without_worktree_folder_as_not_attached() {
        let root = temp_root("repo_service_remove_metadata_without_folder");
        let store = WorkStore::new(root.path().to_path_buf());
        let work = store
            .create("Stale repository metadata", "investigate")
            .expect("work should create");
        let github = FakeGithub;
        let cache = FakeCache;
        let worktrees = FakeWorktrees::default();
        let inspector = FakeInspector::new(vec![Vec::new()]);
        let linker = FakeLinker::default();
        let metadata = FakeMetadata::with_repositories(vec![repository_attachment()]);
        let workspaces = FakeWorkspaces::default();
        let context_files = FakeContextFiles::default();
        let service = RepositoryContextService::new(RepositoryContextDeps {
            store: &store,
            github: &github,
            cache: &cache,
            worktrees: &worktrees,
            inspector: &inspector,
            linker: &linker,
            metadata: &metadata,
            workspaces: &workspaces,
            context_files: &context_files,
        });

        let error = service
            .remove(&work.slug, &["openai/workon".to_string()], false)
            .expect_err("stale metadata without a scanned worktree is not attached");

        assert!(error.to_string().contains("is not attached"));
        assert_eq!(linker.remove_count(), 0);
        assert_eq!(metadata.write_count(), 0);
        assert_eq!(metadata.repositories(), vec![repository_attachment()]);
        assert_eq!(context_files.rewrite_count(), 0);
    }

    #[test]
    fn list_reports_scanned_worktree_even_without_metadata() {
        let root = temp_root("repo_service_list_worktree_without_metadata");
        let store = WorkStore::new(root.path().to_path_buf());
        let work = store
            .create("Manual repository worktree", "investigate")
            .expect("work should create");
        let attached = attached_repository(work.path.join("repos/openai__workon"));
        let inspector = FakeInspector::new(vec![vec![attached.clone()]]);
        let metadata = FakeMetadata::default();

        let CommandOutput::WorkRepositories(list) =
            RepositoryContextService::attached(&store, &metadata, &inspector, &work.slug)
                .expect("attached repos should list")
        else {
            panic!("expected WorkRepositories output");
        };

        assert_eq!(list.repositories, vec![attached]);
        assert_eq!(metadata.write_count(), 0);
    }

    #[test]
    fn remove_can_drop_scanned_worktree_that_has_no_metadata() {
        let root = temp_root("repo_service_remove_worktree_without_metadata");
        let store = WorkStore::new(root.path().to_path_buf());
        let work = store
            .create("Remove manual repository worktree", "investigate")
            .expect("work should create");
        let attached = attached_repository(work.path.join("repos/openai__workon"));
        let github = FakeGithub;
        let cache = FakeCache;
        let worktrees = FakeWorktrees::default();
        let inspector = FakeInspector::new(vec![vec![attached.clone()], Vec::new()]);
        let linker = FakeLinker::default();
        let metadata = FakeMetadata::default();
        let workspaces = FakeWorkspaces::default();
        let context_files = FakeContextFiles::default();
        let service = RepositoryContextService::new(RepositoryContextDeps {
            store: &store,
            github: &github,
            cache: &cache,
            worktrees: &worktrees,
            inspector: &inspector,
            linker: &linker,
            metadata: &metadata,
            workspaces: &workspaces,
            context_files: &context_files,
        });

        let CommandOutput::WorkRepositoriesRemoved(change) = service
            .remove(&work.slug, &["openai/workon".to_string()], false)
            .expect("scanned worktree without metadata should remove")
        else {
            panic!("expected WorkRepositoriesRemoved output");
        };

        assert_eq!(change.repositories, vec![attached]);
        assert_eq!(linker.remove_count(), 1);
        assert_eq!(metadata.write_count(), 1);
        assert!(metadata.repositories().is_empty());
        assert_eq!(context_files.rewrite_count(), 1);
        assert!(context_files.last_repositories().is_empty());
    }

    #[derive(Default)]
    struct FakeGithub;

    impl GithubClient for FakeGithub {
        fn list_repositories(&self) -> Result<Vec<AvailableRepository>> {
            Ok(vec![available_repository("openai/workon")])
        }

        fn fetch_repository(&self, name_with_owner: &str) -> Result<AvailableRepository> {
            Ok(available_repository(name_with_owner))
        }

        fn clone_bare(&self, _name_with_owner: &str, _cache_path: &Path) -> Result<()> {
            Ok(())
        }

        fn fetch_cache(&self, _cache_path: &Path) -> Result<()> {
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeCache;

    impl RepositoryCache for FakeCache {
        fn ensure(&self, repository: &AvailableRepository) -> Result<PathBuf> {
            self.path_for(&repository.name_with_owner)
        }

        fn path_for(&self, name_with_owner: &str) -> Result<PathBuf> {
            Ok(PathBuf::from(format!(
                "/cache/{}.git",
                name_with_owner.replace('/', "__")
            )))
        }
    }

    #[derive(Default)]
    struct FakeWorktrees {
        switches: RefCell<Vec<PathBuf>>,
    }

    impl FakeWorktrees {
        fn switch_count(&self) -> usize {
            self.switches.borrow().len()
        }
    }

    impl WorktreeManager for FakeWorktrees {
        fn switch(
            &self,
            _repository: &crate::domain::AvailableRepository,
            _cache_path: &Path,
            worktree_path: &Path,
            _branch: &str,
        ) -> Result<()> {
            std::fs::create_dir_all(worktree_path)?;
            self.switches.borrow_mut().push(worktree_path.to_path_buf());
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeLinker {
        links: RefCell<Vec<PathBuf>>,
        removes: RefCell<Vec<PathBuf>>,
    }

    impl FakeLinker {
        fn remove_count(&self) -> usize {
            self.removes.borrow().len()
        }
    }

    impl RepositoryLinker for FakeLinker {
        fn link(
            &self,
            work_path: &Path,
            preferred_alias: &str,
            _name_with_owner: &str,
            _target_path: &Path,
        ) -> Result<PathBuf> {
            let link_path = work_path.join("repos").join(preferred_alias);
            self.links.borrow_mut().push(link_path.clone());
            Ok(link_path)
        }

        fn remove(&self, link_path: &Path) -> Result<()> {
            self.removes.borrow_mut().push(link_path.to_path_buf());
            Ok(())
        }
    }

    struct FakeInspector {
        scans: RefCell<Vec<Vec<AttachedRepository>>>,
    }

    impl FakeInspector {
        fn new(scans: Vec<Vec<AttachedRepository>>) -> Self {
            Self {
                scans: RefCell::new(scans.into_iter().rev().collect()),
            }
        }
    }

    impl WorktreeInspector for FakeInspector {
        fn scan(
            &self,
            _work_path: &Path,
            _metadata: &[RepositoryAttachment],
        ) -> Result<Vec<AttachedRepository>> {
            self.scans
                .borrow_mut()
                .pop()
                .ok_or_else(|| WorkonError::RepositoryContext {
                    message: "unexpected repository scan".to_string(),
                })
        }

        fn inspect_candidate(&self, path: &Path) -> Result<Option<RepositoryCandidate>> {
            Ok(Some(RepositoryCandidate {
                name_with_owner: "openai/workon".to_string(),
                branch: "feature/manual".to_string(),
                path: path.to_path_buf(),
                url: "https://github.com/openai/workon".to_string(),
            }))
        }

        fn discover_candidate_paths(&self, roots: &[PathBuf]) -> Result<Vec<PathBuf>> {
            Ok(roots.to_vec())
        }

        fn discover(&self, _roots: &[PathBuf]) -> Result<Vec<RepositoryCandidate>> {
            Ok(Vec::new())
        }
    }

    #[derive(Default)]
    struct FakeWorkspaces {
        workspaces: RefCell<Vec<RepositoryWorkspace>>,
    }

    impl FakeWorkspaces {
        fn with_root(path: PathBuf) -> Self {
            Self {
                workspaces: RefCell::new(vec![RepositoryWorkspace { path }]),
            }
        }
    }

    impl RepoWorkspaceStore for FakeWorkspaces {
        fn read(&self) -> Result<Vec<RepositoryWorkspace>> {
            Ok(self.workspaces.borrow().clone())
        }

        fn write(&self, workspaces: &[RepositoryWorkspace]) -> Result<()> {
            *self.workspaces.borrow_mut() = workspaces.to_vec();
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeMetadata {
        repositories: RefCell<Vec<RepositoryAttachment>>,
        writes: RefCell<usize>,
    }

    impl FakeMetadata {
        fn with_repositories(repositories: Vec<RepositoryAttachment>) -> Self {
            Self {
                repositories: RefCell::new(repositories),
                writes: RefCell::new(0),
            }
        }

        fn repositories(&self) -> Vec<RepositoryAttachment> {
            self.repositories.borrow().clone()
        }

        fn write_count(&self) -> usize {
            *self.writes.borrow()
        }
    }

    impl RepoMetadataStore for FakeMetadata {
        fn read(&self, _work_path: &Path) -> Result<Vec<RepositoryAttachment>> {
            Ok(self.repositories())
        }

        fn write(&self, _work_path: &Path, repositories: &[RepositoryAttachment]) -> Result<()> {
            *self.writes.borrow_mut() += 1;
            *self.repositories.borrow_mut() = repositories.to_vec();
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeContextFiles {
        rewrites: RefCell<usize>,
        repositories: RefCell<Vec<AttachedRepository>>,
    }

    impl FakeContextFiles {
        fn rewrite_count(&self) -> usize {
            *self.rewrites.borrow()
        }

        fn last_repositories(&self) -> Vec<AttachedRepository> {
            self.repositories.borrow().clone()
        }
    }

    impl RepoContextFileWriter for FakeContextFiles {
        fn rewrite(&self, _work: &WorkSummary, repositories: &[AttachedRepository]) -> Result<()> {
            *self.rewrites.borrow_mut() += 1;
            *self.repositories.borrow_mut() = repositories.to_vec();
            Ok(())
        }
    }

    fn available_repository(name_with_owner: &str) -> AvailableRepository {
        AvailableRepository {
            name_with_owner: name_with_owner.to_string(),
            default_branch: "main".to_string(),
            url: format!("https://github.com/{name_with_owner}"),
            ssh_url: format!("git@github.com:{name_with_owner}.git"),
        }
    }

    fn repository_attachment() -> RepositoryAttachment {
        RepositoryAttachment {
            name_with_owner: "openai/workon".to_string(),
            default_branch: "main".to_string(),
            url: "https://github.com/openai/workon".to_string(),
            alias: "workon".to_string(),
            target_path: "/tmp/workon".into(),
        }
    }

    fn attached_repository(path: PathBuf) -> AttachedRepository {
        AttachedRepository {
            name_with_owner: "openai/workon".to_string(),
            branch: "feature/manual".to_string(),
            path,
            default_branch: String::new(),
            url: String::new(),
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
