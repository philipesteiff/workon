use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use crate::domain::{CreatedWork, OpenedWork, WorkList, WorkSummary};
use crate::error::{Result, WorkonError};
use crate::slug::{slugify, title_from_goal};

const META_FILE: &str = "workon.meta";

#[derive(Debug, Clone)]
pub struct WorkStore {
    root: PathBuf,
}

impl WorkStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn create(&self, goal: &str, intent_id: &str) -> Result<CreatedWork> {
        let title = title_from_goal(goal);
        let slug = self.unique_slug(&slugify(&title))?;
        let path = self.work_root().join(&slug);

        fs::create_dir_all(&path)?;

        let work = CreatedWork {
            title,
            slug,
            goal: goal.to_string(),
            intent_id: intent_id.to_string(),
            path,
        };

        fs::write(work.path.join(META_FILE), serialize_work(&work))?;

        Ok(work)
    }

    pub fn list(&self) -> Result<WorkList> {
        let work_root = self.work_root();
        if !work_root.exists() {
            return Ok(WorkList { works: Vec::new() });
        }

        let mut works = Vec::new();
        for entry in fs::read_dir(work_root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }

            let meta_path = entry.path().join(META_FILE);
            if !meta_path.is_file() {
                continue;
            }

            let content = fs::read_to_string(meta_path)?;
            works.push(parse_work(&content, entry.path())?);
        }

        works.sort_by(|left, right| left.title.cmp(&right.title));

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
                matches: matches.into_iter().map(|work| work.title).collect(),
            }),
        }
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

        while work_root.join(&candidate).exists() {
            candidate = format!("{base_slug}-{suffix}");
            suffix += 1;
        }

        Ok(candidate)
    }

    fn work_root(&self) -> PathBuf {
        self.root.join(".workon").join("work")
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
