use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::utils::error::{AppError, AppResult};
use crate::utils::markdown::{self, DocumentMetadata};
use crate::utils::paths::{
    build_document_path, compile_document_regex, normalize_document_name, validate_namespace,
};
use crate::utils::workspace::{self, Workspace};

pub const INDEX_FILE_NAME: &str = "docs.json";
pub const DEFAULT_INDEX_FILE: &str = "./AGENTS.md";
pub const LOWERCASE_NAMING_STYLE: &str = "snake_case";
pub const UPPERCASE_NAMING_STYLE: &str = "PascalCase";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocumentEntry {
    pub namespace: String,
    pub document: String,
    pub path: String,
    pub title: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocsIndex {
    pub index_file: String,
    pub doc_dir: String,
    pub naming_style: String,
}

impl DocsIndex {
    pub fn new(doc_dir: String, naming_style: String) -> Self {
        Self {
            index_file: DEFAULT_INDEX_FILE.to_string(),
            naming_style,
            doc_dir,
        }
    }

    pub fn naming_style_regex(&self) -> AppResult<String> {
        naming_style_regex(&self.naming_style)
    }
}

pub fn index_path(root: &Path) -> PathBuf {
    root.join(INDEX_FILE_NAME)
}

pub fn read_from_root(root: &Path) -> AppResult<DocsIndex> {
    let path = index_path(root);
    if !path.exists() {
        return Err(AppError::input_with_details(
            "workspace_index_not_found",
            "failed to locate docs.json; run adm init first",
            json!({ "path": INDEX_FILE_NAME }),
        ));
    }

    let content = fs::read_to_string(&path).map_err(|error| {
        AppError::fs(
            "read_workspace_index_failed",
            "failed to read docs.json",
            workspace::relative_path_from_root(root, &path),
            &error,
        )
    })?;

    let index = serde_json::from_str::<DocsIndex>(&content).map_err(|error| {
        AppError::input_with_details(
            "invalid_workspace_index",
            "docs.json is not valid JSON",
            json!({
                "path": workspace::relative_path_from_root(root, &path),
                "source": error.to_string()
            }),
        )
    })?;

    validate_index(&index)?;
    Ok(index)
}

pub fn read(workspace: &Workspace) -> AppResult<DocsIndex> {
    read_from_root(&workspace.root)
}

pub fn write_to_root(root: &Path, index: &DocsIndex) -> AppResult<()> {
    validate_index(index)?;
    let path = index_path(root);
    let index = index.clone();

    let content = serde_json::to_string_pretty(&index).map_err(|error| {
        AppError::input_with_details(
            "serialize_workspace_index_failed",
            "failed to serialize docs.json",
            json!({ "source": error.to_string() }),
        )
    })?;

    fs::write(&path, format!("{content}\n")).map_err(|error| {
        AppError::fs(
            "write_workspace_index_failed",
            "failed to write docs.json",
            workspace::relative_path_from_root(root, &path),
            &error,
        )
    })
}

pub fn bootstrap_from_disk(root: &Path, index: &mut DocsIndex) -> AppResult<()> {
    let documents_directory = root.join(config_path_to_relative(&index.doc_dir)?);
    if !documents_directory.is_dir() {
        return Ok(());
    }

    let namespace_entries = fs::read_dir(&documents_directory).map_err(|error| {
        AppError::fs(
            "read_docs_failed",
            "failed to read documents directory",
            workspace::relative_path_from_root(root, &documents_directory),
            &error,
        )
    })?;

    for entry in namespace_entries {
        let entry = entry.map_err(|error| {
            AppError::fs(
                "read_docs_entry_failed",
                "failed to read documents directory entry",
                workspace::relative_path_from_root(root, &documents_directory),
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

        let namespace = entry.file_name().to_string_lossy().to_string();
        let _ = validate_namespace(&namespace);
    }

    Ok(())
}

pub fn scan_documents(workspace: &Workspace) -> AppResult<Vec<DocumentEntry>> {
    let mut documents = Vec::new();

    for document in list_document_stems(workspace, "")? {
        let doc_path = build_document_path(&workspace.documents_directory_name, "", &document);
        let absolute_path = workspace.root.join(doc_path.to_path_buf());
        let content = fs::read_to_string(&absolute_path).map_err(|error| {
            AppError::fs(
                "read_document_failed",
                "failed to read document",
                doc_path.relative_path.clone(),
                &error,
            )
        })?;

        let metadata = document_metadata_from_content(&doc_path.relative_path, &content)?;

        documents.push(DocumentEntry {
            namespace: String::new(),
            document,
            path: doc_path.relative_path,
            title: metadata.title,
            description: metadata.description,
        });
    }

    for namespace in list_namespaces(workspace)? {
        for document in list_document_stems(workspace, &namespace)? {
            let doc_path =
                build_document_path(&workspace.documents_directory_name, &namespace, &document);
            let absolute_path = workspace.root.join(doc_path.to_path_buf());
            let content = fs::read_to_string(&absolute_path).map_err(|error| {
                AppError::fs(
                    "read_document_failed",
                    "failed to read document",
                    doc_path.relative_path.clone(),
                    &error,
                )
            })?;

            let metadata = document_metadata_from_content(&doc_path.relative_path, &content)?;

            documents.push(DocumentEntry {
                namespace: namespace.clone(),
                document,
                path: doc_path.relative_path,
                title: metadata.title,
                description: metadata.description,
            });
        }
    }

    documents.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(documents)
}

pub fn document_metadata_from_content(
    relative_path: &str,
    content: &str,
) -> AppResult<DocumentMetadata> {
    markdown::extract_metadata(content).map_err(|error| {
        AppError::validation(
            "invalid_document_metadata",
            error.message(),
            json!({
                "path": relative_path,
                "required_structure": "# <title> followed by 本文说明<description>。"
            }),
        )
    })
}

pub fn list_namespaces(workspace: &Workspace) -> AppResult<Vec<String>> {
    if !workspace.documents_directory.exists() {
        return Ok(Vec::new());
    }

    let mut namespaces = Vec::new();
    let entries = fs::read_dir(&workspace.documents_directory).map_err(|error| {
        AppError::fs(
            "read_docs_failed",
            "failed to read documents directory",
            workspace::relative_path(workspace, &workspace.documents_directory),
            &error,
        )
    })?;

    for entry in entries {
        let entry = entry.map_err(|error| {
            AppError::fs(
                "read_docs_entry_failed",
                "failed to read documents directory entry",
                workspace::relative_path(workspace, &workspace.documents_directory),
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

        let namespace = entry.file_name().to_string_lossy().to_string();
        if validate_namespace(&namespace).is_ok() {
            namespaces.push(namespace);
        }
    }

    namespaces.sort();
    Ok(namespaces)
}

pub fn list_document_stems(workspace: &Workspace, namespace: &str) -> AppResult<Vec<String>> {
    let directory = if namespace.is_empty() {
        workspace.documents_directory.clone()
    } else {
        workspace.documents_directory.join(namespace)
    };
    if !directory.exists() {
        return Ok(Vec::new());
    }

    let mut documents = Vec::new();
    let entries = fs::read_dir(&directory).map_err(|error| {
        AppError::fs(
            "read_documents_failed",
            "failed to read documents directory",
            workspace::relative_path(workspace, &directory),
            &error,
        )
    })?;

    for entry in entries {
        let entry = entry.map_err(|error| {
            AppError::fs(
                "read_documents_entry_failed",
                "failed to read documents directory entry",
                workspace::relative_path(workspace, &directory),
                &error,
            )
        })?;

        let file_name = entry.file_name().to_string_lossy().to_string();
        if !file_name.ends_with(".md") {
            continue;
        }

        if entry
            .file_type()
            .map(|file_type| file_type.is_file())
            .unwrap_or(false)
        {
            documents.push(normalize_document_name(&file_name)?);
        }
    }

    documents.sort();
    Ok(documents)
}

fn validate_index(index: &DocsIndex) -> AppResult<()> {
    validate_config_path("index_file", &index.index_file)?;
    validate_config_path("doc_dir", &index.doc_dir)?;
    compile_document_regex(&index.naming_style_regex()?)?;
    Ok(())
}

pub fn config_path_to_relative(path: &str) -> AppResult<PathBuf> {
    validate_config_path("path", path)?;
    Ok(PathBuf::from(path.strip_prefix("./").unwrap_or(path)))
}

pub fn config_path_to_display(path: &str) -> AppResult<String> {
    validate_config_path("path", path)?;
    Ok(path.strip_prefix("./").unwrap_or(path).to_string())
}

pub fn naming_style_regex(naming_style: &str) -> AppResult<String> {
    match naming_style {
        "snake_case" => Ok("[a-z][a-z0-9_]*".to_string()),
        "PascalCase" | "PaselCase" => Ok("[A-Z][A-Za-z0-9]*".to_string()),
        "camelCase" | "camalCase" => Ok("[a-z][A-Za-z0-9]*".to_string()),
        "kebab-case" => Ok("[a-z][a-z0-9-]*".to_string()),
        "SCREAMING_SNAKE_CASE" | "screaming_snake_case" => Ok("[A-Z][A-Z0-9_]*".to_string()),
        _ => Err(AppError::input_with_details(
            "invalid_naming_style",
            "naming_style is not supported",
            json!({
                "naming_style": naming_style,
                "supported": [
                    "snake_case",
                    "PascalCase",
                    "PaselCase",
                    "camelCase",
                    "camalCase",
                    "kebab-case",
                    "SCREAMING_SNAKE_CASE",
                    "screaming_snake_case"
                ]
            }),
        )),
    }
}

pub fn infer_naming_style_from_name(name: &str) -> &'static str {
    if name.contains('-') && matches_naming_style(name, "kebab-case") {
        return "kebab-case";
    }

    if name.contains('_') && matches_naming_style(name, "SCREAMING_SNAKE_CASE") {
        return "SCREAMING_SNAKE_CASE";
    }

    if name.contains('_') && matches_naming_style(name, "snake_case") {
        return "snake_case";
    }

    if name.chars().any(|character| character.is_ascii_uppercase()) {
        if matches_naming_style(name, "PascalCase") {
            return UPPERCASE_NAMING_STYLE;
        }

        if matches_naming_style(name, "camelCase") {
            return "camelCase";
        }
    }

    LOWERCASE_NAMING_STYLE
}

pub fn naming_style_uses_uppercase_doc_dir(naming_style: &str) -> bool {
    matches!(
        naming_style,
        "PascalCase" | "PaselCase" | "SCREAMING_SNAKE_CASE"
    )
}

pub fn convert_name_to_naming_style(name: &str, naming_style: &str) -> AppResult<String> {
    let words = split_name_words(name);
    if words.is_empty() {
        return Err(AppError::validation(
            "invalid_document_name",
            "document name cannot be converted to naming_style",
            json!({
                "document": name,
                "naming_style": naming_style
            }),
        ));
    }

    match naming_style {
        "snake_case" => Ok(words.join("_")),
        "kebab-case" => Ok(words.join("-")),
        "SCREAMING_SNAKE_CASE" | "screaming_snake_case" => Ok(words.join("_").to_uppercase()),
        "PascalCase" | "PaselCase" => Ok(words
            .iter()
            .map(|word| capitalize_ascii(word))
            .collect::<Vec<_>>()
            .join("")),
        "camelCase" | "camalCase" => {
            let mut converted = String::new();
            converted.push_str(&words[0]);
            for word in words.iter().skip(1) {
                converted.push_str(&capitalize_ascii(word));
            }
            Ok(converted)
        }
        _ => Err(AppError::input_with_details(
            "invalid_naming_style",
            "naming_style is not supported",
            json!({
                "naming_style": naming_style,
                "supported": [
                    "snake_case",
                    "PascalCase",
                    "PaselCase",
                    "camelCase",
                    "camalCase",
                    "kebab-case",
                    "SCREAMING_SNAKE_CASE",
                    "screaming_snake_case"
                ]
            }),
        )),
    }
}

fn matches_naming_style(name: &str, naming_style: &str) -> bool {
    let Ok(pattern) = naming_style_regex(naming_style) else {
        return false;
    };
    let Ok(regex) = compile_document_regex(&pattern) else {
        return false;
    };
    regex.is_match(name)
}

fn split_name_words(name: &str) -> Vec<String> {
    let chars = name.chars().collect::<Vec<_>>();
    let mut words = Vec::new();
    let mut current = String::new();

    for (index, character) in chars.iter().enumerate() {
        if !character.is_ascii_alphanumeric() {
            push_word(&mut words, &mut current);
            continue;
        }

        if character.is_ascii_uppercase() && !current.is_empty() {
            let previous = chars[index - 1];
            let next_is_lowercase = chars
                .get(index + 1)
                .is_some_and(|next| next.is_ascii_lowercase());
            if previous.is_ascii_lowercase()
                || previous.is_ascii_digit()
                || (previous.is_ascii_uppercase() && next_is_lowercase)
            {
                push_word(&mut words, &mut current);
            }
        }

        current.push(character.to_ascii_lowercase());
    }

    push_word(&mut words, &mut current);
    words
}

fn push_word(words: &mut Vec<String>, current: &mut String) {
    if !current.is_empty() {
        words.push(std::mem::take(current));
    }
}

fn capitalize_ascii(word: &str) -> String {
    let mut chars = word.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };

    let mut output = String::new();
    output.push(first.to_ascii_uppercase());
    output.push_str(chars.as_str());
    output
}

fn validate_config_path(field: &str, path: &str) -> AppResult<()> {
    let relative = path.strip_prefix("./").unwrap_or(path);
    let slash_normalized = relative.replace('\\', "/");
    if relative.is_empty()
        || relative == "."
        || has_forbidden_config_path_component(Path::new(path))
        || has_forbidden_config_path_component(Path::new(&slash_normalized))
        || has_windows_drive_prefix(relative)
    {
        return Err(AppError::input_with_details(
            "invalid_workspace_config",
            "workspace config path must be relative",
            json!({ field: path }),
        ));
    }

    Ok(())
}

fn has_forbidden_config_path_component(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    })
}

fn has_windows_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_uses_provided_naming_style() {
        let index = DocsIndex::new("./docs".to_string(), "snake_case".to_string());
        assert_eq!(index.index_file, "./AGENTS.md");
        assert_eq!(index.doc_dir, "./docs");
        assert_eq!(index.naming_style, "snake_case");
    }

    #[test]
    fn infers_naming_style_from_workspace_name() {
        assert_eq!(infer_naming_style_from_name("cloud_engine"), "snake_case");
        assert_eq!(infer_naming_style_from_name("cloud-engine"), "kebab-case");
        assert_eq!(infer_naming_style_from_name("CloudEngine"), "PascalCase");
        assert_eq!(infer_naming_style_from_name("cloudEngine"), "camelCase");
        assert_eq!(
            infer_naming_style_from_name("CLOUD_ENGINE"),
            "SCREAMING_SNAKE_CASE"
        );
    }

    #[test]
    fn converts_names_to_supported_naming_styles() {
        assert_eq!(
            convert_name_to_naming_style("CodeStyle", "snake_case").unwrap(),
            "code_style"
        );
        assert_eq!(
            convert_name_to_naming_style("CODE_STYLE", "camelCase").unwrap(),
            "codeStyle"
        );
        assert_eq!(
            convert_name_to_naming_style("code-style", "PascalCase").unwrap(),
            "CodeStyle"
        );
        assert_eq!(
            convert_name_to_naming_style("codeStyle", "SCREAMING_SNAKE_CASE").unwrap(),
            "CODE_STYLE"
        );
    }

    #[test]
    fn rejects_workspace_config_paths_that_escape_the_repository() {
        assert!(validate_config_path("doc_dir", "./docs").is_ok());
        assert!(validate_config_path("index_file", "nested/AGENTS.md").is_ok());

        for path in [
            "",
            ".",
            "..",
            "../docs",
            "docs/../outside",
            "..\\docs",
            "docs\\..\\outside",
            "\\absolute",
            "C:\\absolute",
            "C:relative",
        ] {
            assert!(
                validate_config_path("doc_dir", path).is_err(),
                "path should be rejected: {path}"
            );
        }
    }
}
