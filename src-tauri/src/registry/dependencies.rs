use super::{Dependency, ModId, ReleaseId};
use std::cmp::Ordering;

struct Version<'a> {
    core: [&'a str; 3],
    pre: Vec<&'a str>,
}

fn numeric(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && (value == "0" || !value.starts_with('0'))
}

fn identifiers(value: &str) -> Option<Vec<&str>> {
    let parts: Vec<_> = value.split('.').collect();
    parts
        .iter()
        .all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
        .then_some(parts)
}

impl<'a> Version<'a> {
    fn parse(value: &'a str) -> Option<Self> {
        if value.len() > 100 {
            return None;
        }
        let (version, build) = value
            .split_once('+')
            .map_or((value, None), |(version, build)| (version, Some(build)));
        if build.is_some_and(|build| identifiers(build).is_none()) {
            return None;
        }
        let (core, pre) = version
            .split_once('-')
            .map_or((version, None), |(core, pre)| (core, Some(pre)));
        let core: [&str; 3] = core.split('.').collect::<Vec<_>>().try_into().ok()?;
        if !core.iter().all(|value| numeric(value)) {
            return None;
        }
        let pre = match pre {
            Some(pre) => identifiers(pre)?,
            None => vec![],
        };
        if pre
            .iter()
            .any(|part| part.bytes().all(|byte| byte.is_ascii_digit()) && !numeric(part))
        {
            return None;
        }
        Some(Self { core, pre })
    }

    fn compare(&self, other: &Self) -> Ordering {
        // The website accepts arbitrary-length SemVer numbers within its text limit.
        let number = |left: &str, right: &str| left.len().cmp(&right.len()).then(left.cmp(right));
        for (left, right) in self.core.iter().zip(other.core.iter()) {
            let comparison = number(left, right);
            if comparison != Ordering::Equal {
                return comparison;
            }
        }
        if self.pre.is_empty() || other.pre.is_empty() {
            return self.pre.is_empty().cmp(&other.pre.is_empty());
        }
        for (left, right) in self.pre.iter().zip(other.pre.iter()) {
            let left_number = left.bytes().all(|byte| byte.is_ascii_digit());
            let right_number = right.bytes().all(|byte| byte.is_ascii_digit());
            let comparison = match (left_number, right_number) {
                (true, true) => number(left, right),
                (true, false) => Ordering::Less,
                (false, true) => Ordering::Greater,
                (false, false) => left.cmp(right),
            };
            if comparison != Ordering::Equal {
                return comparison;
            }
        }
        self.pre.len().cmp(&other.pre.len())
    }
}

impl Dependency {
    pub fn mod_id(&self) -> ModId {
        match self {
            Self::Exact { mod_id, .. } | Self::Range { mod_id, .. } => *mod_id,
        }
    }

    pub fn valid(&self, source: ModId) -> bool {
        if self.mod_id() == source {
            return false;
        }
        match self {
            Self::Exact { release_id, .. } => {
                release_id.0.get_version_num() == 4
                    && release_id.0.get_variant() == uuid::Variant::RFC4122
            }
            Self::Range {
                minimum, before, ..
            } => {
                let minimum_version = minimum.as_deref().and_then(Version::parse);
                let before_version = before.as_deref().and_then(Version::parse);
                (minimum.is_some() || before.is_some())
                    && minimum.is_some() == minimum_version.is_some()
                    && before.is_some() == before_version.is_some()
                    && match (minimum_version, before_version) {
                        (Some(minimum), Some(before)) => minimum.compare(&before) == Ordering::Less,
                        _ => true,
                    }
            }
        }
    }

    pub fn matches(&self, mod_id: ModId, release_id: ReleaseId, version_label: &str) -> bool {
        if self.mod_id() != mod_id {
            return false;
        }
        match self {
            Self::Exact {
                release_id: expected,
                ..
            } => *expected == release_id,
            Self::Range {
                minimum,
                before,
                include_prerelease,
                ..
            } => {
                let Some(candidate) = Version::parse(version_label) else {
                    return false;
                };
                (*include_prerelease || candidate.pre.is_empty())
                    && minimum.as_deref().is_none_or(|minimum| {
                        Version::parse(minimum)
                            .is_some_and(|minimum| candidate.compare(&minimum) != Ordering::Less)
                    })
                    && before.as_deref().is_none_or(|before| {
                        Version::parse(before)
                            .is_some_and(|before| candidate.compare(&before) == Ordering::Less)
                    })
                    && (minimum.is_some() || before.is_some())
            }
        }
    }

    pub fn suggest(&self, candidates: &[super::Release]) -> Option<super::ExactReference> {
        candidates
            .iter()
            .filter(|candidate| {
                matches!(candidate.availability, super::Availability::Available)
                    && matches!(
                        candidate.security.status,
                        super::wire::SecurityStatus::NotBlocked
                    )
                    && candidate
                        .metadata
                        .installation
                        .as_ref()
                        .is_some_and(|plan| plan.valid())
                    && self.matches(
                        candidate.mod_id,
                        candidate.release_id,
                        &candidate.version_label,
                    )
            })
            .max_by_key(|candidate| u64::from(candidate.publication_order))
            .map(|candidate| super::ExactReference {
                mod_id: candidate.mod_id,
                release_id: candidate.release_id,
                sha256: candidate.artifact.sha256.clone(),
            })
    }
}

pub(crate) fn valid_dependencies(source: ModId, dependencies: &[Dependency]) -> bool {
    let mut seen = std::collections::HashSet::new();
    dependencies.len() <= 100
        && dependencies
            .iter()
            .all(|dependency| dependency.valid(source) && seen.insert(dependency.mod_id()))
}

#[cfg(test)]
mod tests;
