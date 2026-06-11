use std::fs;

use serde_json::json;

use crate::namespace::config;
use crate::namespace::types::{
    NamespaceCreated, NamespaceDeleted, NamespaceDocument, NamespaceDocuments, NamespaceRenamed,
    NamespaceSummary,
};
use crate::utils::error::{AppError, AppResult};
use crate::utils::index;
use crate::utils::paths::validate_namespace;
use crate::utils::workspace;

pub fn list() -> AppResult<Vec<NamespaceSummary>> {
    let workspace = workspace::find()?;
    config::list_namespaces(&workspace)?
        .into_iter()
        .map(|namespace| {
            let namespace_config = config::read(&workspace, &namespace)?;
            let directory = config::namespace_dir(&workspace, &namespace);
            Ok(NamespaceSummary {
                namespace,
                path: workspace::relative_path(&workspace, &directory),
                naming_style_regex: namespace_config.naming_style_regex,
            })
        })
        .collect::<AppResult<Vec<_>>>()
}

pub fn create(
    namespace_name: &str,
    document_name_regex: Option<&str>,
) -> AppResult<NamespaceCreated> {
    if document_name_regex.is_some() {
        return Err(AppError::input(
            "unsupported_namespace_regex",
            "namespace-specific document_name_regex is no longer supported; use docs.json naming_style",
        ));
    }

    validate_namespace(namespace_name)?;
    let workspace = workspace::find()?;
    let namespace_config = config::NamespaceConfig::from_workspace(&workspace)?;
    config::validate_namespace_name_style(namespace_name, &namespace_config)?;
    workspace::ensure_documents_directory(&workspace)?;

    let directory = config::namespace_dir(&workspace, namespace_name);
    if config::namespace_exists(&workspace, namespace_name)? || directory.exists() {
        return Err(AppError::validation(
            "namespace_exists",
            "namespace already exists",
            json!({ "namespace": namespace_name }),
        ));
    }

    fs::create_dir_all(&directory).map_err(|error| {
        AppError::fs(
            "create_namespace_failed",
            "failed to create namespace",
            workspace::relative_path(&workspace, &directory),
            &error,
        )
    })?;

    Ok(NamespaceCreated {
        namespace: namespace_name.to_string(),
        path: workspace::relative_path(&workspace, &directory),
        naming_style_regex: namespace_config.naming_style_regex,
    })
}

pub fn list_docs() -> AppResult<Vec<NamespaceDocuments>> {
    let workspace = workspace::find()?;
    let documents = index::scan_documents(&workspace)?;
    let namespaces = config::list_namespaces(&workspace)?
        .into_iter()
        .map(|namespace| {
            let namespace_config = config::read(&workspace, &namespace)?;
            let namespace_documents = documents
                .iter()
                .filter(|document| document.namespace == namespace)
                .map(|document| NamespaceDocument {
                    document: document.document.clone(),
                    path: document.path.clone(),
                    title: document.title.clone(),
                    description: document.description.clone(),
                })
                .collect::<Vec<_>>();

            Ok(NamespaceDocuments {
                namespace,
                naming_style_regex: namespace_config.naming_style_regex,
                documents: namespace_documents,
            })
        })
        .collect::<AppResult<Vec<_>>>()?;

    Ok(namespaces)
}

pub fn rename(origin_name: &str, new_name: &str) -> AppResult<NamespaceRenamed> {
    validate_namespace(origin_name)?;
    validate_namespace(new_name)?;
    let workspace = workspace::find()?;
    let namespace_config = config::NamespaceConfig::from_workspace(&workspace)?;
    config::validate_namespace_name_style(new_name, &namespace_config)?;
    let old_directory = config::namespace_dir(&workspace, origin_name);
    let new_directory = config::namespace_dir(&workspace, new_name);
    ensure_namespace_exists(&workspace, origin_name)?;

    if config::namespace_exists(&workspace, new_name)? || new_directory.exists() {
        return Err(AppError::validation(
            "namespace_exists",
            "target namespace already exists",
            json!({ "namespace": new_name }),
        ));
    }

    fs::rename(&old_directory, &new_directory).map_err(|error| {
        AppError::fs(
            "rename_namespace_failed",
            "failed to rename namespace",
            format!(
                "{} -> {}",
                workspace::relative_path(&workspace, &old_directory),
                workspace::relative_path(&workspace, &new_directory)
            ),
            &error,
        )
    })?;

    Ok(NamespaceRenamed {
        old_namespace: origin_name.to_string(),
        new_namespace: new_name.to_string(),
        path: workspace::relative_path(&workspace, &new_directory),
    })
}

pub fn delete(namespace_name: &str, delete_docs: bool) -> AppResult<NamespaceDeleted> {
    validate_namespace(namespace_name)?;
    let workspace = workspace::find()?;
    let directory = config::namespace_dir(&workspace, namespace_name);
    ensure_namespace_exists(&workspace, namespace_name)?;

    if !delete_docs && !config::is_empty(&directory)? {
        return Err(AppError::validation(
            "namespace_not_empty",
            "namespace is not empty",
            json!({
                "namespace": namespace_name,
                "required_flag": "--delete-docs"
            }),
        ));
    }

    fs::remove_dir_all(&directory).map_err(|error| {
        AppError::fs(
            "delete_namespace_failed",
            "failed to delete namespace",
            workspace::relative_path(&workspace, &directory),
            &error,
        )
    })?;

    Ok(NamespaceDeleted {
        namespace: namespace_name.to_string(),
        deleted: true,
        deleted_docs: delete_docs,
    })
}

fn ensure_namespace_exists(workspace: &workspace::Workspace, namespace: &str) -> AppResult<()> {
    let directory = config::namespace_dir(workspace, namespace);
    if config::namespace_exists(workspace, namespace)? && directory.is_dir() {
        return Ok(());
    }

    Err(AppError::validation(
        "namespace_not_found",
        "namespace does not exist",
        json!({ "namespace": namespace }),
    ))
}
