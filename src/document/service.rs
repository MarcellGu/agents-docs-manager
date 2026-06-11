use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::json;

use crate::document::patch::apply_stdin_patch;
use crate::document::types::{
    DocumentCreated, DocumentDeleted, DocumentPatched, DocumentRenamed, DocumentSummary,
    FixMigration,
};
use crate::namespace::config;
use crate::utils::error::{AppError, AppResult};
use crate::utils::index;
use crate::utils::paths::{
    DocumentPath, build_document_path, normalize_document_name, validate_namespace,
};
use crate::utils::workspace;

pub fn list(namespace: Option<&str>) -> AppResult<Vec<DocumentSummary>> {
    let workspace = workspace::find()?;
    if let Some(namespace) = namespace {
        validate_namespace(namespace)?;
        let namespace_config = config::read(&workspace, namespace)?;
        config::validate_namespace_name_style(namespace, &namespace_config)?;
        ensure_namespace_exists(&workspace, namespace)?;
    }

    let documents = index::scan_documents(&workspace)?
        .into_iter()
        .filter(|document| namespace.is_none_or(|namespace| document.namespace == namespace))
        .map(|document| DocumentSummary {
            namespace: document.namespace,
            document: document.document,
            path: document.path,
            title: document.title,
            description: document.description,
        })
        .collect::<Vec<_>>();

    Ok(documents)
}

pub fn create(
    namespace: Option<&str>,
    doc_name: &str,
    markdown_content: &str,
) -> AppResult<DocumentCreated> {
    let document_stem = normalize_document_name(doc_name)?;
    let workspace = workspace::find()?;
    let namespace_config = document_namespace_config(&workspace, namespace)?;
    if let Some(namespace) = namespace {
        validate_namespace(namespace)?;
        config::validate_namespace_name_style(namespace, &namespace_config)?;
    }
    let namespace = namespace.unwrap_or("");
    let document_path = build_document_path(
        &workspace.documents_directory_name,
        namespace,
        &document_stem,
    );
    if !document_path.namespace.is_empty() {
        ensure_namespace_exists(&workspace, &document_path.namespace)?;
    } else {
        workspace::ensure_documents_directory(&workspace)?;
    }
    config::validate_document_name(
        &document_path.namespace,
        &document_path.document_stem,
        &namespace_config,
    )?;
    index::document_metadata_from_content(&document_path.relative_path, markdown_content)?;

    let absolute_path = workspace.root.join(document_path.to_path_buf());
    if absolute_path.exists() {
        return Err(AppError::validation(
            "document_exists",
            "document already exists",
            json!({ "path": document_path.relative_path }),
        ));
    }

    fs::write(&absolute_path, markdown_content).map_err(|error| {
        AppError::fs(
            "create_document_failed",
            "failed to create document",
            document_path.relative_path.clone(),
            &error,
        )
    })?;

    Ok(DocumentCreated {
        path: document_path.relative_path,
        document: document_path.document_stem,
        namespace: document_path.namespace,
    })
}

pub fn rename_unique(origin_doc_name: &str, new_doc_name: &str) -> AppResult<DocumentRenamed> {
    let origin_document = normalize_document_name(origin_doc_name)?;
    let new_document = normalize_document_name(new_doc_name)?;
    let workspace = workspace::find()?;
    let source = find_unique_document(&workspace, &origin_document)?;
    let namespace_config = document_namespace_config(&workspace, optional_namespace(&source))?;
    if !source.namespace.is_empty() {
        config::validate_namespace_name_style(&source.namespace, &namespace_config)?;
    }
    config::validate_document_name(&source.namespace, &new_document, &namespace_config)?;

    let old_absolute_path = workspace.root.join(source.to_path_buf());
    ensure_document_exists(&old_absolute_path, &source.relative_path)?;
    let new_document_path = build_document_path(
        &workspace.documents_directory_name,
        &source.namespace,
        &new_document,
    );
    let new_absolute_path = workspace.root.join(new_document_path.to_path_buf());
    if new_absolute_path.exists() {
        return Err(AppError::validation(
            "document_exists",
            "target document already exists",
            json!({ "path": new_document_path.relative_path }),
        ));
    }

    fs::rename(&old_absolute_path, &new_absolute_path).map_err(|error| {
        AppError::fs(
            "rename_document_failed",
            "failed to rename document",
            format!(
                "{} -> {}",
                source.relative_path, new_document_path.relative_path
            ),
            &error,
        )
    })?;

    Ok(DocumentRenamed {
        old_path: source.relative_path,
        new_path: new_document_path.relative_path,
        namespace: source.namespace,
    })
}

