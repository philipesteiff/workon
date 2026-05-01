use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

use crate::domain::RepositoryCandidate;
use crate::shared::error::{Result, WorkonError};

const REPOSITORY_CANDIDATE_CACHE_FILE: &str = "repo-candidates.json";
static CACHE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

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
        let _guard = cache_lock()?;
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
        let _guard = cache_lock()?;
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
        let _guard = cache_lock()?;
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
        let temp_path = path.with_extension("json.tmp");
        fs::write(&temp_path, format!("{content}\n"))?;
        fs::rename(temp_path, path)?;
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
    entries: Vec<CandidatePathFingerprintEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CandidatePathFingerprintEntry {
    path: PathBuf,
    modified_nanos: u128,
    len: u64,
}

fn candidate_path_fingerprint(path: &Path) -> Result<Option<CandidatePathFingerprint>> {
    let Some(paths) = git_metadata_paths(path)? else {
        return Ok(None);
    };
    let entries = paths
        .into_iter()
        .filter_map(|path| metadata_fingerprint_entry(path).transpose())
        .collect::<Result<Vec<_>>>()?;
    if entries.is_empty() {
        return Ok(None);
    }
    Ok(Some(CandidatePathFingerprint { entries }))
}

fn git_metadata_paths(path: &Path) -> Result<Option<Vec<PathBuf>>> {
    let git_path = path.join(".git");
    if !git_path.exists() {
        return Ok(None);
    }

    let mut paths = vec![git_path.clone()];
    if git_path.is_dir() {
        paths.push(git_path.join("HEAD"));
        paths.push(git_path.join("config"));
    } else if git_path.is_file() {
        if let Some(git_dir) = git_dir_from_file(&git_path)? {
            paths.push(git_dir.clone());
            paths.push(git_dir.join("HEAD"));
            paths.push(git_dir.join("config"));
            if let Some(common_dir) = common_git_dir(&git_dir)? {
                paths.push(common_dir.clone());
                paths.push(common_dir.join("config"));
            }
        }
    }
    paths.sort();
    paths.dedup();
    Ok(Some(paths))
}

fn git_dir_from_file(git_path: &Path) -> Result<Option<PathBuf>> {
    let content = fs::read_to_string(git_path)?;
    let Some(raw_path) = content.strip_prefix("gitdir:").map(str::trim) else {
        return Ok(None);
    };
    Ok(Some(resolve_metadata_path(git_path.parent(), raw_path)))
}

fn common_git_dir(git_dir: &Path) -> Result<Option<PathBuf>> {
    let path = git_dir.join("commondir");
    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    Ok(Some(resolve_metadata_path(Some(git_dir), content.trim())))
}

fn resolve_metadata_path(parent: Option<&Path>, raw_path: &str) -> PathBuf {
    let path = PathBuf::from(raw_path);
    if path.is_absolute() {
        path
    } else {
        parent.unwrap_or_else(|| Path::new("")).join(path)
    }
}

fn metadata_fingerprint_entry(path: PathBuf) -> Result<Option<CandidatePathFingerprintEntry>> {
    let metadata = match fs::metadata(&path) {
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
                path.display()
            ),
        })?;
    Ok(Some(CandidatePathFingerprintEntry {
        path: canonical_or_owned(&path),
        modified_nanos: modified.as_nanos(),
        len: metadata.len(),
    }))
}

fn cache_lock() -> Result<MutexGuard<'static, ()>> {
    CACHE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| WorkonError::RepositoryContext {
            message: "repository candidate cache lock was poisoned".to_string(),
        })
}

fn canonical_or_owned(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
