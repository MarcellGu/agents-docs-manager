use std::fs;
use std::path::Path;

use regex::Regex;
use serde::Serialize;
use serde_json::json;

use crate::utils::error::{AppError, AppResult};
use crate::utils::index;
use crate::utils::paths::{self, validate_namespace};
use crate::utils::workspace::Workspace;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamespaceConfig {
    pub naming_style_regex: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct InvalidNamespace {
    pub namespace: String,
    pub naming_style_regex: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct InvalidDocument {
    pub namespace: String,
    pub document: String,
    pub naming_style_regex: String,
}

impl NamespaceConfig {
    pub fn from_workspace(workspace: &Workspace) -> AppResult<Self> {
        let index = index::read(workspace)?;
        Ok(Self {
            naming_style_regex: index.naming_style_regex()?,
        })
    }
}

pub fn namespace_exists(workspace: &Workspace, namespace: &str) -> AppResult<bool> {
    validate_namespace(namespace)?;
    Ok(namespace_dir(workspace, namespace).is_dir())
}

pub fn namespace_dir(workspace: &Workspace, namespace: &str) -> std::path::PathBuf {
    workspace.documents_directory.join(namespace)
}

pub fn read(workspace: &Workspace, namespace: &str) -> AppResult<NamespaceConfig> {
    validate_namespace(namespace)?;
    NamespaceConfig::from_workspace(workspace)
}

pub fn compile_document_regex(pattern: &str) -> AppResult<Regex> {
    paths::compile_document_regex(pattern)
}

pub fn validate_document_name(
    namespace: &str,
    document_stem: &str,
    config: &NamespaceConfig,
) -> AppResult<()> {
    let regex = compile_document_regex(&config.naming_style_regex)?;
    if regex.is_match(document_stem) {
        return Ok(());
    }

    Err(AppError::validation(
        "invalid_document_name",
        "document name does not match naming_style",
        json!({
            "namespace": namespace,
            "document": document_stem,
            "naming_style_regex": config.naming_style_regex
        }),
    ))
}

pub fn validate_namespace_name_style(namespace: &str, config: &NamespaceConfig) -> AppResult<()> {
    let regex = compile_document_regex(&config.naming_style_regex)?;
    if regex.is_match(namespace) {
        return Ok(());
    }

    Err(AppError::validation(
        "invalid_namespace_name",
        "namespace name does not match naming_style",
        json!({
            "namespace": namespace,
            "naming_style_regex": config.naming_style_regex
        }),
    ))
}

pub fn list_namespaces(workspace: &Workspace) -> AppResult<Vec<String>> {
    index::list_namespaces(workspace)
}

pub fn list_document_stems(workspace: &Workspace, namespace: &str) -> AppResult<Vec<String>> {
    index::list_document_stems(workspace, namespace)
}

pub fn invalid_documents_for_regex(
    workspace: &Workspace,
    namespace: &str,
    regex_pattern: &str,
) -> AppResult<Vec<InvalidDocument>> {
    let config = NamespaceConfig {
        naming_style_regex: regex_pattern.to_string(),
    };
    let regex = compile_document_regex(&config.naming_style_regex)?;
    let mut invalid = Vec::new();

    for document in list_document_stems(workspace, namespace)? {
        if !regex.is_match(&document) {
            invalid.push(InvalidDocument {
                namespace: namespace.to_string(),
                document,
                naming_style_regex: regex_pattern.to_string(),
            });
        }
    }

    Ok(invalid)
}

pub fn all_invalid_documents(workspace: &Workspace) -> AppResult<Vec<InvalidDocument>> {
    let mut invalid = Vec::new();
    let config = NamespaceConfig::from_workspace(workspace)?;
    invalid.extend(invalid_documents_for_regex(
        workspace,
        "",
        &config.naming_style_regex,
    )?);

    for namespace in list_namespaces(workspace)? {
        let config = read(workspace, &namespace)?;
        invalid.extend(invalid_documents_for_regex(
            workspace,
            &namespace,
            &config.naming_style_regex,
        )?);
    }
    Ok(invalid)
}

pub fn all_invalid_namespaces(workspace: &Workspace) -> AppResult<Vec<InvalidNamespace>> {
    let config = NamespaceConfig::from_workspace(workspace)?;
    let regex = compile_document_regex(&config.naming_style_regex)?;
    let mut invalid = Vec::new();

    for namespace in list_namespaces(workspace)? {
        if !regex.is_match(&namespace) {
            invalid.push(InvalidNamespace {
                namespace,
                naming_style_regex: config.naming_style_regex.clone(),
            });
        }
    }

    Ok(invalid)
}

pub fn is_empty(path: &Path) -> AppResult<bool> {
    let mut entries = fs::read_dir(path).map_err(|error| {
        AppError::fs(
            "read_namespace_failed",
            "failed to read namespace directory",
            path.to_string_lossy(),
            &error,
        )
    })?;

    if let Some(entry) = entries.next() {
        entry.map_err(|error| {
            AppError::fs(
                "read_namespace_entry_failed",
                "failed to read namespace directory entry",
                path.to_string_lossy(),
                &error,
            )
        })?;
        return Ok(false);
    }

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regex_matches_complete_document_stem() {
        let config = NamespaceConfig {
            naming_style_regex: "[A-Z][A-Z0-9_]*".to_string(),
        };
        assert!(validate_document_name("Conventions", "CODE_STYLE", &config).is_ok());
        assert!(validate_document_name("Conventions", "CODE_STYLE_extra", &config).is_err());
        assert!(validate_document_name("Conventions", "code-style", &config).is_err());
    }
}
