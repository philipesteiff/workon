use crate::application::CommandOutput;
use crate::domain::repository_context::paths::{
    normalize_requested_repositories, work_branch, worktree_path,
};
use crate::domain::{
    AttachedRepository, RepositoryAttachment, RepositoryCatalog, RepositoryContextChange,
    WorkRepositoryList, WorkSummary,
};
use crate::infrastructure::agent_files::RepoContextFileWriter;
use crate::infrastructure::git_worktree::{WorktreeInspector, WorktreeManager};
use crate::infrastructure::github::GithubClient;
use crate::infrastructure::storage::{RepoMetadataStore, RepositoryCache, WorkStore};
use crate::shared::error::{Result, WorkonError};

pub(super) struct RepositoryContextService<'a> {
    store: &'a WorkStore,
    github: &'a dyn GithubClient,
    cache: &'a dyn RepositoryCache,
    worktrees: &'a dyn WorktreeManager,
    inspector: &'a dyn WorktreeInspector,
    metadata: &'a dyn RepoMetadataStore,
    context_files: &'a dyn RepoContextFileWriter,
}

impl<'a> RepositoryContextService<'a> {
    pub(super) fn new(
        store: &'a WorkStore,
        github: &'a dyn GithubClient,
        cache: &'a dyn RepositoryCache,
        worktrees: &'a dyn WorktreeManager,
        inspector: &'a dyn WorktreeInspector,
        metadata: &'a dyn RepoMetadataStore,
        context_files: &'a dyn RepoContextFileWriter,
    ) -> Self {
        Self {
            store,
            github,
            cache,
            worktrees,
            inspector,
            metadata,
            context_files,
        }
    }

    pub(super) fn catalog(github: &dyn GithubClient) -> Result<CommandOutput> {
        Ok(CommandOutput::RepositoryCatalog(RepositoryCatalog {
            repositories: github.list_repositories()?,
        }))
    }

    pub(super) fn attached(
        store: &WorkStore,
        metadata: &dyn RepoMetadataStore,
        inspector: &dyn WorktreeInspector,
        query: &str,
    ) -> Result<CommandOutput> {
        let work = store.open(query)?;
        let metadata = metadata.read(&work.path)?;
        let repositories = inspector.scan(&work.path, &metadata)?;
        Ok(CommandOutput::WorkRepositories(WorkRepositoryList {
            work: work.into(),
            repositories,
        }))
    }

    pub(super) fn add(&self, query: &str, repositories: &[String]) -> Result<CommandOutput> {
        let work = self.store.open(query)?;
        let work_summary = WorkSummary::from(work.clone());
        let requested = normalize_requested_repositories(repositories)?;
        let mut metadata = self.metadata.read(&work.path)?;
        let mut attached = self.inspector.scan(&work.path, &metadata)?;
        let mut changed = Vec::new();

        for name_with_owner in requested {
            if attached
                .iter()
                .any(|repo| repo.name_with_owner == name_with_owner)
            {
                continue;
            }

            let available = self.github.fetch_repository(&name_with_owner)?;
            let cache_path = self.cache.ensure(&available)?;
            let branch = work_branch(&work);
            let path = worktree_path(&work, &available.name_with_owner)?;

            self.worktrees
                .switch(&cache_path, &path, &branch, &available.default_branch)?;

            let attachment = RepositoryAttachment {
                name_with_owner: available.name_with_owner,
                default_branch: available.default_branch,
                url: available.url,
            };
            let mut next_metadata = metadata.clone();
            next_metadata.retain(|repo| repo.name_with_owner != attachment.name_with_owner);
            next_metadata.push(attachment.clone());
            next_metadata.sort_by(|left, right| left.name_with_owner.cmp(&right.name_with_owner));
            let next_attached = self.inspector.scan(&work.path, &next_metadata)?;
            let repository = next_attached
                .iter()
                .find(|repo| repo.name_with_owner == attachment.name_with_owner)
                .cloned()
                .ok_or_else(|| WorkonError::RepositoryContext {
                    message: format!(
                        "repository worktree was created but could not be inspected: {}",
                        attachment.name_with_owner
                    ),
                })?;
            self.persist(&work_summary, &next_metadata, &next_attached)?;
            metadata = next_metadata;
            attached = next_attached;
            changed.push(repository);
        }

        Ok(CommandOutput::WorkRepositoriesAdded(
            RepositoryContextChange {
                work: work.into(),
                repositories: changed,
            },
        ))
    }

