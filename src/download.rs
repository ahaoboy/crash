use anyhow::Context as _;
use anyhow::Result;
use reqwest::Client;
use std::io::Write as _;
use std::fs::File;

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
