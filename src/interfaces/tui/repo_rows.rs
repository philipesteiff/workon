use std::path::{Path, PathBuf};

use ratatui::prelude::{Line, Span};
use ratatui::style::Style;

use crate::domain::repository_context::paths::workspace_worktree_path;

use super::repo_state::{RepoCatalogRow, RepoPane, RepoPickerState};
use super::theme;

pub(super) fn catalog_header_row(width: u16) -> Line<'static> {
    let columns = repo_columns(usize::from(width));
    let mut spans = vec![
        Span::styled("  ".to_string(), theme::style_command()),
        Span::raw(" "),
        Span::styled("   ".to_string(), theme::style_muted_text()),
        Span::raw(" "),
    ];
    push_column(
        &mut spans,
        "REPOSITORY",
        columns.name,
        theme::style_muted_text(),
        Truncate::End,
    );
    push_column(
        &mut spans,
        "SOURCE",
        columns.source,
        theme::style_muted_text(),
        Truncate::End,
    );
    push_column(
        &mut spans,
        "BRANCH",
        columns.branch,
        theme::style_muted_text(),
        Truncate::End,
    );
    push_column(
        &mut spans,
        "STATE",
        columns.status,
        theme::style_muted_text(),
        Truncate::End,
    );
    Line::from(spans)
}

pub(super) fn catalog_row(
    row: &RepoCatalogRow,
    repo: &RepoPickerState,
    index: usize,
    width: u16,
) -> Line<'static> {
    match row {
        RepoCatalogRow::GitHub(repository) => catalog_repo_row(
            RepoRowRenderData {
                name: repository.name_with_owner.clone(),
                source: "github",
                branch: repository.default_branch.clone(),
                attached_to_work: repo.is_selected(&repository.name_with_owner),
                pending_add: repo.pending_add.contains(&repository.name_with_owner),
                pending_remove: repo.pending_remove.contains(&repository.name_with_owner),
                force_remove: repo.force_remove,
            },
            index == repo.selected_catalog && repo.focus == RepoPane::Catalog,
            width,
        ),
        RepoCatalogRow::Local(candidate) => catalog_repo_row(
            RepoRowRenderData {
                name: candidate.name_with_owner.clone(),
                source: "local",
                branch: candidate.branch.clone(),
                attached_to_work: repo.is_selected(&candidate.name_with_owner),
                pending_add: repo.is_local_candidate_selected(&candidate.path),
                pending_remove: repo.pending_remove.contains(&candidate.name_with_owner),
                force_remove: repo.force_remove,
            },
            index == repo.selected_catalog && repo.focus == RepoPane::Catalog,
            width,
        ),
        RepoCatalogRow::Attached(repository) => catalog_repo_row(
            RepoRowRenderData {
                name: repository.name_with_owner.clone(),
                source: "attached",
                branch: repository.branch.clone(),
                attached_to_work: repo.is_selected(&repository.name_with_owner),
                pending_add: false,
                pending_remove: repo.pending_remove.contains(&repository.name_with_owner),
                force_remove: repo.force_remove,
            },
            index == repo.selected_catalog && repo.focus == RepoPane::Catalog,
            width,
        ),
    }
}

pub(super) fn catalog_row_detail(
    row: &RepoCatalogRow,
    width: u16,
    workspace: Option<&Path>,
    work_slug: &str,
) -> Line<'static> {
    let detail = match row {
        RepoCatalogRow::GitHub(repository) => DetailFields {
            key: "worktree",
            value: workspace
                .map(|path| github_worktree_path(path, work_slug, &repository.name_with_owner))
                .unwrap_or_else(|| "workspace not set".to_string()),
            secondary_key: "base",
            secondary_value: workspace.map(compact_path),
        },
        RepoCatalogRow::Local(candidate) => DetailFields {
            key: "path",
            value: compact_path(&candidate.path),
            secondary_key: "work",
            secondary_value: Some(row_path(&candidate.path, workspace)),
        },
        RepoCatalogRow::Attached(repository) => DetailFields {
            key: "path",
            value: compact_path(&repository.path),
            secondary_key: "work",
            secondary_value: Some(row_path(&repository.path, workspace)),
        },
    };
    let width = usize::from(width);
    let indent = 7.min(width);
    let value_width = width.saturating_sub(indent);

    let mut spans = vec![
        Span::raw(" ".repeat(indent)),
        Span::styled(detail.key.to_string(), theme::style_command()),
        Span::raw(" "),
    ];
    match detail.secondary_value {
        Some(secondary_value) => {
            let fixed_width =
                detail.key.len() + " ".len() + " | ".len() + detail.secondary_key.len() + " ".len();
            let available = value_width.saturating_sub(fixed_width);
            let value_column = detail_value_width(&detail.value, available);
            let secondary_column = available.saturating_sub(value_column);
            spans.push(Span::styled(
                fit_padded(&detail.value, value_column, Truncate::Start),
                theme::style_muted_text(),
            ));
            spans.push(Span::raw(" | "));
            spans.push(Span::styled(
                detail.secondary_key.to_string(),
                theme::style_command(),
            ));
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                fit_text(&secondary_value, secondary_column, Truncate::Start),
                theme::style_muted_text(),
            ));
        }
        None => spans.push(Span::styled(
            fit_text(&detail.value, value_width, Truncate::Start),
            theme::style_muted_text(),
        )),
    }
    Line::from(spans)
}

fn github_worktree_path(workspace: &Path, work_slug: &str, name_with_owner: &str) -> String {
    workspace_worktree_path(workspace, work_slug, name_with_owner)
        .map(|path| compact_path(&path))
        .unwrap_or_else(|_| compact_path(workspace))
}

