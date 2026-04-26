use crate::application::CommandOutput;
use std::path::{Path, PathBuf};

use crate::domain::repository_context::paths::{
    normalize_requested_repositories, repository_alias, work_branch, workspace_worktree_path,
};
use crate::domain::{
    AttachedRepository, RepositoryAttachment, RepositoryCandidateList, RepositoryCatalog,
    RepositoryContextChange, RepositoryWorkspace, RepositoryWorkspaceList, WorkRepositoryList,
    WorkSummary,
};
use crate::infrastructure::agent_files::RepoContextFileWriter;
use crate::infrastructure::git_worktree::{RepositoryLinker, WorktreeInspector, WorktreeManager};
use crate::infrastructure::github::GithubClient;
use crate::infrastructure::storage::{
    RepoMetadataStore, RepoWorkspaceStore, RepositoryCache, WorkStore,
};
use crate::shared::error::{Result, WorkonError};

pub(super) struct RepositoryContextService<'a> {
    store: &'a WorkStore,
    github: &'a dyn GithubClient,
    cache: &'a dyn RepositoryCache,
    worktrees: &'a dyn WorktreeManager,
    inspector: &'a dyn WorktreeInspector,
    linker: &'a dyn RepositoryLinker,
    metadata: &'a dyn RepoMetadataStore,
    workspaces: &'a dyn RepoWorkspaceStore,
    context_files: &'a dyn RepoContextFileWriter,
}

pub(super) struct RepositoryContextDeps<'a> {
    pub(super) store: &'a WorkStore,
    pub(super) github: &'a dyn GithubClient,
    pub(super) cache: &'a dyn RepositoryCache,
    pub(super) worktrees: &'a dyn WorktreeManager,
    pub(super) inspector: &'a dyn WorktreeInspector,
    pub(super) linker: &'a dyn RepositoryLinker,
    pub(super) metadata: &'a dyn RepoMetadataStore,
    pub(super) workspaces: &'a dyn RepoWorkspaceStore,
    pub(super) context_files: &'a dyn RepoContextFileWriter,
}

