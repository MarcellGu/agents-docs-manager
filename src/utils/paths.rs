use std::path::PathBuf;

use regex::Regex;
use serde::Serialize;
use serde_json::json;

use crate::utils::error::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DocumentPath {
    pub documents_directory_name: String,
    pub namespace: String,
    pub document_stem: String,
    pub file_name: String,
    pub relative_path: String,
}

impl DocumentPath {
    pub fn to_path_buf(&self) -> PathBuf {
        let mut path = PathBuf::from(&self.documents_directory_name);
        if !self.namespace.is_empty() {
            path.push(&self.namespace);
        }
        path.push(&self.file_name);
        path
    }
}

pub fn validate_namespace(namespace: &str) -> AppResult<()> {
    if namespace.is_empty() {
        return invalid_namespace(namespace, "namespace must not be empty");
    }

    if namespace == "." || namespace == ".." {
        return invalid_namespace(namespace, "namespace must not be a relative path segment");
    }

    if namespace.starts_with('.') {
        return invalid_namespace(namespace, "namespace must not start with a dot");
    }

    if namespace.contains('/') || namespace.contains('\\') {
        return invalid_namespace(namespace, "namespace must be a single path segment");
    }

    if std::path::Path::new(namespace).is_absolute() {
        return invalid_namespace(namespace, "namespace must not be an absolute path");
    }

    Ok(())
}

pub fn normalize_document_name(input: &str) -> AppResult<String> {
    if input.is_empty() {
        return invalid_document(input, "document must not be empty");
    }

    if input == "." || input == ".." {
        return invalid_document(input, "document must not be a relative path segment");
    }

    if input.starts_with('.') {
        return invalid_document(input, "document must not start with a dot");
    }

    if input.contains('/') || input.contains('\\') {
        return invalid_document(input, "document must be a single path segment");
    }

    if std::path::Path::new(input).is_absolute() {
        return invalid_document(input, "document must not be an absolute path");
    }

    let stem = input.strip_suffix(".md").unwrap_or(input);
    if stem.is_empty() {
        return invalid_document(input, "document stem must not be empty");
    }

    if stem == "." || stem == ".." || stem.starts_with('.') {
        return invalid_document(input, "document stem must be a normal file name");
    }

    Ok(stem.to_string())
}

pub fn build_document_path(
    documents_directory_name: &str,
    namespace: &str,
    document_stem: &str,
) -> DocumentPath {
    let file_name = format!("{document_stem}.md");
    let relative_path = if namespace.is_empty() {
        format!("{documents_directory_name}/{file_name}")
    } else {
        format!("{documents_directory_name}/{namespace}/{file_name}")
    };

    DocumentPath {
        documents_directory_name: documents_directory_name.to_string(),
        namespace: namespace.to_string(),
        document_stem: document_stem.to_string(),
        file_name: file_name.clone(),
        relative_path,
    }
}

pub fn compile_document_regex(pattern: &str) -> AppResult<Regex> {
    Regex::new(&format!("^(?:{pattern})$")).map_err(|error| {
        AppError::input_with_details(
            "invalid_document_name_regex",
            "document_name_regex is not a valid regex",
            json!({
                "document_name_regex": pattern,
                "source": error.to_string()
            }),
        )
    })
}

fn invalid_namespace<T>(namespace: &str, message: &str) -> AppResult<T> {
    Err(AppError::input_with_details(
        "invalid_namespace",
        message,
        json!({ "namespace": namespace }),
    ))
}

fn invalid_document<T>(document: &str, message: &str) -> AppResult<T> {
    Err(AppError::input_with_details(
        "invalid_document_name",
        message,
        json!({ "document": document }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_md_suffix_and_builds_document_path() {
        let document = normalize_document_name("CODE_STYLE.md").unwrap();
        let path = build_document_path("Docs", "Conventions", &document);
        assert_eq!(path.namespace, "Conventions");
        assert_eq!(path.document_stem, "CODE_STYLE");
        assert_eq!(path.relative_path, "Docs/Conventions/CODE_STYLE.md");

        let root_path = build_document_path("Docs", "", &document);
        assert_eq!(root_path.namespace, "");
        assert_eq!(root_path.relative_path, "Docs/CODE_STYLE.md");
    }

    #[test]
    fn rejects_path_traversal() {
        assert!(validate_namespace("..").is_err());
        assert!(normalize_document_name("../CODE_STYLE").is_err());
        assert!(normalize_document_name(".hidden").is_err());
    }
}