pub fn delete(namespace: Option<&str>, doc_name: &str) -> AppResult<DocumentDeleted> {
    let document_stem = normalize_document_name(doc_name)?;
    let workspace = workspace::find()?;
    let namespace_config = document_namespace_config(&workspace, namespace)?;
    if let Some(namespace) = namespace {
        validate_namespace(namespace)?;
        config::validate_namespace_name_style(namespace, &namespace_config)?;
    }
    let namespace = namespace.unwrap_or("");
    let document_path = build_document_path(
        &workspace.documents_directory_name,
        namespace,
        &document_stem,
    );
    if !namespace.is_empty() {
        ensure_namespace_exists(&workspace, namespace)?;
    }
    let absolute_path = workspace.root.join(document_path.to_path_buf());
    ensure_document_exists(&absolute_path, &document_path.relative_path)?;

    fs::remove_file(&absolute_path).map_err(|error| {
        AppError::fs(
            "delete_document_failed",
            "failed to delete document",
            document_path.relative_path.clone(),
            &error,
        )
    })?;

    Ok(DocumentDeleted {
        path: document_path.relative_path,
        namespace: namespace.to_string(),
        document: document_stem,
        deleted: true,
    })
}

pub fn patch(namespace: Option<&str>, doc_name: &str) -> AppResult<DocumentPatched> {
    let document_stem = normalize_document_name(doc_name)?;
    let workspace = workspace::find()?;
    let namespace_config = document_namespace_config(&workspace, namespace)?;
    if let Some(namespace) = namespace {
        validate_namespace(namespace)?;
        config::validate_namespace_name_style(namespace, &namespace_config)?;
    }
    let namespace = namespace.unwrap_or("");
    let document_path = build_document_path(
        &workspace.documents_directory_name,
        namespace,
        &document_stem,
    );
    if !namespace.is_empty() {
        ensure_namespace_exists(&workspace, namespace)?;
    }

    let absolute_path = workspace.root.join(document_path.to_path_buf());
    ensure_document_exists(&absolute_path, &document_path.relative_path)?;

    let original = fs::read_to_string(&absolute_path).map_err(|error| {
        AppError::fs(
            "read_document_failed",
            "failed to read document",
            document_path.relative_path.clone(),
            &error,
        )
    })?;
    let patched = apply_stdin_patch(&original, &document_path.relative_path)?;
    index::document_metadata_from_content(&document_path.relative_path, &patched)?;

    fs::write(&absolute_path, patched).map_err(|error| {
        AppError::fs(
            "write_document_failed",
            "failed to write document",
            document_path.relative_path.clone(),
            &error,
        )
    })?;

    Ok(DocumentPatched {
        path: document_path.relative_path,
        namespace: namespace.to_string(),
        document: document_stem,
        patched: true,
    })
}

