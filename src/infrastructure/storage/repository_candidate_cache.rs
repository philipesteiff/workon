use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

use crate::domain::RepositoryCandidate;
use crate::shared::error::{Result, WorkonError};

const REPOSITORY_CANDIDATE_CACHE_FILE: &str = "repo-candidates.json";

#[derive(Debug, Clone)]
pub(crate) struct JsonRepositoryCandidateCache {
    root: PathBuf,
}

impl JsonRepositoryCandidateCache {
    pub(crate) fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
        }
    }

    pub(crate) fn valid_candidate(&self, path: &Path) -> Result<Option<RepositoryCandidate>> {
        let path = canonical_or_owned(path);
        let Some(fingerprint) = candidate_path_fingerprint(&path)? else {
            return Ok(None);
        };
        Ok(self
            .read()?
            .into_iter()
            .find(|entry| entry.path == path && entry.fingerprint == fingerprint)
            .map(|entry| entry.candidate))
    }

    pub(crate) fn store(&self, candidate: &RepositoryCandidate) -> Result<()> {
        let path = canonical_or_owned(&candidate.path);
        let Some(fingerprint) = candidate_path_fingerprint(&path)? else {
            return Ok(());
        };
        let mut entries = self.read()?;
        entries.retain(|entry| entry.path != path);
        entries.push(CachedRepositoryCandidate {
            path,
            fingerprint,
            candidate: candidate.clone(),
        });
        entries.sort_by(|left, right| left.path.cmp(&right.path));
        self.write(&entries)
    }

    pub(crate) fn remove(&self, path: &Path) -> Result<()> {
        let path = canonical_or_owned(path);
        let mut entries = self.read()?;
        let original_len = entries.len();
        entries.retain(|entry| entry.path != path);
        if entries.len() == original_len {
            return Ok(());
        }
        self.write(&entries)
    }

    fn read(&self) -> Result<Vec<CachedRepositoryCandidate>> {
        let path = self.path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&content).unwrap_or_default())
    }

    fn write(&self, entries: &[CachedRepositoryCandidate]) -> Result<()> {
        let path = self.path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(entries).map_err(|error| {
            WorkonError::RepositoryContext {
                message: format!("could not write repository candidate cache: {error}"),
            }
        })?;
        fs::write(path, format!("{content}\n"))?;
        Ok(())
    }

    fn path(&self) -> PathBuf {
        self.root
            .join(".workon")
            .join(REPOSITORY_CANDIDATE_CACHE_FILE)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CachedRepositoryCandidate {
    path: PathBuf,
    fingerprint: CandidatePathFingerprint,
    candidate: RepositoryCandidate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CandidatePathFingerprint {
    modified_nanos: u128,
    len: u64,
}

fn candidate_path_fingerprint(path: &Path) -> Result<Option<CandidatePathFingerprint>> {
    let git_path = path.join(".git");
    let metadata_path = if git_path.exists() {
        git_path
    } else {
        path.into()
    };
    let metadata = match fs::metadata(&metadata_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let modified = metadata
        .modified()?
        .duration_since(UNIX_EPOCH)
        .map_err(|error| WorkonError::RepositoryContext {
            message: format!(
                "repository candidate path has unsupported timestamp {}: {error}",
                metadata_path.display()
            ),
        })?;
    Ok(Some(CandidatePathFingerprint {
        modified_nanos: modified.as_nanos(),
        len: metadata.len(),
    }))
}

fn canonical_or_owned(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
