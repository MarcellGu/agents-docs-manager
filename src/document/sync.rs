use std::collections::BTreeMap;
use std::fs;

use serde_json::json;

use crate::utils::error::{AppError, AppResult};
use crate::utils::index::{self, DocumentEntry};
use crate::utils::workspace;

const START_MARKER: &str = "<!-- adm:docs-index:start -->";
const END_MARKER: &str = "<!-- adm:docs-index:end -->";

pub fn sync_index() -> AppResult<String> {
    let workspace = workspace::find()?;
    let documents = index::scan_documents(&workspace)?;
    let block = render_index_block(&workspace.documents_directory_name, &documents);
    let path = workspace.agents_file.clone();
    let relative_path = workspace::relative_path(&workspace, &path);

    let existing = if path.exists() {
        Some(fs::read_to_string(&path).map_err(|error| {
            AppError::fs(
                "read_index_file_failed",
                "failed to read index file",
                relative_path.clone(),
                &error,
            )
        })?)
    } else {
        None
    };

    let content = merge_index_block(existing.as_deref(), &block, &relative_path)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            AppError::fs(
                "create_index_directory_failed",
                "failed to create index file directory",
                workspace::relative_path(&workspace, parent),
                &error,
            )
        })?;
    }

    fs::write(&path, content).map_err(|error| {
        AppError::fs(
            "write_index_file_failed",
            "failed to write index file",
            relative_path.clone(),
            &error,
        )
    })?;

    Ok(relative_path)
}

fn render_index_block(documents_directory: &str, documents: &[DocumentEntry]) -> String {
    let mut output = String::new();
    output.push_str(START_MARKER);
    output.push_str("\n## Docs Index\n\n");
    output.push_str(&format!("Source: `{documents_directory}/`\n\n"));

    let root_documents = documents
        .iter()
        .filter(|document| document.namespace.is_empty())
        .collect::<Vec<_>>();
    let mut namespaces = BTreeMap::<&str, Vec<&DocumentEntry>>::new();
    for document in documents
        .iter()
        .filter(|document| !document.namespace.is_empty())
    {
        namespaces
            .entry(document.namespace.as_str())
            .or_default()
            .push(document);
    }

    if root_documents.is_empty() && namespaces.is_empty() {
        output.push_str("No documents found.\n\n");
    }

    if !root_documents.is_empty() {
        output.push_str("### Root Documents\n\n");
        push_document_lines(&mut output, &root_documents);
    }

    for (namespace, namespace_documents) in namespaces {
        output.push_str(&format!("### {namespace}\n\n"));
        push_document_lines(&mut output, &namespace_documents);
    }

    output.push_str(END_MARKER);
    output.push('\n');
    output
}

fn push_document_lines(output: &mut String, documents: &[&DocumentEntry]) {
    for document in documents {
        output.push_str(&format!(
            "- `{}` - {} - {}\n",
            document.path, document.title, document.description
        ));
    }
    output.push('\n');
}

fn merge_index_block(
    existing: Option<&str>,
    block: &str,
    relative_path: &str,
) -> AppResult<String> {
    let Some(existing) = existing else {
        return Ok(block.to_string());
    };

    let start = existing.find(START_MARKER);
    let end = existing.find(END_MARKER);

    match (start, end) {
        (None, None) => {
            let existing = existing.trim_end();
            if existing.is_empty() {
                Ok(block.to_string())
            } else {
                Ok(format!("{existing}\n\n{block}"))
            }
        }
        (Some(start), Some(end)) if start <= end => {
            let after_start = end + END_MARKER.len();
            let after = existing[after_start..]
                .strip_prefix('\n')
                .unwrap_or(&existing[after_start..]);
            Ok(format!("{}{}{}", &existing[..start], block, after))
        }
        _ => Err(AppError::validation(
            "invalid_index_file_markers",
            "index file contains invalid adm sync markers",
            json!({
                "path": relative_path,
                "start_marker": START_MARKER,
                "end_marker": END_MARKER
            }),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_existing_managed_block() {
        let existing =
            "Manual\n\n<!-- adm:docs-index:start -->\nold\n<!-- adm:docs-index:end -->\n\nFooter\n";
        let merged = merge_index_block(Some(existing), "new\n", "AGENTS.md").unwrap();

        assert_eq!(merged, "Manual\n\nnew\n\nFooter\n");
    }

    #[test]
    fn appends_when_existing_file_has_no_managed_block() {
        let merged = merge_index_block(Some("Manual\n"), "index\n", "AGENTS.md").unwrap();

        assert_eq!(merged, "Manual\n\nindex\n");
    }
}
