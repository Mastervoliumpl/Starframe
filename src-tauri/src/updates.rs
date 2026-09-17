//! Release selection and scheduling, independent of the desktop window.
use semver::Version;
use serde::{Deserialize, Serialize};

pub const RELEASES: &str = "https://github.com/Mastervoliumpl/Starframe/releases";
pub const INTERVAL: u64 = 300;

#[cfg(debug_assertions)]
#[derive(Deserialize)]
pub struct Fixture {
    pub origin: String,
    pub pubkey: String,
}

#[cfg(debug_assertions)]
pub fn fixture() -> Result<Option<Fixture>, String> {
    use std::io::Read;
    let Some(root) = std::env::var_os("STARFRAME_TEST_DATA_DIR") else {
        return Ok(None);
    };
    let path = std::path::PathBuf::from(root).join("update-fixture.json");
    if !path.exists() {
        return Ok(None);
    }
    let mut text = vec![];
    crate::filesystem::read_file(&path)?
        .take(8193)
        .read_to_end(&mut text)
        .map_err(|e| e.to_string())?;
    if text.len() > 8192 {
        return Err("Update fixture is too large.".into());
    }
    let fixture: Fixture = serde_json::from_slice(&text).map_err(|_| "Invalid update fixture.")?;
    let url = reqwest::Url::parse(&fixture.origin).map_err(|_| "Invalid fixture origin.")?;
    if url.scheme() != "http"
        || url.host_str() != Some("127.0.0.1")
        || url.port().is_none()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("Updater fixtures require an explicit loopback port.".into());
    }
    Ok(Some(fixture))
}

pub fn fixture_enabled() -> bool {
    #[cfg(debug_assertions)]
    return fixture().is_ok_and(|f| f.is_some());
    #[cfg(not(debug_assertions))]
    false
}