pub fn fix() -> AppResult<Vec<FixMigration>> {
    let workspace = workspace::find()?;
    let index_config = index::read(&workspace)?;
    let mut namespace_plans = Vec::new();
    let mut document_plans = Vec::new();
    let mut namespace_targets = HashMap::new();
    let mut document_targets = HashMap::new();

    for document in index::list_document_stems(&workspace, "")? {
        let new_document =
            index::convert_name_to_naming_style(&document, &index_config.naming_style)?;
        let old_document_path =
            build_document_path(&workspace.documents_directory_name, "", &document);
        let new_document_path =
            build_document_path(&workspace.documents_directory_name, "", &new_document);

        if let Some(existing) = document_targets.insert(
            new_document_path.relative_path.clone(),
            old_document_path.relative_path.clone(),
        ) {
            return Err(AppError::validation(
                "fix_conflict",
                "naming_style migration would create duplicate document paths",
                json!({
                    "path": new_document_path.relative_path,
                    "sources": [existing, old_document_path.relative_path]
                }),
            ));
        }

        if document == new_document {
            continue;
        }

        let old_absolute_path = workspace.root.join(old_document_path.to_path_buf());
        let new_absolute_path = workspace.root.join(new_document_path.to_path_buf());
        ensure_target_available(
            &old_absolute_path,
            &new_absolute_path,
            &new_document_path.relative_path,
        )?;
        document_plans.push(DocumentFixPlan {
            new_namespace: String::new(),
            old_document: document.clone(),
            new_document,
            old_relative_path: old_document_path.relative_path,
            new_relative_path: new_document_path.relative_path,
        });
    }

    for namespace in config::list_namespaces(&workspace)? {
        let new_namespace =
            index::convert_name_to_naming_style(&namespace, &index_config.naming_style)?;
        let old_namespace_path = workspace.documents_directory.join(&namespace);
        let new_namespace_path = workspace.documents_directory.join(&new_namespace);
        let old_namespace_relative =
            format!("{}/{}", workspace.documents_directory_name, namespace);
        let new_namespace_relative =
            format!("{}/{}", workspace.documents_directory_name, new_namespace);

        if let Some(existing) = namespace_targets.insert(
            new_namespace_relative.clone(),
            old_namespace_relative.clone(),
        ) {
            return Err(AppError::validation(
                "fix_conflict",
                "naming_style migration would create duplicate namespace paths",
                json!({
                    "path": new_namespace_relative,
                    "sources": [existing, old_namespace_relative]
                }),
            ));
        }

        if namespace != new_namespace {
            ensure_target_available(
                &old_namespace_path,
                &new_namespace_path,
                &new_namespace_relative,
            )?;
            namespace_plans.push(NamespaceFixPlan {
                old_path: old_namespace_path,
                new_path: new_namespace_path,
                old_relative_path: old_namespace_relative.clone(),
                new_relative_path: new_namespace_relative.clone(),
            });
        }

        for document in index::list_document_stems(&workspace, &namespace)? {
            let new_document =
                index::convert_name_to_naming_style(&document, &index_config.naming_style)?;
            let old_document_path =
                build_document_path(&workspace.documents_directory_name, &namespace, &document);
            let new_document_path = build_document_path(
                &workspace.documents_directory_name,
                &new_namespace,
                &new_document,
            );

            if let Some(existing) = document_targets.insert(
                new_document_path.relative_path.clone(),
                old_document_path.relative_path.clone(),
            ) {
                return Err(AppError::validation(
                    "fix_conflict",
                    "naming_style migration would create duplicate document paths",
                    json!({
                        "path": new_document_path.relative_path,
                        "sources": [existing, old_document_path.relative_path]
                    }),
                ));
            }

            if document == new_document {
                continue;
            }

            let old_absolute_path = workspace.root.join(old_document_path.to_path_buf());
            let new_absolute_path = workspace.root.join(new_document_path.to_path_buf());
            ensure_target_available(
                &old_absolute_path,
                &new_absolute_path,
                &new_document_path.relative_path,
            )?;
            document_plans.push(DocumentFixPlan {
                new_namespace: new_namespace.clone(),
                old_document: document.clone(),
                new_document,
                old_relative_path: old_document_path.relative_path,
                new_relative_path: new_document_path.relative_path,
            });
        }
    }

    let mut migrations = Vec::new();

    for plan in &namespace_plans {
        fs::rename(&plan.old_path, &plan.new_path).map_err(|error| {
            AppError::fs(
                "fix_namespace_failed",
                "failed to rename namespace during naming_style migration",
                format!("{} -> {}", plan.old_relative_path, plan.new_relative_path),
                &error,
            )
        })?;
        migrations.push(FixMigration {
            old_path: plan.old_relative_path.clone(),
            new_path: plan.new_relative_path.clone(),
        });
    }

    for plan in &document_plans {
        let current_path = build_document_path(
            &workspace.documents_directory_name,
            &plan.new_namespace,
            &plan.old_document,
        );
        let new_path = build_document_path(
            &workspace.documents_directory_name,
            &plan.new_namespace,
            &plan.new_document,
        );
        let current_absolute_path = workspace.root.join(current_path.to_path_buf());
        let new_absolute_path = workspace.root.join(new_path.to_path_buf());

        fs::rename(&current_absolute_path, &new_absolute_path).map_err(|error| {
            AppError::fs(
                "fix_document_failed",
                "failed to rename document during naming_style migration",
                format!(
                    "{} -> {}",
                    current_path.relative_path, new_path.relative_path
                ),
                &error,
            )
        })?;
        migrations.push(FixMigration {
            old_path: plan.old_relative_path.clone(),
            new_path: plan.new_relative_path.clone(),
        });
    }

    Ok(migrations)
}

