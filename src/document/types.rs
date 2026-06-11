#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentSummary {
    pub namespace: String,
    pub document: String,
    pub path: String,
    pub title: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentCreated {
    pub path: String,
    pub document: String,
    pub namespace: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentRenamed {
    pub old_path: String,
    pub new_path: String,
    pub namespace: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentDeleted {
    pub path: String,
    pub namespace: String,
    pub document: String,
    pub deleted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentPatched {
    pub path: String,
    pub namespace: String,
    pub document: String,
    pub patched: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixMigration {
    pub old_path: String,
    pub new_path: String,
}
