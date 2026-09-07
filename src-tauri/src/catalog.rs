use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub mod refresh;

pub const ENDPOINT: &str =
    "https://raw.githubusercontent.com/Mastervoliumpl/Starframe/main/catalog/releases.json";
pub const MAX_BYTES: usize = 2 * 1024 * 1024;
type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Catalog {
    pub schema_version: u32,
    pub catalog_revision: String,
    pub mods: Vec<Mod>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Mod {
    pub id: String,
    pub name: String,
    pub author: String,
    pub source_url: String,
    pub releases: Vec<Release>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Release {
    pub id: String,
    pub version: String,
    pub withdrawn: bool,
    pub artifact: Artifact,
    pub requires: Vec<String>,
    pub tested_game_builds: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Artifact {
    pub url: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub layout: Layout,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Layout {
    #[serde(rename_all = "camelCase")]
    StarframeManagedZip {
        root: String,
        entry_assembly: String,
        entry_type: String,
    },
}

fn ensure(condition: bool, message: &str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("Invalid catalog: {message}."))
    }
}

fn text(value: &str, limit: usize) -> Result<()> {
    ensure(
        !value.trim().is_empty() && value.len() <= limit && !value.chars().any(char::is_control),
        "empty, oversized or control-character text",
    )
}

fn id(value: &str) -> Result<()> {
    ensure(
        !value.is_empty()
            && value.len() <= 128
            && value.bytes().enumerate().all(|(i, c)| {
                c.is_ascii_lowercase() || c.is_ascii_digit() || (i > 0 && b"._-".contains(&c))
            }),
        "IDs must start with a lowercase ASCII letter or digit and contain at most 128 letters, digits, dots, underscores or hyphens",
    )
}

fn https(value: &str) -> Result<()> {
    text(value, 2048)?;
    let url = reqwest::Url::parse(value).map_err(|_| "Invalid catalog: malformed URL.")?;
    ensure(
        url.scheme() == "https"
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none(),
        "URLs require HTTPS without credentials or fragments",
    )
}

impl Catalog {
    pub fn read(bytes: &[u8]) -> Result<Self> {
        ensure(bytes.len() <= MAX_BYTES, "document exceeds 2 MiB")?;
        let catalog: Self =
            serde_json::from_slice(bytes).map_err(|e| format!("Invalid catalog JSON: {e}"))?;
        catalog.validate()?;
        Ok(catalog)
    }

    pub fn revision(&self) -> Result<u64> {
        let revision: u64 = self
            .catalog_revision
            .parse()
            .map_err(|_| "Invalid catalog revision.")?;
        ensure(
            revision > 0 && revision.to_string() == self.catalog_revision,
            "revision must be a positive decimal string without leading zeros",
        )?;
        Ok(revision)
    }

    pub fn releases(&self) -> impl Iterator<Item = (&Mod, &Release)> {
        self.mods
            .iter()
            .flat_map(|m| m.releases.iter().map(move |r| (m, r)))
    }

    pub fn validate(&self) -> Result<()> {
        ensure(self.schema_version == 1, "unsupported schema version")?;
        self.revision()?;
        ensure(self.mods.len() <= 1024, "too many mods")?;
        let mut mods = HashSet::new();
        let mut releases = HashMap::new();
        for m in &self.mods {
            id(&m.id)?;
            ensure(mods.insert(&m.id), "duplicate mod ID")?;
            text(&m.name, 200)?;
            text(&m.author, 200)?;
            https(&m.source_url)?;
            ensure(
                !m.releases.is_empty() && m.releases.len() <= 256,
                "each mod needs 1–256 releases",
            )?;
            for r in &m.releases {
                id(&r.id)?;
                ensure(
                    releases.insert(&r.id, (m, r)).is_none(),
                    "duplicate release ID",
                )?;
                text(&r.version, 128)?;
                https(&r.artifact.url)?;
                ensure(
                    r.artifact.sha256.len() == 64
                        && r.artifact
                            .sha256
                            .bytes()
                            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)),
                    "expected lowercase SHA-256",
                )?;
                ensure(
                    (1..=2_147_483_648).contains(&r.artifact.size_bytes),
                    "artifact size must be 1 byte to 2 GiB",
                )?;
                let Layout::StarframeManagedZip {
                    root,
                    entry_assembly,
                    entry_type,
                } = &r.artifact.layout;
                if !root.is_empty() {
                    crate::runtime_contract::relative_path(root)?;
                }
                crate::runtime_contract::relative_path(entry_assembly)?;
                ensure(
                    entry_assembly.ends_with(".dll"),
                    "managed entry must be a DLL",
                )?;
                text(entry_type, 256)?;
                ensure(
                    entry_type.split('.').all(|part| {
                        !part.is_empty()
                            && part.bytes().enumerate().all(|(i, c)| {
                                c.is_ascii_alphabetic()
                                    || c == b'_'
                                    || (i > 0 && c.is_ascii_digit())
                            })
                    }),
                    "invalid managed entry type",
                )?;
                ensure(
                    r.requires.len() <= 64 && r.tested_game_builds.len() <= 64,
                    "too many dependencies or game builds",
                )?;
                let mut required = HashSet::new();
                for dependency in &r.requires {
                    id(dependency)?;
                    ensure(
                        required.insert(dependency) && dependency != &r.id,
                        "duplicate or self dependency",
                    )?;
                }
                let mut builds = HashSet::new();
                for build in &r.tested_game_builds {
                    text(build, 128)?;
                    ensure(builds.insert(build), "duplicate tested build")?;
                }
            }
        }
        ensure(releases.len() <= 4096, "too many releases")?;
        let mut indegree = HashMap::new();
        let mut dependents: HashMap<&String, Vec<&String>> = HashMap::new();
        for (release_id, (m, r)) in &releases {
            let mut required_mods = HashSet::new();
            for dependency in &r.requires {
                let (dependency_mod, _) = releases
                    .get(dependency)
                    .ok_or_else(|| format!("Invalid catalog: missing dependency {dependency}."))?;
                ensure(
                    dependency_mod.id != m.id && required_mods.insert(&dependency_mod.id),
                    "dependencies require distinct other mods",
                )?;
                dependents.entry(dependency).or_default().push(release_id);
            }
            indegree.insert(*release_id, r.requires.len());
        }
        let mut ready: Vec<_> = indegree
            .iter()
            .filter_map(|(id, count)| (*count == 0).then_some(*id))
            .collect();
        let mut visited = 0;
        while let Some(id) = ready.pop() {
            visited += 1;
            for dependent in dependents.get(id).into_iter().flatten() {
                let count = indegree.get_mut(dependent).unwrap();
                *count -= 1;
                if *count == 0 {
                    ready.push(dependent);
                }
            }
        }
        ensure(visited == releases.len(), "dependency cycle")
    }

    pub fn accepts_after(&self, previous: &Self) -> Result<()> {
        let revision = self.revision()?;
        ensure(revision >= previous.revision()?, "revision moved backwards")?;
        if revision == previous.revision()? {
            return ensure(self == previous, "changed content needs a new revision");
        }
        let current: HashMap<_, _> = self.releases().map(|(m, r)| (&r.id, (m, r))).collect();
        for (old_mod, old) in previous.releases() {
            let (m, r) = current.get(&old.id).ok_or_else(|| {
                format!(
                    "Retain release {} and mark it withdrawn instead of deleting it.",
                    old.id
                )
            })?;
            ensure(
                m.id == old_mod.id
                    && r.version == old.version
                    && r.artifact.sha256 == old.artifact.sha256
                    && r.artifact.size_bytes == old.artifact.size_bytes
                    && r.artifact.layout == old.artifact.layout
                    && r.requires == old.requires,
                "existing release identity changed; use a new release ID",
            )?;
            ensure(
                !old.withdrawn || r.withdrawn,
                "withdrawn release IDs cannot be reused",
            )?;
        }
        Ok(())
    }

    pub fn downloadable(&self, release_id: &str) -> Result<&Artifact> {
        let releases: HashMap<_, _> = self.releases().map(|(_, r)| (r.id.as_str(), r)).collect();
        let mut pending = vec![release_id];
        let mut visited = HashSet::new();
        let mut required_mods = HashMap::new();
        let owners: HashMap<_, _> = self
            .releases()
            .map(|(m, r)| (r.id.as_str(), m.id.as_str()))
            .collect();
        while let Some(id) = pending.pop() {
            if !visited.insert(id) {
                continue;
            }
            let release = releases
                .get(id)
                .ok_or_else(|| format!("Release {id} is not in the approved catalog."))?;
            if release.withdrawn {
                return Err(format!(
                    "Release {id} was withdrawn. New downloads are blocked."
                ));
            }
            if required_mods
                .insert(owners[id], id)
                .is_some_and(|other| other != id)
            {
                return Err("Dependencies require different releases of the same mod.".into());
            }
            pending.extend(release.requires.iter().map(String::as_str));
        }
        Ok(&releases[release_id].artifact)
    }
}