    pub(super) fn remove(
        &self,
        query: &str,
        repositories: &[String],
        force: bool,
    ) -> Result<CommandOutput> {
        let work = self.store.open(query)?;
        let work_summary = WorkSummary::from(work.clone());
        let requested = normalize_requested_repositories(repositories)?;
        let mut metadata = self.metadata.read(&work.path)?;
        let mut attached = self.inspector.scan(&work.path, &metadata)?;
        let mut removed = Vec::new();

        for name_with_owner in &requested {
            let Some(index) = attached
                .iter()
                .position(|repo| &repo.name_with_owner == name_with_owner)
            else {
                return Err(WorkonError::RepositoryContext {
                    message: format!(
                        "repository `{name_with_owner}` is not attached to work `{}`",
                        work.slug
                    ),
                });
            };
            let repository = attached[index].clone();
            let cache_path = self.cache.path_for(&repository.name_with_owner)?;
            self.worktrees
                .remove(&cache_path, &repository.path, force)?;
            let mut next_metadata = metadata.clone();
            next_metadata.retain(|repo| &repo.name_with_owner != name_with_owner);
            let next_attached = self.inspector.scan(&work.path, &next_metadata)?;
            self.persist(&work_summary, &next_metadata, &next_attached)?;
            metadata = next_metadata;
            attached = next_attached;
            removed.push(repository);
        }

        Ok(CommandOutput::WorkRepositoriesRemoved(
            RepositoryContextChange {
                work: work.into(),
                repositories: removed,
            },
        ))
    }

    fn persist(
        &self,
        work: &WorkSummary,
        metadata: &[RepositoryAttachment],
        repositories: &[AttachedRepository],
    ) -> Result<()> {
        self.metadata.write(&work.path, metadata)?;
        self.context_files.rewrite(work, repositories)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::fs;
    use std::path::{Path, PathBuf};

    use crate::application::CommandOutput;
    use crate::domain::{
        AttachedRepository, AvailableRepository, RepositoryAttachment, WorkSummary,
    };
    use crate::infrastructure::agent_files::RepoContextFileWriter;
    use crate::infrastructure::git_worktree::{WorktreeInspector, WorktreeManager};
    use crate::infrastructure::github::GithubClient;
    use crate::infrastructure::storage::{RepoMetadataStore, RepositoryCache, WorkStore};
    use crate::shared::error::{Result, WorkonError};

    use super::RepositoryContextService;

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
        let metadata = FakeMetadata::default();
        let context_files = FakeContextFiles::default();
        let service = RepositoryContextService::new(
            &store,
            &github,
            &cache,
            &worktrees,
            &inspector,
            &metadata,
            &context_files,
        );

        let error = service
            .add(&work.slug, &["openai/workon".to_string()])
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
        let metadata = FakeMetadata::with_repositories(vec![repository_attachment()]);
        let context_files = FakeContextFiles::default();
        let service = RepositoryContextService::new(
            &store,
            &github,
            &cache,
            &worktrees,
            &inspector,
            &metadata,
            &context_files,
        );

        let error = service
            .remove(&work.slug, &["openai/workon".to_string()], false)
            .expect_err("stale metadata without a scanned worktree is not attached");

        assert!(error.to_string().contains("is not attached"));
        assert_eq!(worktrees.remove_count(), 0);
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
        let metadata = FakeMetadata::default();
        let context_files = FakeContextFiles::default();
        let service = RepositoryContextService::new(
            &store,
            &github,
            &cache,
            &worktrees,
            &inspector,
            &metadata,
            &context_files,
        );

        let CommandOutput::WorkRepositoriesRemoved(change) = service
            .remove(&work.slug, &["openai/workon".to_string()], false)
            .expect("scanned worktree without metadata should remove")
        else {
            panic!("expected WorkRepositoriesRemoved output");
        };

        assert_eq!(change.repositories, vec![attached]);
        assert_eq!(worktrees.remove_count(), 1);
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
        removes: RefCell<Vec<PathBuf>>,
    }

    impl FakeWorktrees {
        fn switch_count(&self) -> usize {
            self.switches.borrow().len()
        }

        fn remove_count(&self) -> usize {
            self.removes.borrow().len()
        }
    }

    impl WorktreeManager for FakeWorktrees {
        fn switch(
            &self,
            _cache_path: &Path,
            worktree_path: &Path,
            _branch: &str,
            _default_branch: &str,
        ) -> Result<()> {
            self.switches.borrow_mut().push(worktree_path.to_path_buf());
            Ok(())
        }

        fn remove(&self, _cache_path: &Path, worktree_path: &Path, _force: bool) -> Result<()> {
            self.removes.borrow_mut().push(worktree_path.to_path_buf());
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
