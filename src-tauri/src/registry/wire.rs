use super::{ModId, PublicationOrder, ReleaseId, Sha256};
use serde::{Deserialize, Serialize};

fn safe_u64<'de, D: serde::Deserializer<'de>>(input: D) -> Result<u64, D::Error> {
    let value = u64::deserialize(input)?;
    if value <= super::MAX_SAFE_INTEGER {
        Ok(value)
    } else {
        Err(serde::de::Error::custom("unsafe registry integer"))
    }
}

fn positive_u64<'de, D: serde::Deserializer<'de>>(input: D) -> Result<u64, D::Error> {
    let value = safe_u64(input)?;
    if value > 0 {
        Ok(value)
    } else {
        Err(serde::de::Error::custom(
            "registry integer must be positive",
        ))
    }
}

fn required_option<'de, D, T>(input: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::deserialize(input)
}

#[derive(Clone, Debug, Deserialize)]
#[serde(try_from = "u8")]
pub struct ApiVersion;

impl Serialize for ApiVersion {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8(1)
    }
}

impl TryFrom<u8> for ApiVersion {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if value == 1 {
            Ok(Self)
        } else {
            Err("unsupported registry API version")
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApiResponse<T> {
    pub api_version: ApiVersion,
    pub data: T,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ModList {
    #[cfg_attr(test, ts(type = "1"))]
    pub api_version: ApiVersion,
    pub items: Vec<ModSummary>,
    pub pagination: Pagination,
}

impl ModList {
    pub(super) fn valid(&self) -> bool {
        self.items.len() <= 50
            && [6, 10, 12, 20, 24, 48, 50].contains(&self.pagination.page_size)
            && self.pagination.as_of.ends_with('Z')
            && self.items.iter().all(ModSummary::valid)
            && self
                .items
                .iter()
                .map(|item| item.mod_id)
                .collect::<std::collections::HashSet<_>>()
                .len()
                == self.items.len()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ListQuery {
    #[cfg_attr(test, ts(type = "number"))]
    pub page: u64,
    pub page_size: u8,
    pub query: String,
    pub include_tags: Vec<String>,
    pub exclude_tags: Vec<String>,
    pub sort: Sort,
    pub period: Period,
    pub maintenance: Maintenance,
    pub mod_type: ModTypeFilter,
    pub game_builds: Vec<String>,
}

impl Default for ListQuery {
    fn default() -> Self {
        Self {
            page: 1,
            page_size: 24,
            query: String::new(),
            include_tags: Vec::new(),
            exclude_tags: Vec::new(),
            sort: Sort::Updated,
            period: Period::All,
            maintenance: Maintenance::All,
            mod_type: ModTypeFilter::All,
            game_builds: Vec::new(),
        }
    }
}

impl ListQuery {
    pub(super) fn validate(&self) -> Result<(), &'static str> {
        let unique = |items: &[String], limit: usize, length: usize| {
            items.len() <= limit
                && items
                    .iter()
                    .all(|item| !item.is_empty() && item.encode_utf16().count() <= length)
                && items.iter().collect::<std::collections::HashSet<_>>().len() == items.len()
        };
        if !(1..=super::MAX_SAFE_INTEGER).contains(&self.page)
            || ![6, 10, 12, 20, 24, 48, 50].contains(&self.page_size)
            || self.query.encode_utf16().count() > 200
            || !unique(&self.include_tags, 20, 64)
            || !unique(&self.exclude_tags, 20, 64)
            || !unique(&self.game_builds, 20, 100)
            || self
                .include_tags
                .iter()
                .any(|tag| self.exclude_tags.contains(tag))
        {
            return Err("invalid registry list query");
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum Sort {
    Updated,
    Published,
    Downloads,
    Name,
}
impl Sort {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Updated => "updated",
            Self::Published => "published",
            Self::Downloads => "downloads",
            Self::Name => "name",
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum Period {
    All,
    #[serde(rename = "24h")]
    Day,
    #[serde(rename = "7d")]
    Week,
    #[serde(rename = "1m")]
    Month,
    #[serde(rename = "3m")]
    ThreeMonths,
    #[serde(rename = "6m")]
    SixMonths,
    #[serde(rename = "1y")]
    Year,
}
impl Period {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Day => "24h",
            Self::Week => "7d",
            Self::Month => "1m",
            Self::ThreeMonths => "3m",
            Self::SixMonths => "6m",
            Self::Year => "1y",
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum Maintenance {
    All,
    Maintained,
    Unmaintained,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum ModTypeFilter {
    All,
    Code,
    Map,
    Ai,
    Unclassified,
}
impl ModTypeFilter {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Code => "code",
            Self::Map => "map",
            Self::Ai => "ai",
            Self::Unclassified => "unclassified",
        }
    }
}
impl Maintenance {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Maintained => "maintained",
            Self::Unmaintained => "unmaintained",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Pagination {
    #[serde(deserialize_with = "positive_u64")]
    #[cfg_attr(test, ts(type = "number"))]
    pub page: u64,
    #[serde(deserialize_with = "positive_u64")]
    #[cfg_attr(test, ts(type = "number"))]
    pub page_size: u64,
    #[serde(deserialize_with = "safe_u64")]
    #[cfg_attr(test, ts(type = "number"))]
    pub total_items: u64,
    #[serde(deserialize_with = "safe_u64")]
    #[cfg_attr(test, ts(type = "number"))]
    pub total_pages: u64,
    pub as_of: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Profile {
    pub display_name: String,
    pub avatar_url: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Session {
    pub account_id: uuid::Uuid,
    pub profile: Profile,
    pub context: SessionContext,
    pub authenticated_at: String,
    pub expires_at: String,
    pub capabilities: Vec<Capability>,
    pub is_owner: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionContext {
    Website,
    Manager,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    CreateMod,
    SubmitRelease,
    EditMod,
    DownloadMod,
    Comment,
    Like,
    Report,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ModSummary {
    pub mod_id: ModId,
    #[serde(deserialize_with = "required_option")]
    pub mod_type: Option<ModType>,
    pub name: String,
    pub summary: String,
    pub owner: Option<Profile>,
    pub tags: Vec<String>,
    pub maintained: bool,
    pub replacement_mod_id: Option<ModId>,
    pub updated_at: String,
    pub latest_release_id: Option<ReleaseId>,
    #[serde(deserialize_with = "safe_u64")]
    #[cfg_attr(test, ts(type = "number"))]
    pub downloads: u64,
    #[serde(deserialize_with = "safe_u64")]
    #[cfg_attr(test, ts(type = "number"))]
    pub likes: u64,
    #[serde(deserialize_with = "required_option")]
    #[cfg_attr(test, ts(type = "number | null"))]
    pub archive_bytes: Option<u64>,
    pub icon_id: Option<ReleaseId>,
    pub published_at: Option<String>,
    pub latest_availability: Option<Availability>,
}

impl ModSummary {
    fn valid(&self) -> bool {
        (1..=120).contains(&self.name.len())
            && self.summary.len() <= 500
            && self.tags.len() <= 50
            && self.tags.iter().all(|tag| (1..=64).contains(&tag.len()))
            && self
                .tags
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                == self.tags.len()
            && self.updated_at.ends_with('Z')
            && self
                .archive_bytes
                .is_none_or(|bytes| (1..=super::MAX_SAFE_INTEGER).contains(&bytes))
            && self
                .owner
                .as_ref()
                .is_none_or(|owner| (1..=120).contains(&owner.display_name.len()))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum ModType {
    Code,
    Map,
    Ai,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ModMedia {
    #[serde(deserialize_with = "required_option")]
    pub icon_id: Option<ReleaseId>,
    pub screenshot_ids: Vec<ReleaseId>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ModDetail {
    pub listing: ModSummary,
    pub description: String,
    #[serde(deserialize_with = "required_option")]
    pub source_repository: Option<String>,
    pub media: ModMedia,
    pub latest_release: ReleaseResult,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum ModResult {
    Mod(Box<ModDetail>),
    Tombstone(Tombstone),
}

impl ModResult {
    pub(super) fn valid(&self, expected: ModId) -> bool {
        match self {
            Self::Mod(detail) => {
                detail.listing.mod_id == expected
                    && detail.listing.valid()
                    && detail.description.len() <= 20_000
                    && detail.media.screenshot_ids.len() <= 10
                    && detail
                        .media
                        .screenshot_ids
                        .iter()
                        .collect::<std::collections::HashSet<_>>()
                        .len()
                        == detail.media.screenshot_ids.len()
                    && detail.latest_release.valid()
                    && match &detail.latest_release {
                        ReleaseResult::Release(release) => {
                            release.mod_id == expected
                                && Some(release.release_id) == detail.listing.latest_release_id
                        }
                        ReleaseResult::Tombstone(release) => {
                            release.mod_id == expected
                                && Some(release.release_id) == detail.listing.latest_release_id
                        }
                    }
                    && detail.source_repository.as_ref().is_none_or(|source| {
                        source.len() <= 2048
                            && reqwest::Url::parse(source).is_ok_and(|url| {
                                url.scheme() == "https"
                                    && url.host_str() == Some("github.com")
                                    && url.username().is_empty()
                                    && url.password().is_none()
                                    && url.query().is_none()
                                    && url.fragment().is_none()
                                    && url.path().trim_matches('/').split('/').count() == 2
                            })
                    })
            }
            Self::Tombstone(tombstone) => {
                tombstone.mod_id == expected && ReleaseResult::Tombstone(tombstone.clone()).valid()
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum Availability {
    Available,
    Withdrawn,
    Hidden,
    Pruned,
    Blocked,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Artifact {
    pub sha256: Sha256,
    #[serde(deserialize_with = "positive_u64")]
    #[cfg_attr(test, ts(type = "number"))]
    pub bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Release {
    pub mod_id: ModId,
    pub release_id: ReleaseId,
    pub version_label: String,
    pub artifact: Artifact,
    #[cfg_attr(test, ts(type = "'approved'"))]
    pub submission_state: Approved,
    pub publication_order: PublicationOrder,
    pub published_at: String,
    pub created_at: String,
    pub availability: Availability,
    pub security: Security,
    pub metadata: Metadata,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(try_from = "String")]
pub struct Approved;

impl Serialize for Approved {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str("approved")
    }
}

impl TryFrom<String> for Approved {
    type Error = &'static str;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value == "approved" {
            Ok(Self)
        } else {
            Err("release is not approved")
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Security {
    pub status: SecurityStatus,
    #[serde(deserialize_with = "safe_u64")]
    #[cfg_attr(test, ts(type = "number"))]
    pub revision: u64,
    pub reason: Option<String>,
}

impl Security {
    pub(super) fn valid_for(&self, availability: &Availability) -> bool {
        self.reason
            .as_ref()
            .is_none_or(|reason| (1..=1000).contains(&reason.encode_utf16().count()))
            && matches!(
                (availability, &self.status),
                (Availability::Blocked, SecurityStatus::Blocked)
                    | (
                        Availability::Available
                            | Availability::Withdrawn
                            | Availability::Hidden
                            | Availability::Pruned,
                        SecurityStatus::NotBlocked
                    )
            )
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum SecurityStatus {
    NotBlocked,
    Blocked,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Metadata {
    #[serde(deserialize_with = "positive_u64")]
    #[cfg_attr(test, ts(type = "number"))]
    pub revision: u64,
    #[cfg_attr(test, ts(type = "'approved'"))]
    pub state: Approved,
    pub tested_game_build: String,
    pub source_repository: Option<String>,
    pub release_notes: String,
    pub dependencies: Vec<Dependency>,
    pub dependency_problems: Vec<DependencyProblem>,
    #[serde(default)]
    pub installation: Option<super::installation::Installation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum Dependency {
    Exact {
        #[serde(rename = "modId")]
        mod_id: ModId,
        #[serde(rename = "releaseId")]
        release_id: ReleaseId,
    },
    Range {
        #[serde(rename = "modId")]
        mod_id: ModId,
        #[serde(deserialize_with = "nullable_string")]
        minimum: Option<String>,
        #[serde(deserialize_with = "nullable_string")]
        before: Option<String>,
        #[serde(rename = "includePrerelease")]
        include_prerelease: bool,
    },
}

fn nullable_string<'de, D: serde::Deserializer<'de>>(input: D) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(input)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct DependencyProblem {
    pub dependency: Dependency,
    pub code: DependencyProblemCode,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum DependencyProblemCode {
    NoMatchingRelease,
    ArchiveUnavailable,
    SecurityBlocked,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Tombstone {
    pub mod_id: ModId,
    pub release_id: ReleaseId,
    pub sha256: Sha256,
    pub availability: Availability,
    #[serde(deserialize_with = "safe_u64")]
    #[cfg_attr(test, ts(type = "number"))]
    pub security_revision: u64,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum ReleaseResult {
    Release(Box<Release>),
    Tombstone(Tombstone),
}

impl ReleaseResult {
    pub(super) fn valid(&self) -> bool {
        match self {
            Self::Release(release) => {
                (1..=100).contains(&release.version_label.len())
                    && release.created_at.ends_with('Z')
                    && release.published_at.ends_with('Z')
                    && (1..=100).contains(&release.metadata.tested_game_build.len())
                    && release.metadata.release_notes.len() <= 20_000
                    && super::valid_dependencies(release.mod_id, &release.metadata.dependencies)
                    && release.metadata.dependency_problems.len() <= 100
                    && release.metadata.dependency_problems.iter().all(|problem| {
                        problem.dependency.valid(release.mod_id)
                            && release.metadata.dependencies.contains(&problem.dependency)
                    })
                    && release
                        .metadata
                        .installation
                        .as_ref()
                        .is_none_or(|plan| plan.valid())
                    && release.security.valid_for(&release.availability)
            }
            Self::Tombstone(tombstone) => {
                !matches!(tombstone.availability, Availability::Available)
                    && tombstone.updated_at.ends_with('Z')
            }
        }
    }
}
