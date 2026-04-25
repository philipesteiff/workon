use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;

use crate::domain::work::naming::{slugify, title_from_goal};
use crate::domain::{ArchivedWork, CreatedWork, OpenedWork, WorkList, WorkSummary};
use crate::infrastructure::filesystem::{FileSystem, StdFileSystem};
use crate::shared::error::{Result, WorkonError};

const META_FILE: &str = "workon.meta";

#[derive(Debug, Clone)]
pub struct WorkStore<F = StdFileSystem> {
    root: PathBuf,
    fs: F,
}

impl WorkStore<StdFileSystem> {
    pub fn new(root: PathBuf) -> Self {
        Self::with_filesystem(root, StdFileSystem)
    }
}

impl<F: FileSystem> WorkStore<F> {
    pub fn with_filesystem(root: PathBuf, fs: F) -> Self {
        Self { root, fs }
    }

    pub fn create(&self, goal: &str, intent_id: &str) -> Result<CreatedWork> {
        let title = title_from_goal(goal);
        let slug = self.unique_slug(&slugify(&title))?;
        let path = self.work_root().join(&slug);

        self.fs.create_dir_all(&path)?;

        let work = CreatedWork {
            title,
            slug,
            goal: goal.to_string(),
            intent_id: intent_id.to_string(),
            path,
        };

        self.fs
            .write(&work.path.join(META_FILE), &serialize_work(&work))?;

        Ok(work)
    }

    pub fn list(&self) -> Result<WorkList> {
        let work_root = self.work_root();
        if !self.fs.exists(&work_root) {
            return Ok(WorkList { works: Vec::new() });
        }

        let mut works = Vec::new();
        for path in self.fs.read_dir(&work_root)? {
            if !self.fs.is_dir(&path) {
                continue;
            }

            let meta_path = path.join(META_FILE);
            if !self.fs.is_file(&meta_path) {
                continue;
            }

            let content = self.fs.read_to_string(&meta_path)?;
            works.push(parse_work(&content, path)?);
        }

        works.sort_by(|left, right| {
            left.title
                .cmp(&right.title)
                .then(left.slug.cmp(&right.slug))
        });

        Ok(WorkList { works })
    }

    pub fn open(&self, query: &str) -> Result<OpenedWork> {
        let query = query.trim();
        let query_slug = slugify(query);
        let query_lower = query.to_ascii_lowercase();
        let list = self.list()?;

        let exact_matches: Vec<WorkSummary> = list
            .works
            .iter()
            .filter(|work| work.slug == query_slug || work.title.eq_ignore_ascii_case(query))
            .cloned()
            .collect();

        if exact_matches.len() == 1 {
            return Ok(exact_matches[0].clone().into());
        }

        let matches: Vec<WorkSummary> = list
            .works
            .into_iter()
            .filter(|work| {
                work.slug.contains(&query_slug)
                    || work.title.to_ascii_lowercase().contains(&query_lower)
            })
            .collect();

        match matches.len() {
            0 => Err(WorkonError::WorkNotFound {
                query: query.to_string(),
            }),
            1 => Ok(matches[0].clone().into()),
            _ => Err(WorkonError::AmbiguousWork {
                query: query.to_string(),
                matches: matches.into_iter().map(Into::into).collect(),
            }),
        }
    }

