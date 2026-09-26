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

#[derive(Clone, Debug, Deserialize)]
#[serde(try_from = "u8")]
pub struct ApiVersion;

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

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApiResponse<T> {
    pub api_version: ApiVersion,
    pub data: T,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModList {
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
    }
}

#[derive(Clone, Debug)]
pub struct ListQuery {
    pub page: u64,
    pub page_size: u8,
    pub query: String,
    pub include_tags: Vec<String>,
    pub exclude_tags: Vec<String>,
    pub sort: Sort,
    pub period: Period,
    pub maintenance: Maintenance,
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
                    .all(|item| !item.is_empty() && item.len() <= length)
                && items.iter().collect::<std::collections::HashSet<_>>().len() == items.len()
        };
        if !(1..=super::MAX_SAFE_INTEGER).contains(&self.page)
            || ![6, 10, 12, 20, 24, 48, 50].contains(&self.page_size)
            || self.query.len() > 200
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

#[derive(Clone, Copy, Debug)]
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

#[derive(Clone, Copy, Debug)]
pub enum Period {
    All,
    Day,
    Week,
    Month,
    ThreeMonths,
    SixMonths,
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

#[derive(Clone, Copy, Debug)]
pub enum Maintenance {
    All,
    Maintained,
    Unmaintained,
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

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Pagination {
    #[serde(deserialize_with = "positive_u64")]
    pub page: u64,
    #[serde(deserialize_with = "positive_u64")]
    pub page_size: u64,
    #[serde(deserialize_with = "safe_u64")]
    pub total_items: u64,
    #[serde(deserialize_with = "safe_u64")]
    pub total_pages: u64,
    pub as_of: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModSummary {
    pub mod_id: ModId,
    pub name: String,
    pub summary: String,
    pub owner: Option<Profile>,
    pub tags: Vec<String>,
    pub maintained: bool,
    pub replacement_mod_id: Option<ModId>,
    pub updated_at: String,
    pub latest_release_id: Option<ReleaseId>,
    #[serde(deserialize_with = "safe_u64")]
    pub downloads: u64,
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
                .owner
                .as_ref()
                .is_none_or(|owner| (1..=120).contains(&owner.display_name.len()))
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Availability {
    Available,
    Withdrawn,
    Hidden,
    Pruned,
    Blocked,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Artifact {
    pub sha256: Sha256,
    #[serde(deserialize_with = "positive_u64")]
    pub bytes: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Release {
    pub mod_id: ModId,
    pub release_id: ReleaseId,
    pub version_label: String,
    pub artifact: Artifact,
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

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Security {
    pub status: SecurityStatus,
    #[serde(deserialize_with = "safe_u64")]
    pub revision: u64,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityStatus {
    NotBlocked,
    Blocked,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Metadata {
    #[serde(deserialize_with = "positive_u64")]
    pub revision: u64,
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
        minimum: Option<String>,
        before: Option<String>,
        #[serde(rename = "includePrerelease")]
        include_prerelease: bool,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DependencyProblem {
    pub dependency: Dependency,
    pub code: DependencyProblemCode,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyProblemCode {
    NoMatchingRelease,
    ArchiveUnavailable,
    SecurityBlocked,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Tombstone {
    pub mod_id: ModId,
    pub release_id: ReleaseId,
    pub sha256: Sha256,
    pub availability: Availability,
    #[serde(deserialize_with = "safe_u64")]
    pub security_revision: u64,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
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
                    && release.metadata.dependencies.len() <= 100
                    && release.metadata.dependency_problems.len() <= 100
                    && release
                        .metadata
                        .installation
                        .as_ref()
                        .is_none_or(|plan| plan.valid())
                    && release
                        .security
                        .reason
                        .as_ref()
                        .is_none_or(|reason| (1..=1000).contains(&reason.len()))
                    && matches!(
                        (&release.availability, &release.security.status),
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
            Self::Tombstone(tombstone) => {
                !matches!(tombstone.availability, Availability::Available)
                    && tombstone.updated_at.ends_with('Z')
            }
        }
    }
}