pub(super) fn compact_path(path: &Path) -> String {
    if let Some(home) = home_path() {
        if let Ok(relative) = path.strip_prefix(&home) {
            if relative.as_os_str().is_empty() {
                return "~".to_string();
            }
            return format!("~/{}", relative.display());
        }
    }

    path.display().to_string()
}

struct RepoRowRenderData {
    name: String,
    source: &'static str,
    branch: String,
    attached_to_work: bool,
    pending_add: bool,
    pending_remove: bool,
    force_remove: bool,
}

fn catalog_repo_row(data: RepoRowRenderData, selected: bool, width: u16) -> Line<'static> {
    let marker = if data.pending_remove && data.force_remove {
        "[!]"
    } else if data.pending_remove {
        "[-]"
    } else if data.pending_add || data.attached_to_work {
        "[x]"
    } else {
        "[ ]"
    };
    let status = if data.pending_add {
        "ADD"
    } else if data.pending_remove && data.force_remove {
        "FORCE"
    } else if data.pending_remove {
        "REMOVE"
    } else if data.attached_to_work && data.source != "attached" {
        "ATTACHED"
    } else {
        ""
    };
    let anchor = if selected { ">>" } else { "  " };
    let columns = repo_columns(usize::from(width));
    let mut spans = vec![
        Span::styled(anchor.to_string(), theme::style_command()),
        Span::raw(" "),
        Span::styled(marker.to_string(), theme::style_muted_text()),
        Span::raw(" "),
    ];
    push_column(
        &mut spans,
        &data.name,
        columns.name,
        theme::style_primary_text(),
        Truncate::End,
    );
    push_column(
        &mut spans,
        data.source,
        columns.source,
        theme::style_meta_value(),
        Truncate::End,
    );
    push_column(
        &mut spans,
        &data.branch,
        columns.branch,
        theme::style_meta_value(),
        Truncate::End,
    );
    push_column(
        &mut spans,
        status,
        columns.status,
        theme::style_command(),
        Truncate::End,
    );
    Line::from(spans)
}

struct DetailFields {
    key: &'static str,
    value: String,
    secondary_key: &'static str,
    secondary_value: Option<String>,
}

fn detail_value_width(value: &str, available: usize) -> usize {
    if available == 0 {
        return 0;
    }
    let natural = value.chars().count();
    let preferred = available.saturating_mul(3) / 5;
    natural.min(preferred).max(available.min(16))
}

#[derive(Clone, Copy)]
struct RepoColumns {
    name: usize,
    source: usize,
    branch: usize,
    status: usize,
}

#[derive(Clone, Copy)]
enum Truncate {
    Start,
    End,
}

fn repo_columns(width: usize) -> RepoColumns {
    let source = if width >= 52 { 8 } else { 0 };
    let branch = if width >= 120 {
        18
    } else if width >= 64 {
        14
    } else {
        0
    };
    let status = if width >= 104 { 8 } else { 0 };
    let shown_columns = [source, branch, status]
        .into_iter()
        .filter(|column| *column > 0)
        .count();
    let gaps = shown_columns;
    let fixed = 7 + source + branch + status + gaps;
    let name = width.saturating_sub(fixed).max(12);

    RepoColumns {
        name,
        source,
        branch,
        status,
    }
}

fn push_column(
    spans: &mut Vec<Span<'static>>,
    value: &str,
    width: usize,
    style: Style,
    truncation: Truncate,
) {
    if width == 0 {
        return;
    }
    if spans.len() > 4 {
        spans.push(Span::raw(" "));
    }
    spans.push(Span::styled(fit_padded(value, width, truncation), style));
}

fn fit_padded(value: &str, width: usize, truncation: Truncate) -> String {
    let fitted = fit_text(value, width, truncation);
    format!("{fitted:<width$}")
}

fn fit_text(value: &str, width: usize, truncation: Truncate) -> String {
    if width == 0 {
        return String::new();
    }
    if value.chars().count() <= width {
        return value.to_string();
    }
    if width <= 3 {
        return ".".repeat(width);
    }

    let keep = width - 3;
    match truncation {
        Truncate::Start => {
            let tail = value
                .chars()
                .rev()
                .take(keep)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<String>();
            format!("...{tail}")
        }
        Truncate::End => {
            let head = value.chars().take(keep).collect::<String>();
            format!("{head}...")
        }
    }
}

fn row_path(path: &Path, workspace: Option<&Path>) -> String {
    if let Some(workspace) = workspace {
        if let Ok(relative) = path.strip_prefix(workspace) {
            if relative.as_os_str().is_empty() {
                return ".".to_string();
            }
            return format!("./{}", relative.display());
        }
    }
    compact_path(path)
}

fn home_path() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{fit_text, repo_columns, row_path, Truncate};

    #[test]
    fn repo_columns_drop_optional_columns_by_width() {
        let compact = repo_columns(48);
        let medium = repo_columns(96);
        let wide = repo_columns(128);

        assert_eq!(compact.source, 0);
        assert_eq!(compact.branch, 0);
        assert_eq!(compact.status, 0);
        assert_eq!(medium.source, 8);
        assert_eq!(medium.branch, 14);
        assert_eq!(medium.status, 0);
        assert_eq!(wide.source, 8);
        assert_eq!(wide.branch, 18);
        assert_eq!(wide.status, 8);
    }

    #[test]
    fn fit_text_truncates_from_requested_edge() {
        assert_eq!(fit_text("openai/workon", 9, Truncate::End), "openai...");
        assert_eq!(fit_text("openai/workon", 9, Truncate::Start), "...workon");
    }

    #[test]
    fn row_path_prefers_workspace_relative_paths() {
        let workspace = Path::new("/tmp/repos");

        assert_eq!(
            row_path(Path::new("/tmp/repos/client/api"), Some(workspace)),
            "./client/api"
        );
    }
}