impl<'a> RepositoryContextService<'a> {
    pub(super) fn new(deps: RepositoryContextDeps<'a>) -> Self {
        Self {
            store: deps.store,
            github: deps.github,
            cache: deps.cache,
            worktrees: deps.worktrees,
            inspector: deps.inspector,
            linker: deps.linker,
            metadata: deps.metadata,
            workspaces: deps.workspaces,
            context_files: deps.context_files,
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

    pub(super) fn workspaces(workspaces: &dyn RepoWorkspaceStore) -> Result<CommandOutput> {
        Ok(CommandOutput::RepositoryWorkspaces(
            RepositoryWorkspaceList {
                workspaces: workspaces.read()?,
            },
        ))
    }

    pub(super) fn add_workspaces(
        workspaces: &dyn RepoWorkspaceStore,
        paths: &[PathBuf],
    ) -> Result<CommandOutput> {
        if paths.is_empty() {
            return Err(WorkonError::MissingArgument {
                message: "repos workspace add requires at least one path".to_string(),
            });
        }

        let mut existing = workspaces.read()?;
        for path in paths {
            let workspace = normalize_workspace_path(path)?;
            if !existing.iter().any(|item| item.path == workspace.path) {
                existing.push(workspace);
            }
        }
        existing.sort_by(|left, right| left.path.cmp(&right.path));
        existing.dedup_by(|left, right| left.path == right.path);
        workspaces.write(&existing)?;
        Self::workspaces(workspaces)
    }

    pub(super) fn remove_workspace(
        workspaces: &dyn RepoWorkspaceStore,
        path: &Path,
    ) -> Result<CommandOutput> {
        let path = normalize_existing_or_input_path(path)?;
        let mut existing = workspaces.read()?;
        existing.retain(|workspace| workspace.path != path);
        workspaces.write(&existing)?;
        Self::workspaces(workspaces)
    }

    pub(super) fn discover(
        store: &WorkStore,
        workspaces: &dyn RepoWorkspaceStore,
        inspector: &dyn WorktreeInspector,
        query: &str,
    ) -> Result<CommandOutput> {
        let work = store.open(query)?;
        let roots = workspaces
            .read()?
            .into_iter()
            .map(|workspace| workspace.path)
            .collect::<Vec<_>>();
        Ok(CommandOutput::RepositoryCandidates(
            RepositoryCandidateList {
                work: work.into(),
                candidates: inspector.discover(&roots)?,
            },
        ))
    }

    pub(super) fn add(
        &self,
        query: &str,
        repositories: &[String],
        workspace: Option<&Path>,
    ) -> Result<CommandOutput> {
        let work = self.store.open(query)?;
        let work_summary = WorkSummary::from(work.clone());
        let requested = normalize_requested_repositories(repositories)?;
        let mut metadata = self.metadata.read(&work.path)?;
        let mut attached = self.inspector.scan(&work.path, &metadata)?;
        let mut changed = Vec::new();
        let workspace = self.select_workspace(workspace)?;

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
            let target_path =
                workspace_worktree_path(&workspace.path, &work.slug, &available.name_with_owner)?;

            self.worktrees.switch(
                &cache_path,
                &target_path,
                &branch,
                &available.default_branch,
            )?;
            let alias = repository_alias(&available.name_with_owner)?;
            let link_path =
                self.linker
                    .link(&work.path, &alias, &available.name_with_owner, &target_path)?;

            let attachment = RepositoryAttachment {
                name_with_owner: available.name_with_owner,
                default_branch: available.default_branch,
                url: available.url,
                alias: link_alias(&link_path)?,
                target_path: fs_canonicalize(&target_path)?,
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

    pub(super) fn link(&self, query: &str, paths: &[PathBuf]) -> Result<CommandOutput> {
        let work = self.store.open(query)?;
        let work_summary = WorkSummary::from(work.clone());
        let mut metadata = self.metadata.read(&work.path)?;
        let mut attached = self.inspector.scan(&work.path, &metadata)?;
        let mut changed = Vec::new();

        for path in paths {
            let target_path = fs_canonicalize(path)?;
            let Some(candidate) = self.inspector.inspect_candidate(&target_path)? else {
                return Err(WorkonError::RepositoryContext {
                    message: format!("not a Git working tree: {}", path.display()),
                });
            };
            if attached
                .iter()
                .any(|repo| repo.name_with_owner == candidate.name_with_owner)
            {
                continue;
            }

            let alias = repository_alias(&candidate.name_with_owner)?;
            let link_path =
                self.linker
                    .link(&work.path, &alias, &candidate.name_with_owner, &target_path)?;
            let attachment = RepositoryAttachment {
                name_with_owner: candidate.name_with_owner,
                default_branch: String::new(),
                url: candidate.url,
                alias: link_alias(&link_path)?,
                target_path,
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
                        "repository was linked but could not be inspected: {}",
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
        _force: bool,
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
            self.linker.remove(&repository.path)?;
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

    fn select_workspace(&self, selected: Option<&Path>) -> Result<RepositoryWorkspace> {
        let workspaces = self.workspaces.read()?;
        if let Some(selected) = selected {
            let selected = normalize_existing_or_input_path(selected)?;
            return workspaces
                .into_iter()
                .find(|workspace| workspace.path == selected)
                .ok_or_else(|| WorkonError::RepositoryContext {
                    message: format!(
                        "repository workspace is not configured: {}",
                        selected.display()
                    ),
                });
        }

        match workspaces.as_slice() {
            [workspace] => Ok(workspace.clone()),
            [] => Err(WorkonError::RepositoryContext {
                message: "no repository workspace configured. Add one with: wo repos workspace add <path>...".to_string(),
            }),
            _ => Err(WorkonError::RepositoryContext {
                message:
                    "multiple repository workspaces configured. Choose one with --workspace <path>"
                        .to_string(),
            }),
        }
    }
}

fn normalize_workspace_path(path: &Path) -> Result<RepositoryWorkspace> {
    let path = expand_home_path(path)?;
    std::fs::create_dir_all(&path)?;
    Ok(RepositoryWorkspace {
        path: fs_canonicalize(&path)?,
    })
}

fn normalize_existing_or_input_path(path: &Path) -> Result<PathBuf> {
    let path = expand_home_path(path)?;
    if path.exists() {
        fs_canonicalize(&path)
    } else {
        Ok(path)
    }
}

fn fs_canonicalize(path: &Path) -> Result<PathBuf> {
    std::fs::canonicalize(path).map_err(Into::into)
}

fn expand_home_path(path: &Path) -> Result<PathBuf> {
    let Some(value) = path.to_str() else {
        return Ok(path.to_path_buf());
    };
    if value == "~" {
        return home_dir();
    }
    let Some(rest) = value.strip_prefix("~/") else {
        return Ok(path.to_path_buf());
    };
    Ok(home_dir()?.join(rest))
}

fn home_dir() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| WorkonError::RepositoryContext {
            message: "HOME is required to expand repo workspace paths".to_string(),
        })
}

fn link_alias(path: &Path) -> Result<String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(ToString::to_string)
        .ok_or_else(|| WorkonError::RepositoryContext {
            message: format!("repository link has no valid alias: {}", path.display()),
        })
}

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

    use super::{RepositoryContextDeps, RepositoryContextService};

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
            _cache_path: &Path,
            worktree_path: &Path,
            _branch: &str,
            _default_branch: &str,
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
