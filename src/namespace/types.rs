#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamespaceSummary {
    pub namespace: String,
    pub path: String,
    pub naming_style_regex: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamespaceCreated {
    pub namespace: String,
    pub path: String,
    pub naming_style_regex: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamespaceRenamed {
    pub old_namespace: String,
    pub new_namespace: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamespaceDeleted {
    pub namespace: String,
    pub deleted: bool,
    pub deleted_docs: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamespaceDocument {
    pub document: String,
    pub path: String,
    pub title: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamespaceDocuments {
    pub namespace: String,
    pub naming_style_regex: String,
    pub documents: Vec<NamespaceDocument>,
}
