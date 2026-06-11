use std::fs;
use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use tempfile::TempDir;

struct TestRepo {
    _temp: TempDir,
    path: std::path::PathBuf,
}

impl TestRepo {
    fn path(&self) -> &Path {
        &self.path
    }
}

fn raw_repo() -> TestRepo {
    raw_repo_named("agents_docs_manager")
}

fn raw_repo_named(name: &str) -> TestRepo {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join(name);
    fs::create_dir(&path).unwrap();
    fs::create_dir(path.join(".git")).unwrap();
    TestRepo { _temp: temp, path }
}

fn repo() -> TestRepo {
    let temp = raw_repo();
    let output = run_cli(temp.path(), &["init"], 0);
    assert_line(&output, "index_path docs.json");
    assert_line(&output, "doc_dir ./docs");
    assert_line(&output, "naming_style snake_case");
    assert!(temp.path().join("docs.json").is_file());
    assert!(!temp.path().join(".doc.root").exists());
    assert!(!temp.path().join(".adm.json").exists());
    temp
}

fn run_cli(repo: &Path, args: &[&str], code: i32) -> String {
    let mut command = Command::cargo_bin("adm").unwrap();
    command.current_dir(repo).args(args);
    let assert = command.assert().code(code);
    String::from_utf8(assert.get_output().stdout.clone()).unwrap()
}

fn run_cli_with_stdin(repo: &Path, args: &[&str], stdin: &str, code: i32) -> String {
    let mut command = Command::cargo_bin("adm").unwrap();
    command.current_dir(repo).args(args).write_stdin(stdin);
    let assert = command.assert().code(code);
    String::from_utf8(assert.get_output().stdout.clone()).unwrap()
}

fn assert_line(output: &str, expected: &str) {
    assert!(
        output.lines().any(|line| line == expected),
        "missing line: {expected}\nstdout:\n{output}"
    );
}

fn read_docs_json(repo: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(repo.join("docs.json")).unwrap()).unwrap()
}

#[test]
fn help_uses_adm_command() {
    let mut command = Command::cargo_bin("adm").unwrap();
    command
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("adm"))
        .stdout(predicates::str::contains("init"))
        .stdout(predicates::str::contains(
            "Initialize docs.json and the managed docs directory",
        ))
        .stdout(predicates::str::contains("namespace"))
        .stdout(predicates::str::contains(
            "Manage namespace directories under doc_dir",
        ))
        .stdout(predicates::str::contains(
            "Migrate namespace and document names to naming_style",
        ))
        .stdout(predicates::str::contains(
            "Sync the managed docs index to AGENTS.md",
        ))
        .stdout(predicates::str::contains("docs"));
}

#[test]
fn namespace_create_help_hides_unsupported_regex_option() {
    let mut command = Command::cargo_bin("adm").unwrap();
    command
        .args(["namespace", "create", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Usage: adm namespace create"))
        .stdout(predicates::str::contains("--document-name-regex").not());
}

#[test]
fn init_uses_most_common_first_level_directory_naming_style() {
    let repo = raw_repo();
    fs::create_dir(repo.path().join("src")).unwrap();
    fs::create_dir(repo.path().join("tests")).unwrap();
    fs::write(repo.path().join("CloudEngine"), "not a directory").unwrap();

    let output = run_cli(repo.path(), &["init"], 0);
    assert_line(&output, "index_path docs.json");
    assert_line(&output, "doc_dir ./docs");
    assert_line(&output, "naming_style snake_case");
    let docs_json = read_docs_json(repo.path());
    assert_eq!(docs_json["naming_style"], "snake_case");
    assert!(repo.path().join("docs.json").is_file());
    assert!(!repo.path().join(".doc.root").exists());
    assert!(!repo.path().join(".adm.json").exists());
    assert!(repo.path().join("docs").is_dir());

    let namespace = run_cli(repo.path(), &["namespace", "create", "conventions"], 0);
    assert_line(&namespace, "docs/conventions");

    let namespace_list = run_cli(repo.path(), &["namespace", "list"], 0);
    assert_line(&namespace_list, "conventions docs/conventions");
}

#[test]
fn commands_require_init_marker() {
    let repo = raw_repo();
    let output = run_cli(repo.path(), &["namespace", "list"], 2);
    assert_line(
        &output,
        "error: workspace_not_initialized: failed to locate docs.json; run adm init first",
    );
}

#[test]
fn docs_json_rejects_unknown_fields() {
    let repo = raw_repo();
    fs::write(
        repo.path().join("docs.json"),
        r#"{
  "index_file": "./AGENTS.md",
  "doc_dir": "./docs",
  "naming_style": "snake_case",
  "namespaces": {}
}
"#,
    )
    .unwrap();

    let output = run_cli(repo.path(), &["namespace", "list"], 2);
    assert_line(
        &output,
        "error: invalid_workspace_index: docs.json is not valid JSON",
    );
}

#[test]
fn docs_json_rejects_old_doc_type_field() {
    let repo = raw_repo();
    fs::write(
        repo.path().join("docs.json"),
        r#"{
  "index_file": "./AGENTS.md",
  "doc_dir": "./docs",
  "doc_type": "snake_case"
}
"#,
    )
    .unwrap();

    let output = run_cli(repo.path(), &["namespace", "list"], 2);
    assert_line(
        &output,
        "error: invalid_workspace_index: docs.json is not valid JSON",
    );
}

