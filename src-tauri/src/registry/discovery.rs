use super::{ApiResponse, Client, Error, ModId, PublicationOrder, ReleaseId, wire};
use serde::{Deserialize, Serialize};
use tokio::sync::watch;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Tag {
    pub id: String,
    pub label: String,
    pub group_name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Options {
    pub tags: Vec<Tag>,
    pub game_builds: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ReleaseSummary {
    pub mod_id: ModId,
    pub release_id: ReleaseId,
    pub version_label: String,
    pub artifact: wire::Artifact,
    #[cfg_attr(test, ts(type = "'approved'"))]
    pub submission_state: wire::Approved,
    pub publication_order: PublicationOrder,
    pub published_at: String,
    pub created_at: String,
    pub availability: wire::Availability,
    pub security: wire::Security,
    #[cfg_attr(test, ts(type = "number"))]
    pub metadata_revision: u64,
    pub tested_game_build: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum HistoryEntry {
    Release(Box<ReleaseSummary>),
    Tombstone(wire::Tombstone),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ReleaseHistory {
    #[cfg_attr(test, ts(type = "1"))]
    pub api_version: wire::ApiVersion,
    pub items: Vec<HistoryEntry>,
    pub pagination: wire::Pagination,
}

fn text(value: &str, limit: usize) -> bool {
    !value.is_empty()
        && value.encode_utf16().count() <= limit
        && !value.chars().any(char::is_control)
}

impl Client {
    pub async fn discovery_options(
        &self,
        bearer: &str,
        cancel: watch::Receiver<bool>,
    ) -> Result<Options, Error> {
        let result: ApiResponse<Options> =
            self.get("/registry/options", Some(bearer), cancel).await?;
        let options = result.data;
        if options.tags.len() > 200
            || options.game_builds.len() > 200
            || options.tags.iter().any(|tag| {
                !text(&tag.id, 64) || !text(&tag.label, 100) || !text(&tag.group_name, 64)
            })
            || options.game_builds.iter().any(|build| !text(build, 100))
            || options
                .tags
                .iter()
                .map(|tag| &tag.id)
                .collect::<std::collections::HashSet<_>>()
                .len()
                != options.tags.len()
            || options
                .game_builds
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                != options.game_builds.len()
        {
            return Err(Error::Protocol);
        }
        Ok(options)
    }

    pub async fn release_history(
        &self,
        mod_id: ModId,
        page: u64,
        page_size: u8,
        bearer: &str,
        cancel: watch::Receiver<bool>,
    ) -> Result<ReleaseHistory, Error> {
        if !(1..=super::MAX_SAFE_INTEGER).contains(&page) || ![12, 24, 48].contains(&page_size) {
            return Err(Error::InvalidQuery);
        }
        let result: ReleaseHistory = self
            .get_query(
                &format!("/registry/mods/{}/releases", u64::from(mod_id)),
                &[
                    ("page", page.to_string()),
                    ("pageSize", page_size.to_string()),
                ],
                Some(bearer),
                cancel,
            )
            .await?;
        if result.items.len() > usize::from(page_size)
            || result.pagination.page_size != u64::from(page_size)
            || !result.pagination.as_of.ends_with('Z')
        {
            return Err(Error::Protocol);
        }
        let mut ids = std::collections::HashSet::new();
        let mut previous = None;
        for entry in &result.items {
            match entry {
                HistoryEntry::Release(release) => {
                    if release.mod_id != mod_id
                        || !text(&release.version_label, 100)
                        || !text(&release.tested_game_build, 100)
                        || !(1..=super::MAX_SAFE_INTEGER).contains(&release.metadata_revision)
                        || release.artifact.bytes == 0
                        || release.artifact.bytes > super::MAX_SAFE_INTEGER
                        || !release.published_at.ends_with('Z')
                        || !release.created_at.ends_with('Z')
                        || !release.security.valid_for(&release.availability)
                        || !ids.insert(release.release_id)
                        || previous.is_some_and(|previous| {
                            u64::from(release.publication_order) >= previous
                        })
                    {
                        return Err(Error::Protocol);
                    }
                    previous = Some(u64::from(release.publication_order));
                }
                HistoryEntry::Tombstone(release) => {
                    if release.mod_id != mod_id
                        || !ids.insert(release.release_id)
                        || !wire::ReleaseResult::Tombstone(release.clone()).valid()
                    {
                        return Err(Error::Protocol);
                    }
                }
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ts_rs::TS;

    #[test]
    fn generated_discovery_contract_matches_rust() {
        let config = ts_rs::Config::default();
        let declarations = [
            super::super::ModId::decl(&config),
            super::super::PublicationOrder::decl(&config),
            super::super::ReleaseId::decl(&config),
            super::super::Sha256::decl(&config),
            wire::ListQuery::decl(&config),
            wire::Sort::decl(&config),
            wire::Period::decl(&config),
            wire::Maintenance::decl(&config),
            wire::ModTypeFilter::decl(&config),
            wire::Profile::decl(&config),
            wire::ModSummary::decl(&config),
            wire::ModType::decl(&config),
            wire::Pagination::decl(&config),
            wire::ModList::decl(&config),
            wire::ModMedia::decl(&config),
            wire::ModDetail::decl(&config),
            wire::ModResult::decl(&config),
            wire::Availability::decl(&config),
            wire::Artifact::decl(&config),
            wire::Release::decl(&config),
            wire::Security::decl(&config),
            wire::SecurityStatus::decl(&config),
            wire::Metadata::decl(&config),
            wire::Dependency::decl(&config),
            wire::DependencyProblem::decl(&config),
            wire::DependencyProblemCode::decl(&config),
            wire::Tombstone::decl(&config),
            wire::ReleaseResult::decl(&config),
            super::super::installation::Kind::decl(&config),
            super::super::installation::Loader::decl(&config),
            super::super::installation::Destination::decl(&config),
            super::super::installation::Installation::decl(&config),
            Tag::decl(&config),
            Options::decl(&config),
            ReleaseSummary::decl(&config),
            HistoryEntry::decl(&config),
            ReleaseHistory::decl(&config),
        ];
        let content = format!(
            "// Generated by the Rust discovery contract test. Do not edit.\n{}",
            declarations
                .iter()
                .map(|declaration| format!("export {declaration}\n"))
                .collect::<String>()
        );
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../src/lib/generated/registry.ts");
        if std::env::var_os("UPDATE_BINDINGS").is_some() {
            std::fs::write(&path, &content).unwrap();
        }
        assert_eq!(std::fs::read_to_string(path).unwrap(), content);
    }
}
