use std::error::Error;
use std::fmt;
use std::fmt::{Display, Formatter};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone)]
pub struct Release {
    pub url: String,
    pub id: u64,
    pub draft: bool,
    pub prerelease: bool,
    pub tag_name: String,
    pub assets: Vec<Asset>,
    pub created_at: String,
    pub published_at: String,
    pub tarball_url: String,
    pub body: String,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Asset {
    pub url: String,
    pub id: u64,
    pub name: String,
    pub content_type: String,
    pub state: String,
    pub size: u64,
    pub download_count: u64,
    pub created_at: String,
    pub updated_at: String,
    pub browser_download_url: String,
}

pub async fn list_all_releases(owner: &str, repository: &str) -> Result<Vec<Release>, GitHubUtilError> {
    let client = reqwest::Client::builder()
        .user_agent("FlashyReese/decky-wine-cellar")
        .build()
        .expect("Failed to create HTTP client");

    let mut releases: Vec<Release> = Vec::new();
    let mut page = 1;

    loop {
        let url = format!(
            "https://api.github.com/repos/{}/{}/releases?per_page=100&page={}",
            owner, repository, page
        );

        let response = client
            .get(&url)
            .send()
            .await
            .map_err(|err| GitHubUtilError::RequestError(err.to_string()))?;

        if response.status().is_success() {
            let response_text = response.text().await.unwrap();
            let page_releases: Vec<Release> = serde_json::from_str(&response_text)
                .map_err(|err| GitHubUtilError::JsonParsingError(err.to_string())).unwrap();

            if page_releases.is_empty() {
                break; // No more releases, exit the loop
            }

            releases.extend(page_releases);
            page += 1;
        } else {
            return Err(GitHubUtilError::RequestError(format!(
                "Failed to fetch releases: {}",
                response.status()
            )));
        }
    }

    Ok(releases)
}

#[derive(Debug)]
pub enum GitHubUtilError {
    RequestError(String),
    JsonParsingError(String),
}

impl Display for GitHubUtilError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            GitHubUtilError::RequestError(json) =>
                write!(f, "Request Error: {}", json),
            GitHubUtilError::JsonParsingError(json) =>
                write!(f, "Failed to parse Json: {}", json),
        }
    }
}

impl Error for GitHubUtilError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}