#[test]
fn init_uses_pascal_case_from_first_level_directories() {
    let repo = raw_repo();
    fs::create_dir(repo.path().join("Source")).unwrap();
    fs::create_dir(repo.path().join("Tests")).unwrap();
    fs::write(repo.path().join("cloud_engine"), "not a directory").unwrap();

    let output = run_cli(repo.path(), &["init"], 0);
    assert_line(&output, "doc_dir ./Docs");
    assert_line(&output, "naming_style PascalCase");
    let docs_json = read_docs_json(repo.path());
    assert_eq!(docs_json["doc_dir"], "./Docs");
    assert_eq!(docs_json["naming_style"], "PascalCase");

    let namespace = run_cli(repo.path(), &["namespace", "create", "Conventions"], 0);
    assert_line(&namespace, "Docs/Conventions");
}

#[test]
fn init_uses_kebab_case_from_first_level_directories() {
    let repo = raw_repo();
    fs::create_dir(repo.path().join("project-docs")).unwrap();
    fs::create_dir(repo.path().join("api-guides")).unwrap();
    fs::create_dir(repo.path().join("src")).unwrap();

    let output = run_cli(repo.path(), &["init"], 0);
    assert_line(&output, "doc_dir ./docs");
    assert_line(&output, "naming_style kebab-case");
    let docs_json = read_docs_json(repo.path());
    assert_eq!(docs_json["doc_dir"], "./docs");
    assert_eq!(docs_json["naming_style"], "kebab-case");

    let namespace = run_cli(repo.path(), &["namespace", "create", "project-docs"], 0);
    assert_line(&namespace, "docs/project-docs");
}

#[test]
fn commands_find_root_from_subdirectory_using_docs_json() {
    let repo = repo();
    fs::create_dir(repo.path().join("src")).unwrap();
    fs::create_dir(repo.path().join("src/nested")).unwrap();
    run_cli(repo.path(), &["namespace", "create", "conventions"], 0);

    let output = run_cli(&repo.path().join("src/nested"), &["namespace", "list"], 0);
    assert_line(&output, "conventions docs/conventions");
}

