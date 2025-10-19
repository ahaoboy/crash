#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Repo {
    pub user: String,
    pub repo: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct RepoFile {
    pub repo: Repo,
    pub file_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct RepoRelease {
    pub repo: Repo,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum Proxy {
    #[default]
    Github,
    Xget,
}