    pub fn archive(&self, query: &str) -> Result<ArchivedWork> {
        let work = self.open(query)?;
        let archive_root = self.archive_root();
        self.fs.create_dir_all(&archive_root)?;

        let archive_path = self.unique_archive_path(&work.slug);
        self.fs.rename(&work.path, &archive_path)?;

        Ok(ArchivedWork {
            title: work.title,
            slug: work.slug,
            goal: work.goal,
            intent_id: work.intent_id,
            path: work.path,
            archive_path,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn unique_slug(&self, base_slug: &str) -> Result<String> {
        let base_slug = if base_slug.is_empty() {
            "work"
        } else {
            base_slug
        };
        let work_root = self.work_root();
        let mut candidate = base_slug.to_string();
        let mut suffix = 2;

        while self.fs.exists(&work_root.join(&candidate)) {
            candidate = format!("{base_slug}-{suffix}");
            suffix += 1;
        }

        Ok(candidate)
    }

    fn work_root(&self) -> PathBuf {
        self.root.join(".workon").join("work")
    }

    fn archive_root(&self) -> PathBuf {
        self.root.join(".workon").join("archive")
    }

    fn unique_archive_path(&self, slug: &str) -> PathBuf {
        let archive_root = self.archive_root();
        let mut candidate = archive_root.join(slug);
        let mut suffix = 2;

        while self.fs.exists(&candidate) {
            candidate = archive_root.join(format!("{slug}-{suffix}"));
            suffix += 1;
        }

        candidate
    }
}

fn serialize_work(work: &CreatedWork) -> String {
    format!(
        "title={}\nslug={}\ngoal={}\nintent_id={}\n",
        encode(&work.title),
        encode(&work.slug),
        encode(&work.goal),
        encode(&work.intent_id)
    )
}

fn parse_work(content: &str, path: PathBuf) -> Result<WorkSummary> {
    let mut values = BTreeMap::new();
    for line in content.lines() {
        if let Some((key, value)) = line.split_once('=') {
            values.insert(key.to_string(), decode(value));
        }
    }

    Ok(WorkSummary {
        title: value_or_empty(&values, "title"),
        slug: value_or_empty(&values, "slug"),
        goal: value_or_empty(&values, "goal"),
        intent_id: value_or_empty(&values, "intent_id"),
        path,
    })
}

fn value_or_empty(values: &BTreeMap<String, String>, key: &str) -> String {
    values.get(key).cloned().unwrap_or_default()
}

fn encode(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
}

fn decode(value: &str) -> String {
    let mut decoded = String::new();
    let mut chars = value.chars();

    while let Some(character) = chars.next() {
        if character != '\\' {
            decoded.push(character);
            continue;
        }

        match chars.next() {
            Some('n') => decoded.push('\n'),
            Some('t') => decoded.push('\t'),
            Some('\\') => decoded.push('\\'),
            Some(other) => {
                decoded.push('\\');
                decoded.push(other);
            }
            None => decoded.push('\\'),
        }
    }

    decoded
}

#[cfg(test)]
mod tests {
    use super::WorkStore;
    use crate::infrastructure::filesystem::FileSystem;
    use std::cell::RefCell;
    use std::io;
    use std::path::{Path, PathBuf};
    use std::rc::Rc;

    #[derive(Clone, Default)]
    struct RecordingFileSystem {
        created_dirs: Rc<RefCell<Vec<PathBuf>>>,
    }

    impl FileSystem for RecordingFileSystem {
        fn create_dir_all(&self, path: &Path) -> io::Result<()> {
            self.created_dirs.borrow_mut().push(path.to_path_buf());
            Ok(())
        }

        fn rename(&self, _from: &Path, _to: &Path) -> io::Result<()> {
            Ok(())
        }

        fn write(&self, _path: &Path, _content: &str) -> io::Result<()> {
            Ok(())
        }

        fn read_to_string(&self, _path: &Path) -> io::Result<String> {
            Ok(String::new())
        }

        fn read_dir(&self, _path: &Path) -> io::Result<Vec<PathBuf>> {
            Ok(Vec::new())
        }

        fn exists(&self, _path: &Path) -> bool {
            false
        }

        fn is_dir(&self, _path: &Path) -> bool {
            false
        }

        fn is_file(&self, _path: &Path) -> bool {
            false
        }
    }

    #[test]
    fn store_can_use_injected_filesystem() {
        let fs = RecordingFileSystem::default();
        let store = WorkStore::with_filesystem(PathBuf::from("/tmp/workon"), fs.clone());

        let work = store
            .create("Answer billing question", "investigate")
            .expect("create should use injected filesystem");

        assert_eq!(work.slug, "answer-billing-question");
        assert_eq!(
            fs.created_dirs.borrow().as_slice(),
            &[PathBuf::from(
                "/tmp/workon/.workon/work/answer-billing-question"
            )]
        );
    }
}
