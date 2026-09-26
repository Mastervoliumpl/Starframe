use super::{Config, CredentialStore};
use serde::{Deserialize, de::DeserializeOwned};
use std::{fs, io::Read, path::Path};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Fixture {
    schema_version: u8,
    api_url: String,
    website_url: String,
    pub(super) root_public: [u8; 32],
}

impl Fixture {
    pub(super) fn connection(&self) -> Result<Config, String> {
        let invalid = || "Invalid isolated registry fixture connection.".to_owned();
        let api = reqwest::Url::parse(&self.api_url).map_err(|_| invalid())?;
        let website = reqwest::Url::parse(&self.website_url).map_err(|_| invalid())?;
        let loopback = |url: &reqwest::Url| {
            url.scheme() == "http"
                && url.host_str() == Some("127.0.0.1")
                && url.port() != Some(0)
                && url.username().is_empty()
                && url.password().is_none()
                && url.query().is_none()
                && url.fragment().is_none()
        };
        if self.schema_version != 1
            || !loopback(&api)
            || !loopback(&website)
            || api.origin() != website.origin()
            || api.path() != "/v1"
            || website.path() != "/"
        {
            return Err(invalid());
        }
        Config::new(&self.api_url, &self.website_url, true).map_err(|_| invalid())
    }
}

fn optional<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, String> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("Could not read isolated registry fixture.".into()),
    };
    let mut bytes = Vec::new();
    file.take(4097)
        .read_to_end(&mut bytes)
        .map_err(|_| "Could not read isolated registry fixture.")?;
    if bytes.len() > 4096 {
        return Err("Isolated registry fixture is too large.".into());
    }
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| "Invalid isolated registry fixture.".into())
}

pub(super) fn read(root: &Path) -> Result<Option<Fixture>, String> {
    optional(&root.join("registry-fixture.json"))
}

pub(super) fn seed(root: &Path, store: &CredentialStore) -> Result<(), String> {
    let path = root.join("registry-credential.fixture.json");
    if let Some(token) = optional::<starframe::registry::StoredToken>(&path)? {
        store.save(&token).map_err(str::to_owned)?;
        if fs::remove_file(path).is_err() {
            let _ = store.delete();
            return Err("Could not consume isolated registry credential fixture.".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixture_connection_cannot_use_production_or_foreign_origins() {
        let base = serde_json::json!({"schemaVersion":1,"apiUrl":"http://127.0.0.1:43210/v1","websiteUrl":"http://127.0.0.1:43210/","rootPublic":vec![7;32]});
        for case in 0..8 {
            let mut value = base.clone();
            match case {
                1 => value["apiUrl"] = "https://api.starframemanager.com/v1".into(),
                2 => value["websiteUrl"] = "http://127.0.0.1:43211/".into(),
                3 => value["apiUrl"] = "http://localhost:43210/v1".into(),
                4 => value["apiUrl"] = "http://127.0.0.1:43210/v1?override=1".into(),
                5 => value["apiUrl"] = "http://fixture@127.0.0.1:43210/v1".into(),
                6 => value["schemaVersion"] = 2.into(),
                7 => value["privateKey"] = "forbidden field".into(),
                _ => (),
            }
            let valid = serde_json::from_value::<Fixture>(value)
                .ok()
                .is_some_and(|fixture| fixture.connection().is_ok());
            assert_eq!(valid, case == 0, "case {case}");
        }
    }
}
