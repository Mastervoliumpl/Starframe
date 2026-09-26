use crate::{
    registry::{ExactReference, Sha256},
    storage::{ModReference, Origin},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalReference {
    pub mod_id: String,
    pub sha256: Sha256,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "reference",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Reference {
    Registry(ExactReference),
    Local(LocalReference),
}

impl Reference {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Registry(reference) => {
                crate::registry::ReleaseId::try_from(reference.release_id.0.to_string())
                    .map_err(str::to_owned)?;
            }
            Self::Local(local) => {
                crate::catalog::id(&local.mod_id)?;
                if local.mod_id.len() > 128 {
                    return Err("Local mod IDs must fit the runtime's 128-byte limit.".into());
                }
            }
        }
        Ok(())
    }

    pub(crate) fn runtime_id(&self) -> String {
        match self {
            Self::Registry(reference) => crate::registry::activation::runtime_id(reference),
            Self::Local(reference) => reference.mod_id.clone(),
        }
    }

    pub(crate) fn hash(&self) -> &str {
        match self {
            Self::Registry(reference) => reference.sha256.as_str(),
            Self::Local(reference) => reference.sha256.as_str(),
        }
    }

    pub(crate) fn local_reference(&self) -> Option<ModReference> {
        match self {
            Self::Local(reference) => Some(ModReference {
                mod_id: reference.mod_id.clone(),
                hash: reference.sha256.as_str().to_owned(),
                origin: Origin::LocalImport,
                release_id: None,
            }),
            Self::Registry(_) => None,
        }
    }
}

impl TryFrom<&ModReference> for Reference {
    type Error = String;

    fn try_from(reference: &ModReference) -> Result<Self, String> {
        if reference.origin != Origin::LocalImport || reference.release_id.is_some() {
            return Err("An obsolete catalog dependency cannot authorize a registry release. Update the local declaration with an exact supported reference.".into());
        }
        let value = Self::Local(LocalReference {
            mod_id: reference.mod_id.clone(),
            sha256: Sha256::try_from(reference.hash.clone()).map_err(str::to_owned)?,
        });
        value.validate()?;
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn closed_references_preserve_numeric_registry_and_distinct_local_identity() {
        let registry = json!({"kind":"registry","reference":{"modId":1,"releaseId":"11111111-1111-4111-8111-111111111111","sha256":"a".repeat(64)}});
        let local =
            json!({"kind":"local","reference":{"modId":"example.local","sha256":"a".repeat(64)}});
        for value in [&registry, &local] {
            let reference: Reference = serde_json::from_value(value.clone()).unwrap();
            reference.validate().unwrap();
            assert_eq!(serde_json::to_value(reference).unwrap(), *value);
        }
        for bad in [
            json!({"kind":"catalog","reference":registry["reference"]}),
            json!({"kind":"registry","reference":local["reference"]}),
            json!({"kind":"local","reference":{"modId":"example.local","sha256":"a".repeat(64),"url":"https://example.invalid"}}),
            json!({"kind":"local","reference":local["reference"],"token":"secret"}),
        ] {
            assert!(serde_json::from_value::<Reference>(bad).is_err());
        }
        let old = ModReference {
            mod_id: "old.mod".into(),
            hash: "a".repeat(64),
            origin: Origin::Catalog,
            release_id: Some("old.release".into()),
        };
        assert!(Reference::try_from(&old).is_err());
    }
}
