use super::{ensure, https, id, text};
use crate::packages::PreparedFile;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const MAX_BYTES: usize = 1_048_576;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Advisories {
    pub schema_version: u32,
    pub revision: String,
    pub advisories: Vec<Advisory>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Advisory {
    pub id: String,
    pub title: String,
    pub affected: Vec<AffectedArtifact>,
    pub history: Vec<Finding>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct AffectedArtifact {
    pub release_id: String,
    pub sha256: String,
    pub payload_sha256: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Finding {
    #[cfg_attr(test, ts(type = "number"))]
    pub recorded_at: u64,
    pub state: State,
    pub explanation: String,
    pub evidence: Vec<String>,
    pub recommended_action: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(rename = "FindingState"))]
pub enum State {
    Suspected,
    Confirmed,
    Cleared,
}

fn hash(value: &str) -> Result<(), String> {
    ensure(
        value.len() == 64
            && value
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)),
        "invalid advisory SHA-256",
    )
}

impl Advisories {
    pub fn read(bytes: &[u8]) -> Result<Self, String> {
        ensure(bytes.len() <= MAX_BYTES, "advisories exceed 1 MiB")?;
        let value: Self =
            serde_json::from_slice(bytes).map_err(|e| format!("Invalid advisory JSON: {e}"))?;
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<(), String> {
        ensure(self.schema_version == 1, "unsupported advisory schema")?;
        self.revision_number()?;
        ensure(self.advisories.len() <= 256, "too many advisories")?;
        let mut ids = HashSet::new();
        for advisory in &self.advisories {
            id(&advisory.id)?;
            ensure(ids.insert(&advisory.id), "duplicate advisory ID")?;
            text(&advisory.title, 200)?;
            ensure(
                !advisory.affected.is_empty() && advisory.affected.len() <= 16,
                "invalid affected artifact count",
            )?;
            let mut artifacts = HashSet::new();
            for affected in &advisory.affected {
                id(&affected.release_id)?;
                hash(&affected.sha256)?;
                ensure(
                    artifacts.insert((&affected.release_id, &affected.sha256)),
                    "duplicate affected artifact",
                )?;
                ensure(
                    affected.payload_sha256.len() <= 64,
                    "too many affected payload hashes",
                )?;
                let mut hashes = HashSet::new();
                for payload in &affected.payload_sha256 {
                    hash(payload)?;
                    ensure(hashes.insert(payload), "duplicate affected payload hash")?;
                }
            }
            ensure(
                !advisory.history.is_empty() && advisory.history.len() <= 64,
                "invalid advisory history count",
            )?;
            let mut previous = 0;
            for finding in &advisory.history {
                ensure(
                    finding.recorded_at > previous && finding.recorded_at <= 253_402_300_799,
                    "advisory history timestamps must increase",
                )?;
                previous = finding.recorded_at;
                text(&finding.explanation, 4000)?;
                text(&finding.recommended_action, 2000)?;
                ensure(
                    !finding.evidence.is_empty() && finding.evidence.len() <= 16,
                    "advisories require bounded evidence links",
                )?;
                for url in &finding.evidence {
                    https(url)?;
                }
            }
        }
        Ok(())
    }
    fn revision_number(&self) -> Result<u64, String> {
        let revision: u64 = self
            .revision
            .parse()
            .map_err(|_| "Invalid advisory revision.")?;
        ensure(
            revision > 0 && revision.to_string() == self.revision,
            "invalid advisory revision",
        )?;
        Ok(revision)
    }
    pub fn accepts_after(&self, previous: &Self) -> Result<(), String> {
        self.validate()?;
        ensure(
            self.revision_number()? >= previous.revision_number()?,
            "advisory revision rollback",
        )?;
        if self.revision == previous.revision {
            return ensure(
                self == previous,
                "advisory content changed without a revision",
            );
        }
        for old in &previous.advisories {
            let current = self.advisories.iter().find(|a| a.id == old.id).ok_or(
                "Previously received advisories must be retained; append a correction instead.",
            )?;
            ensure(
                current.affected == old.affected && current.history.starts_with(&old.history),
                "advisory identity or history was rewritten",
            )?;
            ensure(
                current == old || current.history.len() > old.history.len(),
                "advisory edits require a history entry",
            )?;
        }
        Ok(())
    }
    pub fn findings_for(&self, archive_hash: &str, files: &[PreparedFile]) -> Vec<&Advisory> {
        self.advisories
            .iter()
            .filter(|advisory| {
                advisory
                    .history
                    .last()
                    .is_some_and(|finding| finding.state != State::Cleared)
                    && advisory.affected.iter().any(|affected| {
                        affected.sha256 == archive_hash
                            || files
                                .iter()
                                .any(|file| affected.payload_sha256.contains(&file.sha256))
                    })
            })
            .collect()
    }
}

impl Advisory {
    pub fn current(&self) -> &Finding {
        self.history.last().expect("validated advisory history")
    }
}

#[cfg(test)]
mod tests;
