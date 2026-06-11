use std::io::{self, Write};

use serde_json::Value;

use crate::utils::error::AppError;
use crate::utils::workspace::{InitOutput, WorkspaceRules};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandOutput {
    RawText(String),
    WorkspaceInit(InitOutput),
    WorkspaceRules(WorkspaceRules),
    Namespaces(Vec<NamespaceRow>),
    NamespaceDocuments(Vec<NamespaceDocuments>),
    Documents(Vec<DocumentRow>),
    Fix(Vec<FixRow>),
    Path(String),
    NamespaceName(String),
    Tree(String),
    CheckOk,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamespaceRow {
    pub namespace: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentRow {
    pub document: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamespaceDocuments {
    pub namespace: String,
    pub documents: Vec<DocumentRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixRow {
    pub old_path: String,
    pub new_path: String,
}

pub fn write_success_and_exit(output: &CommandOutput) -> ! {
    let text = format_success(output);
    write_text_and_exit(&text, 0)
}

pub fn write_error_and_exit(error: &AppError) -> ! {
    let mut output = format!("error: {}: {}\n", error.code, error.message);
    append_error_details(&mut output, "details", &error.details);
    write_text_and_exit(&output, error.exit_code)
}

fn write_text_and_exit(output: &str, exit_code: i32) -> ! {
    let mut stdout = io::stdout().lock();
    let result = stdout.write_all(output.as_bytes());

    if result.is_err() {
        std::process::exit(2);
    }

    std::process::exit(exit_code);
}

fn format_success(output: &CommandOutput) -> String {
    match output {
        CommandOutput::RawText(text) => text.clone(),
        CommandOutput::WorkspaceInit(init) => {
            format!(
                "index_path {}\ndoc_dir {}\nnaming_style {}\n",
                init.index_path, init.documents_directory, init.naming_style
            )
        }
        CommandOutput::WorkspaceRules(rules) => format_workspace_rules(rules),
        CommandOutput::Namespaces(namespaces) => {
            let lines = namespaces
                .iter()
                .map(|namespace| format!("{} {}", namespace.namespace, namespace.path))
                .collect::<Vec<_>>();
            format_lines(&lines)
        }
        CommandOutput::NamespaceDocuments(namespaces) => format_namespace_docs(namespaces),
        CommandOutput::Documents(documents) => {
            let lines = documents
                .iter()
                .map(|document| format!("{} {}", document.document, document.path))
                .collect::<Vec<_>>();
            format_lines(&lines)
        }
        CommandOutput::Fix(migrations) => {
            if migrations.is_empty() {
                "OK\n".to_string()
            } else {
                let lines = migrations
                    .iter()
                    .map(|migration| format!("{} -> {}", migration.old_path, migration.new_path))
                    .collect::<Vec<_>>();
                format_lines(&lines)
            }
        }
        CommandOutput::Path(path) => format!("{path}\n"),
        CommandOutput::NamespaceName(namespace) => format!("{namespace}\n"),
        CommandOutput::Tree(tree) => ensure_newline(tree),
        CommandOutput::CheckOk => "OK\n".to_string(),
    }
}

fn format_workspace_rules(rules: &WorkspaceRules) -> String {
    let mut output = String::new();
    output.push_str(&format!("agents_file {}\n", rules.agents_file));
    output.push_str(&format!(
        "documents_directory {}\n",
        rules.documents_directory
    ));
    output.push_str(&format!("index_file {}\n", rules.index_file));
    output.push_str(&format!("repository {}\n", rules.repository));
    for (index, rule) in rules.rules.iter().enumerate() {
        output.push_str(&format!("rules[{index}].content {}\n", rule.content));
        output.push_str(&format!("rules[{index}].path {}\n", rule.path));
    }
    output
}

fn format_namespace_docs(namespaces: &[NamespaceDocuments]) -> String {
    let mut lines = Vec::new();
    for namespace in namespaces {
        lines.push(format!("{}/", namespace.namespace));
        for document in &namespace.documents {
            lines.push(format!("  {} {}", document.document, document.path));
        }
    }

    format_lines(&lines)
}

fn format_lines(lines: &[String]) -> String {
    if lines.is_empty() {
        String::new()
    } else {
        lines.join("\n") + "\n"
    }
}

fn append_error_details(output: &mut String, path: &str, value: &Value) {
    match value {
        Value::Object(map) => {
            let mut keys = map.keys().collect::<Vec<_>>();
            keys.sort();
            for key in keys {
                append_error_details(output, &format!("{path}.{key}"), &map[key]);
            }
        }
        Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                append_error_details(output, &format!("{path}[{index}]"), child);
            }
        }
        Value::String(value) => output.push_str(&format!("{path}: {value}\n")),
        Value::Bool(value) => output.push_str(&format!("{path}: {value}\n")),
        Value::Number(value) => output.push_str(&format!("{path}: {value}\n")),
        Value::Null => {}
    }
}

fn ensure_newline(value: &str) -> String {
    if value.ends_with('\n') {
        value.to_string()
    } else {
        format!("{value}\n")
    }
}
