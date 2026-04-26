use crate::domain::{AttachedRepository, RepositoryAttachment, WorkSummary};
use crate::infrastructure::agent_files::RepoContextFileWriter;
use crate::infrastructure::git_worktree::{RepositoryLinker, WorktreeInspector};
use crate::infrastructure::storage::RepoMetadataStore;
use crate::shared::error::{Result, WorkonError};

pub(super) struct RepositoryContextEditor<'a> {
    work: WorkSummary,
    metadata: Vec<RepositoryAttachment>,
    attached: Vec<AttachedRepository>,
    inspector: &'a dyn WorktreeInspector,
    metadata_store: &'a dyn RepoMetadataStore,
    context_files: &'a dyn RepoContextFileWriter,
}

impl<'a> RepositoryContextEditor<'a> {
    pub(super) fn new(
        work: WorkSummary,
        metadata: Vec<RepositoryAttachment>,
        attached: Vec<AttachedRepository>,
        inspector: &'a dyn WorktreeInspector,
        metadata_store: &'a dyn RepoMetadataStore,
        context_files: &'a dyn RepoContextFileWriter,
    ) -> Self {
        Self {
            work,
            metadata,
            attached,
            inspector,
            metadata_store,
            context_files,
        }
    }

    pub(super) fn is_attached(&self, name_with_owner: &str) -> bool {
        self.attached
            .iter()
            .any(|repo| repo.name_with_owner == name_with_owner)
    }

    pub(super) fn add(
        &mut self,
        attachment: RepositoryAttachment,
        missing_after_scan_message: &str,
    ) -> Result<AttachedRepository> {
        let mut next_metadata = self.metadata.clone();
        next_metadata.retain(|repo| repo.name_with_owner != attachment.name_with_owner);
        next_metadata.push(attachment.clone());
        next_metadata.sort_by(|left, right| left.name_with_owner.cmp(&right.name_with_owner));

        let next_attached = self.inspector.scan(&self.work.path, &next_metadata)?;
        let repository = next_attached
            .iter()
            .find(|repo| repo.name_with_owner == attachment.name_with_owner)
            .cloned()
            .ok_or_else(|| WorkonError::RepositoryContext {
                message: format!(
                    "{missing_after_scan_message}: {}",
                    attachment.name_with_owner
                ),
            })?;

        self.persist(&next_metadata, &next_attached)?;
        self.metadata = next_metadata;
        self.attached = next_attached;
        Ok(repository)
    }

    pub(super) fn remove(
        &mut self,
        name_with_owner: &str,
        linker: &dyn RepositoryLinker,
    ) -> Result<AttachedRepository> {
        let Some(index) = self
            .attached
            .iter()
            .position(|repo| repo.name_with_owner == name_with_owner)
        else {
            return Err(WorkonError::RepositoryContext {
                message: format!(
                    "repository `{name_with_owner}` is not attached to work `{}`",
                    self.work.slug
                ),
            });
        };

        let repository = self.attached[index].clone();
        linker.remove(&repository.path)?;

        let mut next_metadata = self.metadata.clone();
        next_metadata.retain(|repo| repo.name_with_owner != name_with_owner);
        let next_attached = self.inspector.scan(&self.work.path, &next_metadata)?;
        self.persist(&next_metadata, &next_attached)?;
        self.metadata = next_metadata;
        self.attached = next_attached;
        Ok(repository)
    }

    fn persist(
        &self,
        metadata: &[RepositoryAttachment],
        repositories: &[AttachedRepository],
    ) -> Result<()> {
        self.metadata_store.write(&self.work.path, metadata)?;
        self.context_files.rewrite(&self.work, repositories)
    }
}
