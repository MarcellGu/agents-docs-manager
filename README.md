# agents-docs-manager

`agents-docs-manager` 安装一个名为 `adm` 的 CLI 命令，用于管理仓库本地的 `docs.json` 文档配置，以及带固定元数据头部的 Markdown 文档。

## 安装

```bash
cargo install agents-docs-manager
```

## 开发安装

```bash
cargo install --path . --force
```

## 命令形式

命令使用完整单词参数，并将普通 CLI 文本写入 `stdout`。

在仓库中首次使用：

```bash
adm init
```

`adm init` 会在项目根目录创建 `docs.json`，并创建配置中的 `./docs` 目录。

初始化成功后会打印推断出的文档目录和命名风格：

```text
index_path docs.json
doc_dir ./docs
naming_style snake_case
```

`naming_style` 作用于 `doc_dir` 下的名称：namespace 目录名和受管 Markdown 文档的文件名主干。`adm init` 会根据当前工作目录一级子目录中最常见的命名风格推断 `doc_dir` 和 `naming_style`；文件会被忽略。手动修改 `docs.json.naming_style` 后，运行 `adm fix` 迁移已有 namespace 和文档文件名。

省略 `--namespace` 可以直接在 `doc_dir` 下创建文档：

```bash
adm docs create overview "# overview

本文说明 Project overview。"
```

```bash
adm namespace create conventions
```

```bash
adm docs create code_style --namespace conventions "# code_style

本文说明 Rust code style conventions。"
```

从 stdin 读取单文件 unified diff 来 patch 文档：

```bash
adm docs patch code_style --namespace conventions <<'EOF'
--- a/docs/conventions/code_style.md
+++ b/docs/conventions/code_style.md
@@ -1,3 +1,3 @@
 # code_style
 
-本文说明 Rust code style conventions。
+本文说明 Rust code style guidelines。
EOF
```

列出 namespaces：

```text
conventions docs/conventions
```

列出文档：

```text
overview docs/overview.md
code_style docs/conventions/code_style.md
```

显示公开文档树：

```text
docs/
|-- conventions/
|   `-- code_style.md - 本文说明 Rust code style conventions。
`-- overview.md - 本文说明 Project overview。
```

将当前 docs index 同步到 `AGENTS.md`：

```bash
adm sync
```

预期错误会以普通文本写入 `stdout`：

```text
error: invalid_document_name: document name does not match naming_style
details.namespace: conventions
```

修改 `docs.json.naming_style` 后运行命名迁移：

```bash
adm fix
```

只支持 `adm` 这一个二进制命令。子命令使用完整单词，例如 `namespace`、`docs`、`tree`、`sync`、`check` 和 `fix`。
