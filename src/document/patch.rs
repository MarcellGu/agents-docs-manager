use std::io::{self, IsTerminal, Read};

use diffy::patch_set::{FileOperation, FilePatch, ParseOptions, PatchSet};
use serde_json::json;

use crate::utils::error::{AppError, AppResult};

pub fn apply_stdin_patch(original: &str, expected_path: &str) -> AppResult<String> {
    let patch_content = read_stdin_patch()?;
    let patches = parse_patch_set(&patch_content)?;

    let [file_patch] = patches.as_slice() else {
        return Err(AppError::input_with_details(
            "patch_must_target_single_document",
            "patch must contain exactly one file patch",
            json!({ "patches": patches.len() }),
        ));
    };

    if !matches!(file_patch.operation(), FileOperation::Modify { .. }) {
        return Err(AppError::input_with_details(
            "unsupported_patch_operation",
            "patch must modify an existing document",
            json!({ "operation": format!("{:?}", file_patch.operation()) }),
        ));
    }

    ensure_patch_targets_document(file_patch, expected_path)?;

    let text_patch = file_patch.patch().as_text().ok_or_else(|| {
        AppError::input_with_details(
            "unsupported_patch_kind",
            "patch must be a text unified diff",
            json!({ "path": expected_path }),
        )
    })?;

    diffy::apply(original, text_patch).map_err(|error| {
        AppError::validation(
            "patch_apply_failed",
            "patch does not apply to current document",
            json!({
                "path": expected_path,
                "source": error.to_string()
            }),
        )
    })
}

fn read_stdin_patch() -> AppResult<String> {
    let stdin = io::stdin();
    if stdin.is_terminal() {
        return Err(AppError::input(
            "patch_stdin_required",
            "patch content must be provided on stdin",
        ));
    }

    let mut input = String::new();
    stdin.lock().read_to_string(&mut input).map_err(|error| {
        AppError::fs(
            "read_stdin_failed",
            "failed to read patch content from stdin",
            "stdin",
            &error,
        )
    })?;

    if input.trim().is_empty() {
        return Err(AppError::input(
            "empty_patch",
            "patch content must not be empty",
        ));
    }

    Ok(input)
}

fn parse_patch_set(input: &str) -> AppResult<Vec<FilePatch<'_, str>>> {
    let git_error = match PatchSet::parse(input, ParseOptions::gitdiff()).collect::<Result<_, _>>()
    {
        Ok(patches) => return Ok(patches),
        Err(error) => error,
    };

    PatchSet::parse(input, ParseOptions::unidiff())
        .collect::<Result<_, _>>()
        .map_err(|unidiff_error| {
            AppError::input_with_details(
                "invalid_patch",
                "patch is not a valid unified diff",
                json!({
                    "gitdiff_source": git_error.to_string(),
                    "unidiff_source": unidiff_error.to_string()
                }),
            )
        })
}

fn ensure_patch_targets_document(
    file_patch: &FilePatch<'_, str>,
    expected_path: &str,
) -> AppResult<()> {
    let FileOperation::Modify { original, modified } = file_patch.operation() else {
        return Ok(());
    };
    let original = normalize_patch_path(original.as_ref());
    let modified = normalize_patch_path(modified.as_ref());
    if original == expected_path && modified == expected_path {
        return Ok(());
    }

    Err(AppError::input_with_details(
        "patch_target_mismatch",
        "patch must target the selected document",
        json!({
            "expected": expected_path,
            "actual_original": original,
            "actual_modified": modified
        }),
    ))
}

fn normalize_patch_path(path: &str) -> &str {
    path.strip_prefix("a/")
        .or_else(|| path.strip_prefix("b/"))
        .unwrap_or(path)
}
