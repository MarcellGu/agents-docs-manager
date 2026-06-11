use clap::error::ErrorKind;
use clap::{ColorChoice, Parser, Subcommand};

use crate::document;
use crate::namespace;
use crate::utils::error::{AppError, AppResult};
use crate::utils::output::{CommandOutput, DocumentRow, FixRow, NamespaceDocuments, NamespaceRow};
use crate::utils::workspace;

pub fn run() -> AppResult<CommandOutput> {
    let cli = match CommandLine::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            return match err.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
                    Ok(CommandOutput::RawText(err.to_string()))
                }
                _ => Err(AppError::input(
                    "invalid_command",
                    clean_clap_error(err.to_string()),
                )),
            };
        }
    };

    dispatch(cli)
}

fn dispatch(cli: CommandLine) -> AppResult<CommandOutput> {
    match cli.command {
        None => workspace::read_applicable_rules().map(CommandOutput::WorkspaceRules),
        Some(Command::Init) => workspace::init().map(CommandOutput::WorkspaceInit),
        Some(Command::Namespace { action }) => dispatch_namespace(action),
        Some(Command::Docs { action }) => dispatch_docs(action),
        Some(Command::Tree) => document::tree().map(CommandOutput::Tree),
        Some(Command::Sync) => document::sync_index().map(CommandOutput::Path),
        Some(Command::Check) => {
            document::check()?;
            Ok(CommandOutput::CheckOk)
        }
        Some(Command::Fix) => document::fix().map(|migrations| {
            CommandOutput::Fix(
                migrations
                    .into_iter()
                    .map(|migration| FixRow {
                        old_path: migration.old_path,
                        new_path: migration.new_path,
                    })
                    .collect(),
            )
        }),
    }
}

fn dispatch_namespace(action: NamespaceAction) -> AppResult<CommandOutput> {
    match action {
        NamespaceAction::List => namespace::list().map(|namespaces| {
            CommandOutput::Namespaces(
                namespaces
                    .into_iter()
                    .map(|namespace| NamespaceRow {
                        namespace: namespace.namespace,
                        path: namespace.path,
                    })
                    .collect(),
            )
        }),
        NamespaceAction::Create {
            namespace_name,
            document_name_regex,
        } => namespace::create(&namespace_name, document_name_regex.as_deref())
            .map(|namespace| CommandOutput::Path(namespace.path)),
        NamespaceAction::ListDocs => namespace::list_docs().map(|namespaces| {
            CommandOutput::NamespaceDocuments(
                namespaces
                    .into_iter()
                    .map(|namespace| NamespaceDocuments {
                        namespace: namespace.namespace,
                        documents: namespace
                            .documents
                            .into_iter()
                            .map(|document| DocumentRow {
                                document: document.document,
                                path: document.path,
                            })
                            .collect(),
                    })
                    .collect(),
            )
        }),
        NamespaceAction::Rename {
            origin_name,
            new_name,
        } => namespace::rename(&origin_name, &new_name)
            .map(|namespace| CommandOutput::Path(namespace.path)),
        NamespaceAction::Delete {
            namespace_name,
            delete_docs,
        } => namespace::delete(&namespace_name, delete_docs)
            .map(|namespace| CommandOutput::NamespaceName(namespace.namespace)),
    }
}

fn dispatch_docs(action: DocsAction) -> AppResult<CommandOutput> {
    match action {
        DocsAction::List { namespace } => document::list(namespace.as_deref()).map(|documents| {
            CommandOutput::Documents(
                documents
                    .into_iter()
                    .map(|document| DocumentRow {
                        document: document.document,
                        path: document.path,
                    })
                    .collect(),
            )
        }),
        DocsAction::Create {
            doc_name,
            namespace,
            markdown_content,
        } => document::create(namespace.as_deref(), &doc_name, &markdown_content)
            .map(|document| CommandOutput::Path(document.path)),
        DocsAction::Rename {
            origin_doc_name,
            new_doc_name,
        } => document::rename_unique(&origin_doc_name, &new_doc_name)
            .map(|document| CommandOutput::Path(document.new_path)),
        DocsAction::Patch {
            doc_name,
            namespace,
        } => document::patch(namespace.as_deref(), &doc_name)
            .map(|document| CommandOutput::Path(document.path)),
        DocsAction::Delete {
            namespace,
            doc_name,
        } => document::delete(namespace.as_deref(), &doc_name)
            .map(|document| CommandOutput::Path(document.path)),
    }
}

fn clean_clap_error(message: String) -> String {
    message.trim().to_string()
}

#[derive(Debug, Parser)]
#[command(
    name = "adm",
    version,
    about = "Manage docs.json and repository-local markdown docs.",
    disable_help_subcommand = true,
    color = ColorChoice::Never
)]
pub struct CommandLine {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(about = "Initialize docs.json and the managed docs directory")]
    Init,
    #[command(about = "Manage namespace directories under doc_dir")]
    Namespace {
        #[command(subcommand)]
        action: NamespaceAction,
    },
    #[command(about = "Manage markdown documents under doc_dir")]
    Docs {
        #[command(subcommand)]
        action: DocsAction,
    },
    #[command(about = "Print the managed docs tree")]
    Tree,
    #[command(about = "Sync the managed docs index to AGENTS.md")]
    Sync,
    #[command(about = "Validate naming_style and markdown metadata")]
    Check,
    #[command(about = "Migrate namespace and document names to naming_style")]
    Fix,
}

#[derive(Debug, Subcommand)]
pub enum NamespaceAction {
    #[command(about = "List namespaces")]
    List,
    #[command(about = "Create a namespace")]
    Create {
        namespace_name: String,
        #[arg(long, hide = true)]
        document_name_regex: Option<String>,
    },
    #[command(about = "List documents grouped by namespace")]
    ListDocs,
    #[command(about = "Rename a namespace")]
    Rename {
        origin_name: String,
        new_name: String,
    },
    #[command(about = "Delete a namespace")]
    Delete {
        namespace_name: String,
        #[arg(long)]
        delete_docs: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum DocsAction {
    #[command(about = "List documents")]
    List {
        #[arg(long)]
        namespace: Option<String>,
    },
    #[command(about = "Create a markdown document")]
    Create {
        doc_name: String,
        #[arg(long)]
        namespace: Option<String>,
        markdown_content: String,
    },
    #[command(about = "Rename a document by unique document name")]
    Rename {
        origin_doc_name: String,
        new_doc_name: String,
    },
    #[command(about = "Apply a single-file unified diff from stdin")]
    Patch {
        doc_name: String,
        #[arg(long)]
        namespace: Option<String>,
    },
    #[command(about = "Delete a document")]
    Delete {
        #[arg(long)]
        namespace: Option<String>,
        doc_name: String,
    },
}
