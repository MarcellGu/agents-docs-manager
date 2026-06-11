use serde_json::json;

use crate::namespace::config;
use crate::utils::error::{AppError, AppResult};
use crate::utils::index;
use crate::utils::workspace;

pub fn tree() -> AppResult<String> {
    let workspace = workspace::find()?;
    let documents = index::scan_documents(&workspace)?;
    let namespaces = config::list_namespaces(&workspace)?;
    let root_documents = documents
        .iter()
        .filter(|document| document.namespace.is_empty())
        .collect::<Vec<_>>();
    let mut lines = Vec::new();

    lines.push(format!("{}/", workspace.documents_directory_name));

    let top_level_count = root_documents.len() + namespaces.len();
    let mut top_level_index = 0usize;

    for document in root_documents {
        top_level_index += 1;
        let document_connector = if top_level_index == top_level_count {
            "`-- "
        } else {
            "|-- "
        };
        lines.push(format!(
            "{document_connector}{}.md - {}",
            document.document, document.description
        ));
    }

    for namespace in &namespaces {
        top_level_index += 1;
        let is_last_namespace = top_level_index == top_level_count;
        let namespace_connector = if is_last_namespace { "`-- " } else { "|-- " };
        lines.push(format!("{namespace_connector}{namespace}/"));

        let namespace_documents = documents
            .iter()
            .filter(|document| document.namespace == *namespace)
            .collect::<Vec<_>>();

        for (document_index, document) in namespace_documents.iter().enumerate() {
            let namespace_prefix = if is_last_namespace { "    " } else { "|   " };
            let document_connector = if document_index + 1 == namespace_documents.len() {
                "`-- "
            } else {
                "|-- "
            };
            lines.push(format!(
                "{namespace_prefix}{document_connector}{}.md - {}",
                document.document, document.description
            ));
        }
    }

    Ok(lines.join("\n"))
}

pub fn check() -> AppResult<()> {
    let workspace = workspace::find()?;
    index::scan_documents(&workspace)?;
    let invalid_namespaces = config::all_invalid_namespaces(&workspace)?;
    let invalid_documents = config::all_invalid_documents(&workspace)?;

    if invalid_namespaces.is_empty() && invalid_documents.is_empty() {
        return Ok(());
    }

    Err(AppError::validation(
        "check_failed",
        "repository documentation checks failed",
        json!({
            "invalid_namespaces": invalid_namespaces,
            "invalid_documents": invalid_documents
        }),
    ))
}
