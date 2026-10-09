use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReleaseSource {
    GitHub,
    Codeberg,
}

impl ReleaseSource {
    pub fn cache_prefix(self) -> &'static str {
        match self {
            Self::GitHub => "github",
            Self::Codeberg => "codeberg",
        }
    }

    fn releases_url(self, owner: &str, repository: &str, page: usize) -> String {
        match self {
            Self::GitHub => format!(
                "https://api.github.com/repos/{}/{}/releases?per_page=100&page={}",
                owner, repository, page
            ),
            Self::Codeberg => format!(
                "https://codeberg.org/api/v1/repos/{}/{}/releases?limit=100&page={}",
                owner, repository, page
            ),
        }
    }
}

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
    // Codeberg attachments omit these GitHub-specific metadata fields.
    #[serde(default)]
    pub url: String,
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub content_type: String,
    #[serde(default)]
    pub state: String,
    pub size: u64,
    pub download_count: u64,
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
    pub browser_download_url: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Response {
    pub message: String,
}

pub async fn list_all_releases(
    source: ReleaseSource,
    owner: &str,
    repository: &str,
) -> Result<Vec<Release>, ReleaseUtilError> {
    let client = reqwest::Client::builder()
        .user_agent("FlashyReese/decky-wine-cellar")
        .no_proxy()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(ReleaseUtilError::Request)?;

    let mut releases: Vec<Release> = Vec::new();
    let mut page = 1;

    loop {
        let url = source.releases_url(owner, repository, page);

        let response = client.get(&url).send().await?;

        let response = response.error_for_status()?;

        let response_text = response.text().await?;

        match serde_json::from_str::<Vec<Release>>(&response_text) {
            Ok(page_releases) => {
                if page_releases.is_empty() {
                    break;
                }

                releases.extend(page_releases);
                page += 1;
            }
            Err(json_err) => {
                if let Ok(api_error) = serde_json::from_str::<Response>(&response_text) {
                    return Err(ReleaseUtilError::ResponseError(api_error.message));
                }

                return Err(ReleaseUtilError::Json {
                    source: json_err,
                    body: response_text,
                });
            }
        }
    }

    Ok(releases)
}

#[derive(Debug)]
pub enum ReleaseUtilError {
    Request(reqwest::Error),
    Json {
        source: serde_json::Error,
        body: String,
    },
    ResponseError(String),
}

impl Display for ReleaseUtilError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ReleaseUtilError::Request(err) => write!(f, "Request error: {}", err),
            ReleaseUtilError::Json { source, body } => {
                write!(
                    f,
                    "Failed to parse JSON: {}. Response body: {}",
                    source, body
                )
            }
            ReleaseUtilError::ResponseError(msg) => write!(f, "Response error: {}", msg),
        }
    }
}

impl Error for ReleaseUtilError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ReleaseUtilError::Request(err) => Some(err),
            ReleaseUtilError::Json { source, .. } => Some(source),
            ReleaseUtilError::ResponseError(_) => None,
        }
    }
}

impl From<reqwest::Error> for ReleaseUtilError {
    fn from(err: reqwest::Error) -> Self {
        ReleaseUtilError::Request(err)
    }
}

impl From<serde_json::Error> for ReleaseUtilError {
    fn from(err: serde_json::Error) -> Self {
        ReleaseUtilError::Json {
            source: err,
            body: String::new(),
        }
    }
}

pub fn format_error_chain(err: &(dyn Error + 'static)) -> String {
    let mut output = format!("{}", err);
    let mut current = err.source();

    while let Some(src) = current {
        output.push_str(&format!("\ncaused by: {}", src));
        current = src.source();
    }

    output
}