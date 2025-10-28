use anyhow::Context as _;
use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::io::Write as _;
use std::{fmt::Display, fs::File};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Deserialize, Serialize)]
pub struct Repo {
    pub user: String,
    pub repo: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Deserialize, Serialize)]
pub struct RepoFile {
    pub repo: Repo,
    pub file_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Deserialize, Serialize)]
pub struct RepoRelease {
    pub repo: Repo,
    pub name: String,
    pub tag: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Deserialize, Serialize)]
pub enum Proxy {
    #[default]
    Github,
    Xget,
    GhProxy,
}

impl Display for Proxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Proxy::Github => write!(f, "github"),
            Proxy::Xget => write!(f, "xget"),
            Proxy::GhProxy => write!(f, "gh-proxy"),
        }
    }
}

impl From<&str> for Proxy {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "github" => Proxy::Github,
            "xget" => Proxy::Xget,
            "gh-proxy" => Proxy::GhProxy,
            _ => Proxy::Github,
        }
    }
}

impl Proxy {
    pub fn url(
        &self,
        RepoRelease {
            name,
            repo: Repo { user, repo },
            tag,
        }: RepoRelease,
    ) -> String {
        match self {
            Proxy::Github => format!(
                "https://github.com/{}/{}/releases/download/{}/{}",
                user, repo, tag, name
            ),
            Proxy::Xget => {
                format!("https://xget.xi-xu.me/gh/{user}/{repo}/releases/download/{tag}/{name}")
            }
            Proxy::GhProxy => format!(
                "https://gh-proxy.com/https://github.com/{user}/{repo}/releases/download/{tag}/{name}"
            ),
        }
    }
}

pub async fn download_file(url: &str, dest: &str) -> Result<()> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .unwrap_or_else(|_| Client::new());

    let response = client.get(url).send().await.context("发送HTTP请求失败")?;
    let bytes = response.bytes().await.context("读取响应数据失败")?;
    let mut file = File::create(dest).context(format!("创建文件失败: {}", dest))?;
    file.write_all(&bytes).context("写入文件失败")?;

    Ok(())
}