#[test]
fn namespace_and_docs_lifecycle_uses_cli_args() {
    let repo = repo();

    let namespace = run_cli(repo.path(), &["namespace", "create", "conventions"], 0);
    assert_line(&namespace, "docs/conventions");
    assert!(
        !repo
            .path()
            .join("docs/conventions/.adm-namespace.json")
            .exists()
    );
    let docs_json = read_docs_json(repo.path());
    assert_eq!(docs_json["index_file"], "./AGENTS.md");
    assert_eq!(docs_json["doc_dir"], "./docs");
    assert_eq!(docs_json["naming_style"], "snake_case");
    assert_eq!(docs_json.as_object().unwrap().len(), 3);

    let namespace_list = run_cli(repo.path(), &["namespace", "list"], 0);
    assert_line(&namespace_list, "conventions docs/conventions");

    let document = run_cli(
        repo.path(),
        &[
            "docs",
            "create",
            "code_style",
            "--namespace",
            "conventions",
            "# code_style\n\n本文说明 Rust code style conventions。\n",
        ],
        0,
    );
    assert_line(&document, "docs/conventions/code_style.md");
    assert!(repo.path().join("docs/conventions/code_style.md").is_file());
    let docs_json = read_docs_json(repo.path());
    assert_eq!(docs_json.as_object().unwrap().len(), 3);

    let docs_list = run_cli(
        repo.path(),
        &["docs", "list", "--namespace", "conventions"],
        0,
    );
    assert_line(&docs_list, "code_style docs/conventions/code_style.md");

    let namespace_docs = run_cli(repo.path(), &["namespace", "list-docs"], 0);
    assert_line(&namespace_docs, "conventions/");
    assert_line(
        &namespace_docs,
        "  code_style docs/conventions/code_style.md",
    );

    let tree = run_cli(repo.path(), &["tree"], 0);
    assert_line(&tree, "docs/");
    assert_line(&tree, "`-- conventions/");
    assert_line(
        &tree,
        "    `-- code_style.md - 本文说明 Rust code style conventions。",
    );

    let renamed_doc = run_cli(
        repo.path(),
        &["docs", "rename", "code_style", "rust_style"],
        0,
    );
    assert_line(&renamed_doc, "docs/conventions/rust_style.md");
    assert!(repo.path().join("docs/conventions/rust_style.md").is_file());

    let deleted_doc = run_cli(
        repo.path(),
        &["docs", "delete", "--namespace", "conventions", "rust_style"],
        0,
    );
    assert_line(&deleted_doc, "docs/conventions/rust_style.md");
    assert!(!repo.path().join("docs/conventions/rust_style.md").exists());

    let renamed_namespace = run_cli(
        repo.path(),
        &["namespace", "rename", "conventions", "engineering"],
        0,
    );
    assert_line(&renamed_namespace, "docs/engineering");
    assert!(repo.path().join("docs/engineering").is_dir());

    let deleted_namespace = run_cli(repo.path(), &["namespace", "delete", "engineering"], 0);
    assert_line(&deleted_namespace, "engineering");

    let docs_json = read_docs_json(repo.path());
    assert_eq!(docs_json.as_object().unwrap().len(), 3);
}

#[test]
fn root_docs_lifecycle_omits_namespace_arg() {
    let repo = repo();

    let document = run_cli(
        repo.path(),
        &[
            "docs",
            "create",
            "overview",
            "# overview\n\n本文说明 Root overview。\n",
        ],
        0,
    );
    assert_line(&document, "docs/overview.md");
    assert!(repo.path().join("docs/overview.md").is_file());

    let docs_list = run_cli(repo.path(), &["docs", "list"], 0);
    assert_line(&docs_list, "overview docs/overview.md");

    let tree = run_cli(repo.path(), &["tree"], 0);
    assert_line(&tree, "docs/");
    assert_line(&tree, "`-- overview.md - 本文说明 Root overview。");

    let output = run_cli_with_stdin(
        repo.path(),
        &["docs", "patch", "overview"],
        "\
--- a/docs/overview.md
+++ b/docs/overview.md
@@ -1,3 +1,3 @@
 # overview

-本文说明 Root overview。
+本文说明 Root overview document。
",
        0,
    );
    assert_line(&output, "docs/overview.md");

    let check = run_cli(repo.path(), &["check"], 0);
    assert_line(&check, "OK");

    let deleted = run_cli(repo.path(), &["docs", "delete", "overview"], 0);
    assert_line(&deleted, "docs/overview.md");
    assert!(!repo.path().join("docs/overview.md").exists());
}

#[test]
fn sync_creates_agents_md_with_docs_index() {
    let repo = repo();
    run_cli(
        repo.path(),
        &[
            "docs",
            "create",
            "overview",
            "# overview\n\n本文说明 Root overview。\n",
        ],
        0,
    );
    run_cli(repo.path(), &["namespace", "create", "cli"], 0);
    run_cli(
        repo.path(),
        &[
            "docs",
            "create",
            "reference",
            "--namespace",
            "cli",
            "# reference\n\n本文说明 CLI reference。\n",
        ],
        0,
    );

    let output = run_cli(repo.path(), &["sync"], 0);
    assert_line(&output, "AGENTS.md");

    let content = fs::read_to_string(repo.path().join("AGENTS.md")).unwrap();
    assert!(content.contains("<!-- adm:docs-index:start -->"));
    assert!(content.contains("## Docs Index"));
    assert!(content.contains("Source: `docs/`"));
    assert!(content.contains("### Root Documents"));
    assert!(content.contains("- `docs/overview.md` - overview - 本文说明 Root overview。"));
    assert!(content.contains("### cli"));
    assert!(content.contains("- `docs/cli/reference.md` - reference - 本文说明 CLI reference。"));
    assert!(content.contains("<!-- adm:docs-index:end -->"));
}

