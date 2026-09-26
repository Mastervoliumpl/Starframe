mod auth;
#[cfg(windows)]
mod credential;
mod dependencies;
pub mod installation;
mod transport;
pub mod trust;
mod wire;

pub use auth::{Auth, AuthError, ChallengeView, Poll, SignOut};
#[cfg(windows)]
pub use credential::CredentialStore;
pub(crate) use dependencies::valid_dependencies;
pub use transport::{Client, Config, Error, ErrorCode, FieldProblem, ResponseError};
pub(crate) use transport::{ReceiptClaim, VerifiedArchive};
pub use wire::{
    ApiResponse, Artifact, Availability, Dependency, ListQuery, Maintenance, ModList, ModSummary,
    Period, Release, ReleaseResult, Session, SessionContext, Sort,
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const CONTRACT_REVISION: &str = "fcd81be6667e2595166698178d5f55074c5ee92a";
pub const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct ModId(u64);

impl TryFrom<u64> for ModId {
    type Error = &'static str;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        if (1..=MAX_SAFE_INTEGER).contains(&value) {
            Ok(Self(value))
        } else {
            Err("ModID must be a positive safe integer")
        }
    }
}

impl From<ModId> for u64 {
    fn from(value: ModId) -> Self {
        value.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct PublicationOrder(u64);

impl TryFrom<u64> for PublicationOrder {
    type Error = &'static str;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        ModId::try_from(value)?;
        Ok(Self(value))
    }
}

impl From<PublicationOrder> for u64 {
    fn from(value: PublicationOrder) -> Self {
        value.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ReleaseId(pub Uuid);

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Sha256(String);

impl TryFrom<String> for Sha256 {
    type Error = &'static str;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.len() == 64
            && value
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        {
            Ok(Self(value))
        } else {
            Err("SHA-256 must be 64 lowercase hexadecimal characters")
        }
    }
}

impl From<Sha256> for String {
    fn from(value: Sha256) -> Self {
        value.0
    }
}

impl Sha256 {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExactReference {
    pub mod_id: ModId,
    pub release_id: ReleaseId,
    pub sha256: Sha256,
}

#[cfg(test)]
mod tests {
    use super::*;

    const WEBSITE_FIXTURES: &str = include_str!("../../tests/fixtures/registry-v1.json");

    #[test]
    fn exact_identity_round_trips_and_rejects_bad_wire_values() {
        let value = serde_json::json!({"modId": 9007199254740991u64, "releaseId": "00000000-0000-4000-8000-000000000001", "sha256": "a".repeat(64)});
        let reference: ExactReference = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(reference).unwrap(), value);
        for bad in [
            serde_json::json!(0),
            serde_json::json!(9007199254740992u64),
            serde_json::json!("1"),
            serde_json::json!(1.5),
        ] {
            assert!(serde_json::from_value::<ModId>(bad).is_err());
        }
        assert!(Sha256::try_from("A".repeat(64)).is_err());
        assert!(serde_json::from_value::<ExactReference>(serde_json::json!({"modId":1,"releaseId":"00000000-0000-4000-8000-000000000001","sha256":"a".repeat(64),"unexpected":true})).is_err());
    }

    #[test]
    fn pinned_website_examples_parse_as_closed_native_responses() {
        let fixtures: serde_json::Value = serde_json::from_str(WEBSITE_FIXTURES).unwrap();
        for fixture in fixtures.as_array().unwrap() {
            let name = fixture["name"].as_str().unwrap();
            let body = fixture["body"].clone();
            match name {
                "approved" | "unavailable" | "blocked" => {
                    let response: ApiResponse<ReleaseResult> =
                        serde_json::from_value(body).unwrap();
                    match response.data {
                        ReleaseResult::Release(release) => {
                            assert_eq!(u64::from(release.mod_id), 1);
                            assert_ne!(name, "unavailable");
                            if name == "approved" {
                                assert_eq!(release.artifact.bytes, 2_147_483_648);
                            }
                        }
                        ReleaseResult::Tombstone(tombstone) => {
                            assert_eq!(name, "unavailable");
                            assert_eq!(u64::from(tombstone.mod_id), 1);
                        }
                    }
                }
                "list" => {
                    let response: ModList = serde_json::from_value(body).unwrap();
                    assert_eq!(response.items.len(), 1);
                }
                "unauthorised" | "restricted" | "hidden-from-stranger" | "security-stale" => {
                    let response: transport::ErrorEnvelope = serde_json::from_value(body).unwrap();
                    assert_eq!(response.api_version, 1);
                }
                _ => {}
            }
        }
    }
}
