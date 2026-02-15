use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use std::fmt::{Display, Formatter};

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Release {
    pub url: String,
    pub id: u64,
    pub draft: bool,
    pub prerelease: bool,
    pub name: String,
    pub tag_name: String,
    pub assets: Vec<Asset>,
    pub created_at: String,
    pub published_at: String,
    pub tarball_url: String,
    pub body: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
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

#[derive(Deserialize, Serialize, Clone)]
pub struct Response {
    pub message: String,
}

/// Result of a conditional fetch operation
#[derive(Debug)]
pub enum FetchResult {
    /// Releases were modified, contains the new releases and optionally the last-modified timestamp
    Modified(Vec<Release>, Option<String>),
    /// Releases were not modified (304 Not Modified)
    NotModified,
}

/// Fetches all releases from GitHub repository with optional conditional request support
///
/// # Arguments
/// * `owner` - Repository owner
/// * `repository` - Repository name
/// * `if_modified_since` - Optional timestamp for If-Modified-Since header (RFC 2822 format)
///
/// # Returns
/// * `Ok(FetchResult::Modified(releases, last_modified))` - New releases fetched
/// * `Ok(FetchResult::NotModified)` - Server responded with 304 Not Modified
/// * `Err(GitHubUtilError)` - Error occurred
pub async fn list_all_releases(
    owner: &str,
    repository: &str,
    if_modified_since: Option<&str>,
) -> Result<FetchResult, GitHubUtilError> {
    let client = reqwest::Client::builder()
        .user_agent("FlashyReese/decky-wine-cellar")
        .build()
        .expect("Failed to create HTTP client");

    let mut releases: Vec<Release> = Vec::new();
    let mut page = 1;
    let mut last_modified: Option<String> = None;

    loop {
        let url = format!(
            "https://api.github.com/repos/{}/{}/releases?per_page=100&page={}",
            owner, repository, page
        );

        let mut request = client.get(&url);

        // Add If-Modified-Since header only for the first page
        if page == 1 {
            if let Some(timestamp) = if_modified_since {
                request = request.header("If-Modified-Since", timestamp);
            }
        }

        let response = request.send().await?;

        // Handle 304 Not Modified (only relevant for first page)
        if page == 1 && response.status() == reqwest::StatusCode::NOT_MODIFIED {
            return Ok(FetchResult::NotModified);
        }

        if response.status().is_success() {
            // Extract last-modified header from first page response
            if page == 1 {
                last_modified = response
                    .headers()
                    .get("last-modified")
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string());
            }

            let response_text = response.text().await?;
            if let Ok(page_releases) = serde_json::from_str::<Vec<Release>>(&response_text) {
                if page_releases.is_empty() {
                    break; // No more releases, exit the loop
                }

                releases.extend(page_releases);
            } else {
                return if let Ok(response) = serde_json::from_str::<Response>(&response_text) {
                    Err(GitHubUtilError::ResponseError(response.message))
                } else {
                    Err(GitHubUtilError::JsonParsingError(response_text))
                };
            }
            page += 1;
        } else {
            return Err(GitHubUtilError::RequestError(format!(
                "Failed to fetch releases: {}",
                response.status()
            )));
        }
    }

    Ok(FetchResult::Modified(releases, last_modified))
}

#[derive(Debug)]
pub enum GitHubUtilError {
    RequestError(String),
    JsonParsingError(String),
    ResponseError(String),
}

impl Display for GitHubUtilError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            GitHubUtilError::RequestError(json) => write!(f, "Request Error: {}", json),
            GitHubUtilError::JsonParsingError(json) => {
                write!(f, "Failed to parse Json: {}", json)
            }
            GitHubUtilError::ResponseError(json) => {
                write!(f, "Response error: {}", json)
            }
        }
    }
}

impl Error for GitHubUtilError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

impl From<reqwest::Error> for GitHubUtilError {
    fn from(err: reqwest::Error) -> GitHubUtilError {
        GitHubUtilError::RequestError(err.to_string())
    }
}

impl From<serde_json::Error> for GitHubUtilError {
    fn from(err: serde_json::Error) -> GitHubUtilError {
        GitHubUtilError::JsonParsingError(err.to_string())
    }
}