#[test]
fn sync_replaces_managed_agents_block_and_preserves_manual_content() {
    let repo = repo();
    run_cli(
        repo.path(),
        &[
            "docs",
            "create",
            "overview",
            "# overview\n\n本文说明 Root overview。\n",
        ],
        0,
    );
    fs::write(
        repo.path().join("AGENTS.md"),
        "\
Manual intro.

<!-- adm:docs-index:start -->
old index
<!-- adm:docs-index:end -->

Manual footer.
",
    )
    .unwrap();

    let output = run_cli(repo.path(), &["sync"], 0);
    assert_line(&output, "AGENTS.md");

    let content = fs::read_to_string(repo.path().join("AGENTS.md")).unwrap();
    assert!(content.starts_with("Manual intro.\n\n"));
    assert!(content.contains("Manual footer."));
    assert!(!content.contains("old index"));
    assert!(content.contains("- `docs/overview.md` - overview - 本文说明 Root overview。"));
}

#[test]
fn namespace_delete_requires_delete_docs_for_non_empty_namespace() {
    let repo = repo();
    run_cli(repo.path(), &["namespace", "create", "conventions"], 0);
    run_cli(
        repo.path(),
        &[
            "docs",
            "create",
            "code_style",
            "--namespace",
            "conventions",
            "# code_style\n\n本文说明 Rust code style conventions。\n",
        ],
        0,
    );

    let output = run_cli(repo.path(), &["namespace", "delete", "conventions"], 1);
    assert_line(
        &output,
        "error: namespace_not_empty: namespace is not empty",
    );

    let deleted = run_cli(
        repo.path(),
        &["namespace", "delete", "conventions", "--delete-docs"],
        0,
    );
    assert_line(&deleted, "conventions");
    assert!(!repo.path().join("docs/conventions").exists());
}

#[test]
fn docs_patch_applies_single_unified_diff_from_stdin() {
    let repo = repo();
    run_cli(repo.path(), &["namespace", "create", "conventions"], 0);
    run_cli(
        repo.path(),
        &[
            "docs",
            "create",
            "code_style",
            "--namespace",
            "conventions",
            "# code_style\n\n本文说明 Rust code style conventions。\n",
        ],
        0,
    );

    let output = run_cli_with_stdin(
        repo.path(),
        &["docs", "patch", "code_style", "--namespace", "conventions"],
        "\
--- a/docs/conventions/code_style.md
+++ b/docs/conventions/code_style.md
@@ -1,3 +1,3 @@
 # code_style

-本文说明 Rust code style conventions。
+本文说明 Rust code style guidelines。
",
        0,
    );

    assert_line(&output, "docs/conventions/code_style.md");
    let content = fs::read_to_string(repo.path().join("docs/conventions/code_style.md")).unwrap();
    assert!(content.contains("本文说明 Rust code style guidelines。"));

    fs::write(
        repo.path().join("README.md"),
        "# README\n\nRoot README is outside the managed docs directory.\n",
    )
    .unwrap();
    let check = run_cli(repo.path(), &["check"], 0);
    assert_line(&check, "OK");
}

#[test]
fn fix_migrates_namespaces_and_documents_after_naming_style_change() {
    let repo = raw_repo_named("CloudEngine");
    fs::create_dir(repo.path().join("Source")).unwrap();
    fs::create_dir(repo.path().join("Tests")).unwrap();
    run_cli(repo.path(), &["init"], 0);
    run_cli(repo.path(), &["namespace", "create", "Conventions"], 0);
    run_cli(
        repo.path(),
        &[
            "docs",
            "create",
            "CodeStyle",
            "--namespace",
            "Conventions",
            "# CodeStyle\n\n本文说明 Rust code style conventions。\n",
        ],
        0,
    );
    run_cli(
        repo.path(),
        &[
            "docs",
            "create",
            "RootGuide",
            "# RootGuide\n\n本文说明 Root guide。\n",
        ],
        0,
    );
    fs::write(
        repo.path().join("docs.json"),
        r#"{
  "index_file": "./AGENTS.md",
  "doc_dir": "./Docs",
  "naming_style": "snake_case"
}
"#,
    )
    .unwrap();

    let check = run_cli(repo.path(), &["check"], 1);
    assert_line(
        &check,
        "error: check_failed: repository documentation checks failed",
    );
    assert_line(&check, "details.invalid_documents[0].document: RootGuide");
    assert_line(&check, "details.invalid_documents[1].document: CodeStyle");
    assert_line(
        &check,
        "details.invalid_namespaces[0].namespace: Conventions",
    );

    let fix = run_cli(repo.path(), &["fix"], 0);
    assert_line(&fix, "Docs/RootGuide.md -> Docs/root_guide.md");
    assert_line(&fix, "Docs/Conventions -> Docs/conventions");
    assert_line(
        &fix,
        "Docs/Conventions/CodeStyle.md -> Docs/conventions/code_style.md",
    );
    assert!(repo.path().join("Docs/conventions/code_style.md").is_file());
    let namespace_list = run_cli(repo.path(), &["namespace", "list"], 0);
    assert_line(&namespace_list, "conventions Docs/conventions");
    let docs_list = run_cli(repo.path(), &["docs", "list"], 0);
    assert_line(&docs_list, "root_guide Docs/root_guide.md");
    assert_line(&docs_list, "code_style Docs/conventions/code_style.md");

    let check = run_cli(repo.path(), &["check"], 0);
    assert_line(&check, "OK");
}