fn find_unique_document(
    workspace: &workspace::Workspace,
    document_stem: &str,
) -> AppResult<DocumentPath> {
    let mut matches = Vec::new();
    for document in index::scan_documents(workspace)? {
        if document.document != document_stem {
            continue;
        }

        let document_path = build_document_path(
            &workspace.documents_directory_name,
            &document.namespace,
            document_stem,
        );
        matches.push(document_path);
    }

    match matches.len() {
        0 => Err(AppError::validation(
            "document_not_found",
            "document does not exist",
            json!({ "document": document_stem }),
        )),
        1 => Ok(matches.remove(0)),
        _ => Err(AppError::validation(
            "document_ambiguous",
            "document exists in multiple namespaces",
            json!({
                "document": document_stem,
                "matches": matches.into_iter().map(|path| path.relative_path).collect::<Vec<_>>()
            }),
        )),
    }
}

fn document_namespace_config(
    workspace: &workspace::Workspace,
    namespace: Option<&str>,
) -> AppResult<config::NamespaceConfig> {
    if let Some(namespace) = namespace {
        config::read(workspace, namespace)
    } else {
        config::NamespaceConfig::from_workspace(workspace)
    }
}

fn optional_namespace(path: &DocumentPath) -> Option<&str> {
    if path.namespace.is_empty() {
        None
    } else {
        Some(&path.namespace)
    }
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

fn ensure_document_exists(path: &Path, relative_path: &str) -> AppResult<()> {
    if path.is_file() {
        return Ok(());
    }

    Err(AppError::validation(
        "document_not_found",
        "document does not exist",
        json!({ "path": relative_path }),
    ))
}

#[derive(Debug)]
struct NamespaceFixPlan {
    old_path: PathBuf,
    new_path: PathBuf,
    old_relative_path: String,
    new_relative_path: String,
}

#[derive(Debug)]
struct DocumentFixPlan {
    new_namespace: String,
    old_document: String,
    new_document: String,
    old_relative_path: String,
    new_relative_path: String,
}

fn ensure_target_available(source: &Path, target: &Path, relative_target: &str) -> AppResult<()> {
    if !target.exists() || paths_refer_to_same_entry(source, target) {
        return Ok(());
    }

    Err(AppError::validation(
        "fix_conflict",
        "naming_style migration target already exists",
        json!({ "path": relative_target }),
    ))
}

fn paths_refer_to_same_entry(left: &Path, right: &Path) -> bool {
    let Ok(left) = left.canonicalize() else {
        return false;
    };
    let Ok(right) = right.canonicalize() else {
        return false;
    };
    left == right
}
