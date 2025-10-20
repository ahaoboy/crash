use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Deserialize,Serialize)]
pub struct Repo {
    pub user: String,
    pub repo: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Deserialize,Serialize)]
pub struct RepoFile {
    pub repo: Repo,
    pub file_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Deserialize,Serialize)]
pub struct RepoRelease {
    pub repo: Repo,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Deserialize,Serialize)]
pub enum Proxy {
    #[default]
    Github,
    Xget,
}