#[test]
fn fix_outputs_ok_when_no_migration_is_needed() {
    let repo = repo();
    run_cli(repo.path(), &["namespace", "create", "conventions"], 0);
    run_cli(
        repo.path(),
        &[
            "docs",
            "create",
            "code_style",
            "--namespace",
            "conventions",
            "# code_style\n\n本文说明 Rust code style conventions。\n",
        ],
        0,
    );

    let output = run_cli(repo.path(), &["fix"], 0);
    assert_line(&output, "OK");
}

#[test]
fn docs_patch_rejects_when_context_does_not_match() {
    let repo = repo();
    run_cli(repo.path(), &["namespace", "create", "conventions"], 0);
    run_cli(
        repo.path(),
        &[
            "docs",
            "create",
            "code_style",
            "--namespace",
            "conventions",
            "# code_style\n\n本文说明 Rust code style conventions。\n",
        ],
        0,
    );

    let output = run_cli_with_stdin(
        repo.path(),
        &["docs", "patch", "code_style", "--namespace", "conventions"],
        "\
--- a/docs/conventions/code_style.md
+++ b/docs/conventions/code_style.md
@@ -1 +1 @@
-# WRONG
+# RIGHT
",
        1,
    );

    assert_line(
        &output,
        "error: patch_apply_failed: patch does not apply to current document",
    );
}

#[test]
fn docs_patch_rejects_multi_file_patch() {
    let repo = repo();
    run_cli(repo.path(), &["namespace", "create", "conventions"], 0);
    run_cli(
        repo.path(),
        &[
            "docs",
            "create",
            "code_style",
            "--namespace",
            "conventions",
            "# code_style\n\n本文说明 Rust code style conventions。\n",
        ],
        0,
    );

    let output = run_cli_with_stdin(
        repo.path(),
        &["docs", "patch", "code_style", "--namespace", "conventions"],
        "\
--- a/docs/conventions/code_style.md
+++ b/docs/conventions/code_style.md
@@ -1 +1 @@
-# code_style
+# Code Style
--- a/docs/conventions/other.md
+++ b/docs/conventions/other.md
@@ -1 +1 @@
-# other
+# Other
",
        2,
    );

    assert_line(
        &output,
        "error: patch_must_target_single_document: patch must contain exactly one file patch",
    );
}

#[test]
fn docs_patch_rejects_target_path_mismatch() {
    let repo = repo();
    run_cli(repo.path(), &["namespace", "create", "conventions"], 0);
    run_cli(
        repo.path(),
        &[
            "docs",
            "create",
            "code_style",
            "--namespace",
            "conventions",
            "# code_style\n\n本文说明 Rust code style conventions。\n",
        ],
        0,
    );

    let output = run_cli_with_stdin(
        repo.path(),
        &["docs", "patch", "code_style", "--namespace", "conventions"],
        "\
--- a/docs/conventions/other.md
+++ b/docs/conventions/other.md
@@ -1,3 +1,3 @@
 # code_style

-本文说明 Rust code style conventions。
+本文说明 Rust code style guidelines。
",
        2,
    );

    assert_line(
        &output,
        "error: patch_target_mismatch: patch must target the selected document",
    );
    let content = fs::read_to_string(repo.path().join("docs/conventions/code_style.md")).unwrap();
    assert!(content.contains("本文说明 Rust code style conventions。"));
}

