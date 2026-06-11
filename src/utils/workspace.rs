use std::fs;
use std::path::{Path, PathBuf};

use crate::utils::error::{AppError, AppResult};
use crate::utils::index::{self, DocsIndex};

pub const UPPERCASE_DOCUMENTS_DIRECTORY: &str = "Docs";
pub const LOWERCASE_DOCUMENTS_DIRECTORY: &str = "docs";

#[derive(Debug, Clone)]
pub struct Workspace {
    pub root: PathBuf,
    pub index_file: PathBuf,
    pub agents_file: PathBuf,
    pub documents_directory: PathBuf,
    pub documents_directory_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitOutput {
    pub index_path: String,
    pub documents_directory: String,
    pub naming_style: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceRule {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceRules {
    pub repository: String,
    pub agents_file: String,
    pub index_file: String,
    pub documents_directory: String,
    pub rules: Vec<WorkspaceRule>,
}

impl WorkspaceRule {
    pub fn new(path: String, content: String) -> Self {
        Self { path, content }
    }
}

pub fn init() -> AppResult<InitOutput> {
    let current_dir = current_dir()?;
    let root = find_root_from(&current_dir).unwrap_or(current_dir);
    let index_path = index::index_path(&root);

    let index = if index_path.exists() {
        index::read_from_root(&root)?
    } else {
        let workspace_naming = infer_workspace_naming(&root)?;
        let mut index = DocsIndex::new(
            workspace_naming.documents_directory,
            workspace_naming.naming_style,
        );
        index::bootstrap_from_disk(&root, &mut index)?;
        index::write_to_root(&root, &index)?;
        index
    };

    let documents_directory = root.join(index::config_path_to_relative(&index.doc_dir)?);
    fs::create_dir_all(&documents_directory).map_err(|error| {
        AppError::fs(
            "create_documents_directory_failed",
            "failed to create documents directory",
            relative_path_from_root(&root, &documents_directory),
            &error,
        )
    })?;

    Ok(InitOutput {
        index_path: relative_path_from_root(&root, &index_path),
        documents_directory: index.doc_dir,
        naming_style: index.naming_style,
    })
}

pub fn find() -> AppResult<Workspace> {
    let current_dir = current_dir()?;

    let root = find_root_from(&current_dir)?;
    let index = index::read_from_root(&root)?;
    Ok(Workspace {
        agents_file: root.join(index::config_path_to_relative(&index.index_file)?),
        index_file: index::index_path(&root),
        documents_directory: root.join(index::config_path_to_relative(&index.doc_dir)?),
        documents_directory_name: index::config_path_to_display(&index.doc_dir)?,
        root,
    })
}

pub fn read_applicable_rules() -> AppResult<WorkspaceRules> {
    let workspace = find()?;
    let current_dir = std::env::current_dir().map_err(|error| {
        AppError::fs(
            "current_dir_failed",
            "failed to read current directory",
            ".",
            &error,
        )
    })?;

    let mut rules = Vec::new();
    let relative_current = current_dir
        .strip_prefix(&workspace.root)
        .unwrap_or(Path::new(""));

    let mut candidate = workspace.root.clone();
    collect_agents_file(&workspace, &candidate, &mut rules)?;
    for component in relative_current.components() {
        candidate.push(component.as_os_str());
        collect_agents_file(&workspace, &candidate, &mut rules)?;
    }

    Ok(WorkspaceRules {
        repository: display_path(&workspace.root),
        agents_file: relative_path(&workspace, &workspace.agents_file),
        index_file: relative_path(&workspace, &workspace.index_file),
        documents_directory: relative_path(&workspace, &workspace.documents_directory),
        rules,
    })
}

pub fn ensure_documents_directory(workspace: &Workspace) -> AppResult<()> {
    fs::create_dir_all(&workspace.documents_directory).map_err(|error| {
        AppError::fs(
            "create_docs_failed",
            "failed to create documents directory",
            relative_path(workspace, &workspace.documents_directory),
            &error,
        )
    })
}

pub fn relative_path(workspace: &Workspace, path: &Path) -> String {
    path.strip_prefix(&workspace.root)
        .unwrap_or(path)
        .iter()
        .map(|part| part.to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

pub fn display_path(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn find_root_from(current_dir: &Path) -> AppResult<PathBuf> {
    let mut candidate = Some(current_dir);
    while let Some(path) = candidate {
        if index::index_path(path).is_file() {
            return Ok(path.to_path_buf());
        }
        candidate = path.parent();
    }

    Err(AppError::input(
        "workspace_not_initialized",
        "failed to locate docs.json; run adm init first",
    ))
}

fn current_dir() -> AppResult<PathBuf> {
    std::env::current_dir().map_err(|error| {
        AppError::fs(
            "current_dir_failed",
            "failed to read current directory",
            ".",
            &error,
        )
    })
}

fn infer_workspace_naming(root: &Path) -> AppResult<WorkspaceNaming> {
    let naming_style = infer_most_common_child_directory_naming_style(root)?;
    let documents_directory = if index::naming_style_uses_uppercase_doc_dir(&naming_style) {
        format!("./{UPPERCASE_DOCUMENTS_DIRECTORY}")
    } else {
        format!("./{LOWERCASE_DOCUMENTS_DIRECTORY}")
    };

    Ok(WorkspaceNaming {
        documents_directory,
        naming_style,
    })
}

fn infer_most_common_child_directory_naming_style(root: &Path) -> AppResult<String> {
    let mut counts = Vec::<(&'static str, usize)>::new();
    let entries = fs::read_dir(root).map_err(|error| {
        AppError::fs(
            "read_workspace_failed",
            "failed to read current directory",
            display_path(root),
            &error,
        )
    })?;

    for entry in entries {
        let entry = entry.map_err(|error| {
            AppError::fs(
                "read_workspace_entry_failed",
                "failed to read current directory entry",
                display_path(root),
                &error,
            )
        })?;

        if !entry
            .file_type()
            .map(|file_type| file_type.is_dir())
            .unwrap_or(false)
        {
            continue;
        }

        let file_name = entry.file_name().to_string_lossy().to_string();
        if file_name.starts_with('.') {
            continue;
        }

        let naming_style = index::infer_naming_style_from_name(&file_name);
        if let Some((_, count)) = counts
            .iter_mut()
            .find(|(candidate, _)| *candidate == naming_style)
        {
            *count += 1;
        } else {
            counts.push((naming_style, 1));
        }
    }

    let Some(max_count) = counts.iter().map(|(_, count)| *count).max() else {
        return Ok(index::LOWERCASE_NAMING_STYLE.to_string());
    };

    let winners = counts
        .iter()
        .filter(|(_, count)| *count == max_count)
        .map(|(style, _)| *style)
        .collect::<Vec<_>>();

    if winners.len() == 1 {
        Ok(winners[0].to_string())
    } else {
        Ok(index::LOWERCASE_NAMING_STYLE.to_string())
    }
}

struct WorkspaceNaming {
    documents_directory: String,
    naming_style: String,
}

pub fn relative_path_from_root(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .iter()
        .map(|part| part.to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn collect_agents_file(
    workspace: &Workspace,
    directory: &Path,
    output: &mut Vec<WorkspaceRule>,
) -> AppResult<()> {
    let path = directory.join("AGENTS.md");
    if !path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(&path).map_err(|error| {
        AppError::fs(
            "read_agents_failed",
            "failed to read AGENTS.md",
            relative_path(workspace, &path),
            &error,
        )
    })?;

    output.push(WorkspaceRule::new(relative_path(workspace, &path), content));
    Ok(())
}
