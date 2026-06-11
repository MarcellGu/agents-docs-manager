#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentMetadata {
    pub title: String,
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentMetadataError {
    MissingTitle,
    MissingDescription,
    InvalidDescription,
}

impl DocumentMetadataError {
    pub fn message(self) -> &'static str {
        match self {
            Self::MissingTitle => "document must start with a level-one title",
            Self::MissingDescription => {
                "document must include a metadata paragraph after the title"
            }
            Self::InvalidDescription => {
                "document metadata paragraph must match: 本文说明<description>。"
            }
        }
    }
}

pub fn extract_metadata(content: &str) -> Result<DocumentMetadata, DocumentMetadataError> {
    let mut lines = content.lines();
    let title = lines
        .next()
        .and_then(extract_title_from_line)
        .ok_or(DocumentMetadataError::MissingTitle)?;

    let description =
        extract_metadata_paragraph(lines).ok_or(DocumentMetadataError::MissingDescription)?;
    if !is_valid_metadata_description(&description) {
        return Err(DocumentMetadataError::InvalidDescription);
    }

    Ok(DocumentMetadata { title, description })
}

fn extract_title_from_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    trimmed
        .strip_prefix("# ")
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(ToOwned::to_owned)
}

fn extract_metadata_paragraph<'a>(lines: impl IntoIterator<Item = &'a str>) -> Option<String> {
    let mut paragraph = Vec::new();
    let mut in_paragraph = false;

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if in_paragraph {
                break;
            }
            continue;
        }

        in_paragraph = true;
        paragraph.push(trimmed);
    }

    if paragraph.is_empty() {
        None
    } else {
        Some(paragraph.join(" "))
    }
}

fn is_valid_metadata_description(description: &str) -> bool {
    description.starts_with("本文说明")
        && description.ends_with('。')
        && description
            .trim_start_matches("本文说明")
            .trim_end_matches('。')
            .trim()
            .chars()
            .next()
            .is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_required_title_and_metadata_description() {
        let content = "# 代码规范\n\n本文说明 Rust 代码规范。\n\n## 适用范围\n";
        let metadata = extract_metadata(content).unwrap();
        assert_eq!(metadata.title, "代码规范");
        assert_eq!(metadata.description, "本文说明 Rust 代码规范。");
    }

    #[test]
    fn rejects_freeform_description() {
        let content = "# CODE_STYLE\n\nRust code style conventions.\n";
        let error = extract_metadata(content).unwrap_err();
        assert_eq!(error, DocumentMetadataError::InvalidDescription);
    }
}