#[test]
fn rejects_docs_create_when_name_does_not_match_naming_style() {
    let repo = repo();
    run_cli(repo.path(), &["namespace", "create", "conventions"], 0);

    let output = run_cli(
        repo.path(),
        &[
            "docs",
            "create",
            "code-style",
            "--namespace",
            "conventions",
            "# code-style\n",
        ],
        1,
    );

    assert_line(
        &output,
        "error: invalid_document_name: document name does not match naming_style",
    );
    assert!(!repo.path().join("docs/conventions/code-style.md").exists());
}

#[test]
fn namespace_create_rejects_name_that_does_not_match_naming_style() {
    let repo = repo();

    let output = run_cli(repo.path(), &["namespace", "create", "Conventions"], 1);
    assert_line(
        &output,
        "error: invalid_namespace_name: namespace name does not match naming_style",
    );
    assert!(!repo.path().join("docs/Conventions").exists());
}

#[test]
fn check_rejects_existing_namespace_that_does_not_match_naming_style() {
    let repo = repo();
    fs::create_dir(repo.path().join("docs/Conventions")).unwrap();

    let output = run_cli(repo.path(), &["check"], 1);
    assert_line(
        &output,
        "error: check_failed: repository documentation checks failed",
    );
    assert_line(
        &output,
        "details.invalid_namespaces[0].namespace: Conventions",
    );
}

#[test]
fn namespace_create_rejects_namespace_specific_regex() {
    let repo = repo();
    let output = run_cli(
        repo.path(),
        &[
            "namespace",
            "create",
            "conventions",
            "--document-name-regex",
            "[A-Z][A-Z0-9_]*",
        ],
        2,
    );

    assert_line(
        &output,
        "error: unsupported_namespace_regex: namespace-specific document_name_regex is no longer supported; use docs.json naming_style",
    );
}

#[test]
fn docs_create_rejects_missing_metadata_pattern() {
    let repo = repo();
    run_cli(repo.path(), &["namespace", "create", "conventions"], 0);

    let output = run_cli(
        repo.path(),
        &[
            "docs",
            "create",
            "code_style",
            "--namespace",
            "conventions",
            "# code_style\n\nRust code style conventions.\n",
        ],
        1,
    );

    assert_line(
        &output,
        "error: invalid_document_metadata: document metadata paragraph must match: 本文说明<description>。",
    );
    assert!(!repo.path().join("docs/conventions/code_style.md").exists());
}

#[test]
fn check_rejects_existing_document_without_metadata_pattern() {
    let repo = repo();
    run_cli(repo.path(), &["namespace", "create", "conventions"], 0);
    fs::write(
        repo.path().join("docs/conventions/code_style.md"),
        "# code_style\n\nRust code style conventions.\n",
    )
    .unwrap();

    let output = run_cli(repo.path(), &["check"], 1);
    assert_line(
        &output,
        "error: invalid_document_metadata: document metadata paragraph must match: 本文说明<description>。",
    );
    assert_line(&output, "details.path: docs/conventions/code_style.md");
}

#[test]
fn docs_rename_requires_unique_origin_document_name() {
    let repo = repo();
    run_cli(repo.path(), &["namespace", "create", "conventions"], 0);
    run_cli(repo.path(), &["namespace", "create", "design"], 0);
    run_cli(
        repo.path(),
        &[
            "docs",
            "create",
            "readme",
            "--namespace",
            "conventions",
            "# readme\n\n本文说明 conventions readme。\n",
        ],
        0,
    );
    run_cli(
        repo.path(),
        &[
            "docs",
            "create",
            "readme",
            "--namespace",
            "design",
            "# readme\n\n本文说明 design readme。\n",
        ],
        0,
    );

    let output = run_cli(repo.path(), &["docs", "rename", "readme", "readme_NEW"], 1);
    assert_line(
        &output,
        "error: document_ambiguous: document exists in multiple namespaces",
    );
}

#[test]
fn short_commands_fail_with_normal_cli_error() {
    let repo = repo();
    let output = run_cli(repo.path(), &["ns", "read"], 2);
    assert_line(
        &output,
        "error: invalid_command: error: unrecognized subcommand 'ns'",
    );
}

#[test]
fn namespace_list_uses_normal_cli_output() {
    let repo = repo();
    let output = run_cli(repo.path(), &["namespace", "list"], 0);
    assert!(serde_json::from_str::<serde_json::Value>(&output).is_err());
}