pub fn endpoint(official: &str) -> Result<String, String> {
    #[cfg(debug_assertions)]
    if let Some(fixture) = fixture()? {
        let url = reqwest::Url::parse(official).map_err(|_| "Invalid official update URL.")?;
        return Ok(format!(
            "{}{}{}",
            fixture.origin.trim_end_matches('/'),
            url.path(),
            url.query().map(|q| format!("?{q}")).unwrap_or_default()
        ));
    }
    Ok(official.into())
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(rename = "UpdateChannel"))]
pub enum Channel {
    Stable,
    Preview,
}
impl Default for Channel {
    fn default() -> Self {
        if Version::parse(env!("CARGO_PKG_VERSION"))
            .unwrap()
            .pre
            .is_empty()
        {
            Self::Stable
        } else {
            Self::Preview
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(rename = "UpdateRelease"))]
pub struct Release {
    pub version: String,
    pub notes: String,
}
impl Release {
    pub fn url(&self) -> String {
        format!("{RELEASES}/tag/v{}", self.version)
    }
    pub fn manifest_url(&self) -> String {
        format!("{RELEASES}/download/v{}/latest.json", self.version)
    }
    pub fn installer_url(&self) -> String {
        format!(
            "{RELEASES}/download/v{0}/Starframe_{0}_x64-setup.exe",
            self.version
        )
    }
    fn validate(&self) -> Result<(), String> {
        let version = Version::parse(&self.version).map_err(|_| "Invalid release version.")?;
        if version.to_string() != self.version
            || !version.build.is_empty()
            || self.notes.len() > 65536
        {
            return Err("Invalid release metadata.".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Saved {
    pub channel: Channel,
    pub release: Option<Release>,
    pub dismissed: Option<String>,
    pub last_success: Option<u64>,
}
impl Saved {
    pub fn read(text: &str) -> Result<Self, String> {
        if text.len() > 70000 {
            return Err("Saved update information is too large.".into());
        }
        let value: Self =
            serde_json::from_str(text).map_err(|_| "Saved update information is invalid.")?;
        if let Some(release) = &value.release {
            release.validate()?;
        }
        if let Some(version) = &value.dismissed {
            Version::parse(version).map_err(|_| "Invalid dismissed update version.")?;
        }
        Ok(value)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(rename = "UpdatePhase"))]
pub enum Phase {
    #[default]
    Idle,
    Checking,
    Downloading,
    Waiting,
    Installing,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(rename = "UpdateView"))]
pub struct View {
    pub channel: Channel,
    pub phase: Phase,
    pub release: Option<Release>,
    pub dismissed: bool,
    pub last_success: Option<String>,
    pub message: String,
    pub error: Option<String>,
    pub received: String,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(rename = "UpdateAction"))]
pub enum Action {
    Check,
    Install { version: String },
    Later { version: String },
    Cancel,
    Channel { channel: Channel },
    ViewRelease,
}

#[derive(Default)]
pub struct Schedule {
    pub next: u64,
    pub in_flight: bool,
    failures: u32,
    retry_after: u64,
}
impl Schedule {
    pub fn retry_allowed(&self, now: u64) -> bool {
        now >= self.retry_after
    }
    pub fn start(&mut self, now: u64, manual: bool, ready: bool, stopped: bool) -> bool {
        if stopped
            || !ready
            || self.in_flight
            || now < self.retry_after
            || (!manual && now < self.next)
        {
            return false;
        }
        self.in_flight = true;
        true
    }
    pub fn complete(&mut self, now: u64, success: bool, retry_after: Option<u64>) {
        self.in_flight = false;
        self.failures = if success {
            0
        } else {
            self.failures.saturating_add(1)
        };
        let delay = if success {
            INTERVAL
        } else {
            INTERVAL * (1u64 << self.failures.saturating_sub(1).min(4))
        };
        self.retry_after = retry_after.unwrap_or(0).max(if success {
            0
        } else {
            now.saturating_add(delay)
        });
        self.next = now.saturating_add(delay).max(self.retry_after);
    }
}

#[derive(Deserialize)]
pub struct GithubRelease {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    body: Option<String>,
    assets: Vec<GithubAsset>,
}
#[derive(Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

pub fn select(
    releases: Vec<GithubRelease>,
    installed: &str,
    channel: Channel,
) -> Result<Option<Release>, String> {
    let installed = Version::parse(installed).map_err(|_| "Installed version is invalid.")?;
    let mut selected: Option<(Version, Release)> = None;
    for item in releases {
        if item.draft {
            continue;
        }
        let Some(tag) = item.tag_name.strip_prefix('v') else {
            continue;
        };
        let Ok(version) = Version::parse(tag) else {
            continue;
        };
        if version <= installed
            || !version.build.is_empty()
            || (channel == Channel::Stable && (item.prerelease || !version.pre.is_empty()))
        {
            continue;
        }
        let release = Release {
            version: version.to_string(),
            notes: item.body.unwrap_or_default(),
        };
        release.validate()?;
        if item.tag_name != format!("v{}", release.version) {
            continue;
        }
        // Catalog-only and incomplete uploads are not application updates.
        if !item
            .assets
            .iter()
            .any(|a| a.name == "latest.json" && a.browser_download_url == release.manifest_url())
        {
            continue;
        }
        if !item
            .assets
            .iter()
            .any(|a| a.browser_download_url == release.installer_url())
        {
            return Err("The release is missing its Windows installer.".into());
        }
        if selected
            .as_ref()
            .is_none_or(|(previous, _)| version > *previous)
        {
            selected = Some((version, release));
        }
    }
    Ok(selected.map(|(_, release)| release))
}

#[derive(Debug)]
pub struct CheckError {
    pub message: String,
    pub retry_after: Option<u64>,
}
impl From<String> for CheckError {
    fn from(message: String) -> Self {
        Self {
            message,
            retry_after: None,
        }
    }
}
impl From<&str> for CheckError {
    fn from(message: &str) -> Self {
        message.to_owned().into()
    }
}

pub async fn discover(channel: Channel, now: u64) -> Result<Option<Release>, CheckError> {
    let client = reqwest::Client::builder()
        .https_only(!fixture_enabled())
        .user_agent("Starframe updates/1")
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(20))
        .retry(reqwest::retry::never())
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())?;
    let mut releases = vec![];
    for page in 1..=10 {
        let mut response = client.get(endpoint(&format!("https://api.github.com/repos/Mastervoliumpl/Starframe/releases?per_page=100&page={page}"))?)
            .header("Accept", "application/vnd.github+json").send().await.map_err(|_| "Could not reach GitHub. Installed mods remain usable offline.".to_string())?;
        if !response.status().is_success() {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| retry_time(v, now))
                .or_else(|| {
                    response
                        .headers()
                        .get("x-ratelimit-reset")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| v.parse().ok())
                });
            return Err(CheckError {
                message: format!(
                    "GitHub update check returned {}. Starframe will retry later.",
                    response.status()
                ),
                retry_after,
            });
        }
        let mut bytes = vec![];
        while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
            if bytes.len() + chunk.len() > 4 * 1024 * 1024 {
                return Err("GitHub release information is too large."
                    .to_string()
                    .into());
            }
            bytes.extend_from_slice(&chunk);
        }
        let batch: Vec<GithubRelease> = serde_json::from_slice(&bytes)
            .map_err(|_| "GitHub release information is invalid.".to_string())?;
        let done = batch.len() < 100;
        releases.extend(batch);
        if done {
            return select(releases, env!("CARGO_PKG_VERSION"), channel).map_err(Into::into);
        }
    }
    Err("Too many release pages to confirm the latest version."
        .to_string()
        .into())
}

fn retry_time(value: &str, now: u64) -> Option<u64> {
    value
        .parse::<u64>()
        .ok()
        .map(|delay| now.saturating_add(delay))
        .or_else(|| {
            httpdate::parse_http_date(value)
                .ok()?
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .ok()
                .map(|time| time.as_secs())
        })
}

#[cfg(test)]
mod tests;
