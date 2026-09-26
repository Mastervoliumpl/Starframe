use base64::{Engine as _, engine::general_purpose::STANDARD};
use ring::signature::{ED25519, UnparsedPublicKey};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fmt::Write,
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

const MAX_SIGNED_BYTES: usize = 4 * 1024 * 1024;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Signature {
    key_id: String,
    algorithm: String,
    signature: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Key {
    key_id: String,
    public_key: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Keyset {
    #[serde(rename = "type")]
    kind: String,
    schema_version: u8,
    revision: u64,
    issued_at: String,
    expires_at: String,
    keys: Vec<Key>,
}

pub struct VerifiedKeyset {
    pub(crate) revision: u64,
    pub(crate) canonical: String,
    pub(crate) expires: OffsetDateTime,
    pub(crate) keys: Vec<([u8; 32], String)>,
}

#[derive(Clone, Debug, Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionStatus {
    Blocked,
    Cleared,
}

#[derive(Clone, Debug, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Decision {
    pub sha256: super::Sha256,
    pub revision: u64,
    pub status: DecisionStatus,
    pub reason: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SecurityPayload {
    #[serde(rename = "type")]
    kind: String,
    schema_version: u8,
    revision: u64,
    issued_at: String,
    expires_at: String,
    complete: bool,
    decisions: Vec<Decision>,
}

pub struct VerifiedSecurity {
    pub(crate) revision: u64,
    pub(crate) canonical: String,
    pub(crate) expires: OffsetDateTime,
    pub(crate) decisions: Vec<Decision>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReleasePayload {
    #[serde(rename = "type")]
    kind: String,
    schema_version: u8,
    revision: u64,
    issued_at: String,
    expires_at: String,
    security_revision: u64,
    release: super::ReleaseResult,
}

pub struct VerifiedRelease {
    pub(crate) revision: u64,
    pub(crate) canonical: String,
    pub(crate) security_revision: u64,
    pub release: super::ReleaseResult,
    pub(crate) reported_block: Option<Decision>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DownloadIdentity {
    pub(crate) reference: super::ExactReference,
    pub(crate) bytes: u64,
    pub(crate) metadata_revision: u64,
    pub(crate) security_revision: u64,
    pub(crate) installation: Option<super::installation::Installation>,
    pub(crate) dependencies: Vec<super::Dependency>,
}

impl DownloadIdentity {
    pub fn reference(&self) -> &super::ExactReference {
        &self.reference
    }

    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    pub fn metadata_revision(&self) -> u64 {
        self.metadata_revision
    }

    pub fn security_revision(&self) -> u64 {
        self.security_revision
    }
}

impl VerifiedRelease {
    pub(crate) fn download_identity(&self) -> Option<DownloadIdentity> {
        let super::ReleaseResult::Release(release) = &self.release else {
            return None;
        };
        if !matches!(&release.availability, super::Availability::Available)
            || self.reported_block.is_some()
        {
            return None;
        }
        Some(DownloadIdentity {
            reference: super::ExactReference {
                mod_id: release.mod_id,
                release_id: release.release_id,
                sha256: release.artifact.sha256.clone(),
            },
            bytes: release.artifact.bytes,
            metadata_revision: release.metadata.revision,
            security_revision: self.security_revision,
            installation: release.metadata.installation.clone(),
            dependencies: release.metadata.dependencies.clone(),
        })
    }
}

#[derive(Default)]
pub struct SecurityState {
    pub snapshot: Option<VerifiedSecurity>,
    pub decisions: HashMap<super::Sha256, Decision>,
}

impl SecurityState {
    pub fn accept(&mut self, next: VerifiedSecurity) -> Result<(), &'static str> {
        if let Some(previous) = &self.snapshot {
            revision_after(
                previous.revision,
                &previous.canonical,
                next.revision,
                &next.canonical,
            )?;
        }
        let mut decisions = self.decisions.clone();
        for decision in &next.decisions {
            if let Some(previous) = decisions.get(&decision.sha256) {
                let old = serde_json::to_value(previous).map_err(|_| "Invalid saved decision")?;
                let new = serde_json::to_value(decision).map_err(|_| "Invalid new decision")?;
                revision_after(
                    previous.revision,
                    &canonical_text(&old)?,
                    decision.revision,
                    &canonical_text(&new)?,
                )?;
            }
            decisions.insert(decision.sha256.clone(), decision.clone());
        }
        self.decisions = decisions;
        self.snapshot = Some(next);
        Ok(())
    }

    pub fn blocked(&self, hash: &super::Sha256) -> bool {
        self.decisions
            .get(hash)
            .is_some_and(|decision| matches!(decision.status, DecisionStatus::Blocked))
    }
}

fn revision_after(
    old: u64,
    old_canonical: &str,
    new: u64,
    new_canonical: &str,
) -> Result<(), &'static str> {
    if new < old || (new == old && new_canonical != old_canonical) {
        return Err("Signed metadata rollback or equal-revision conflict");
    }
    Ok(())
}

fn fields(value: &Value, expected: &[&str]) -> Result<(), &'static str> {
    let object = value.as_object().ok_or("Invalid signed object")?;
    if object.len() != expected.len() || !expected.iter().all(|field| object.contains_key(*field)) {
        return Err("Unknown or missing signed field");
    }
    Ok(())
}

fn decode64(value: &str, size: usize) -> Result<Vec<u8>, &'static str> {
    let bytes = STANDARD.decode(value).map_err(|_| "Invalid base64")?;
    if bytes.len() != size || STANDARD.encode(&bytes) != value {
        return Err("Noncanonical base64");
    }
    Ok(bytes)
}

fn date(value: &str) -> Result<OffsetDateTime, &'static str> {
    let bytes = value.as_bytes();
    if bytes.len() != 24
        || !bytes.iter().enumerate().all(|(index, byte)| match index {
            4 | 7 => *byte == b'-',
            10 => *byte == b'T',
            13 | 16 => *byte == b':',
            19 => *byte == b'.',
            23 => *byte == b'Z',
            _ => byte.is_ascii_digit(),
        })
    {
        return Err("Invalid signed time");
    }
    OffsetDateTime::parse(value, &Rfc3339).map_err(|_| "Invalid signed time")
}

fn freshness(
    issued_at: &str,
    expires_at: &str,
    now: OffsetDateTime,
    limit: time::Duration,
) -> Result<OffsetDateTime, &'static str> {
    let issued = date(issued_at)?;
    let expires = date(expires_at)?;
    if issued > now + time::Duration::minutes(5)
        || expires <= now
        || expires <= issued
        || expires - issued > limit
    {
        return Err("Signed metadata is stale or has an invalid validity window");
    }
    Ok(expires)
}

fn canonical(value: &Value, output: &mut String) -> Result<(), &'static str> {
    match value {
        Value::Null => output.push_str("null"),
        Value::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
        Value::Number(number) => {
            let integer = number
                .as_i64()
                .filter(|value| value.unsigned_abs() <= MAX_SAFE_INTEGER)
                .or_else(|| {
                    number.as_f64().and_then(|value| {
                        (value.is_finite()
                            && value.fract() == 0.0
                            && value.abs() <= MAX_SAFE_INTEGER as f64)
                            .then_some(value as i64)
                    })
                })
                .ok_or("Unsafe signed number")?;
            write!(output, "{integer}").map_err(|_| "Canonical JSON failed")?;
        }
        Value::String(value) => string(value, output)?,
        Value::Array(items) => {
            output.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                canonical(item, output)?;
            }
            output.push(']');
        }
        Value::Object(object) => {
            output.push('{');
            let mut keys: Vec<_> = object.keys().collect();
            keys.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
            for (index, key) in keys.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                string(key, output)?;
                output.push(':');
                canonical(&object[*key], output)?;
            }
            output.push('}');
        }
    }
    Ok(())
}

fn string(value: &str, output: &mut String) -> Result<(), &'static str> {
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\u{0008}' => output.push_str("\\b"),
            '\t' => output.push_str("\\t"),
            '\n' => output.push_str("\\n"),
            '\u{000C}' => output.push_str("\\f"),
            '\r' => output.push_str("\\r"),
            character if character <= '\u{001F}' => {
                write!(output, "\\u{:04x}", character as u32)
                    .map_err(|_| "Canonical JSON failed")?;
            }
            character => output.push(character),
        }
    }
    output.push('"');
    Ok(())
}

fn canonical_text(value: &Value) -> Result<String, &'static str> {
    let mut output = String::new();
    canonical(value, &mut output)?;
    Ok(output)
}

fn signed_document(bytes: &[u8]) -> Result<Value, &'static str> {
    if bytes.len() > MAX_SIGNED_BYTES {
        return Err("Signed metadata is too large");
    }
    let envelope =
        crate::runtime_contract::unique_json(bytes).map_err(|_| "Invalid signed JSON")?;
    fields(&envelope, &["signed", "signatures"])?;
    Ok(envelope)
}

fn delegated_signature(
    envelope: &Value,
    keys: &VerifiedKeyset,
    now: OffsetDateTime,
) -> Result<String, &'static str> {
    if keys.expires <= now {
        return Err("Registry signing keys have expired");
    }
    let signatures = envelope["signatures"]
        .as_array()
        .ok_or("Invalid signature list")?;
    if !(1..=8).contains(&signatures.len()) {
        return Err("Invalid signature count");
    }
    let encoded = canonical_text(&envelope["signed"])?;
    let mut seen = HashSet::new();
    let mut trusted = false;
    for entry in signatures {
        let signature: Signature =
            serde_json::from_value(entry.clone()).map_err(|_| "Invalid registry signature")?;
        if signature.algorithm != "ed25519" || !seen.insert(signature.key_id.clone()) {
            return Err("Invalid or duplicate registry signature");
        }
        let bytes = decode64(&signature.signature, 64)?;
        if let Some((public_key, _)) = keys.keys.iter().find(|(_, id)| *id == signature.key_id) {
            trusted |= UnparsedPublicKey::new(&ED25519, public_key)
                .verify(encoded.as_bytes(), &bytes)
                .is_ok();
        }
    }
    if !trusted {
        return Err("No trusted registry signature");
    }
    Ok(encoded)
}

pub fn verify_security(
    bytes: &[u8],
    keys: &VerifiedKeyset,
    now: OffsetDateTime,
) -> Result<VerifiedSecurity, &'static str> {
    let envelope = signed_document(bytes)?;
    let signed = &envelope["signed"];
    fields(
        signed,
        &[
            "type",
            "schemaVersion",
            "revision",
            "issuedAt",
            "expiresAt",
            "complete",
            "decisions",
        ],
    )?;
    let payload: SecurityPayload =
        serde_json::from_value(signed.clone()).map_err(|_| "Invalid security snapshot")?;
    if payload.kind != "starframe-security"
        || payload.schema_version != 1
        || !payload.complete
        || !(1..=MAX_SAFE_INTEGER).contains(&payload.revision)
        || payload.decisions.len() > 10_000
    {
        return Err("Invalid security snapshot");
    }
    let expires = freshness(
        &payload.issued_at,
        &payload.expires_at,
        now,
        time::Duration::days(1),
    )?;
    let encoded = delegated_signature(&envelope, keys, now)?;
    let mut seen = HashSet::new();
    for decision in &payload.decisions {
        if !(1..=payload.revision).contains(&decision.revision)
            || !(1..=1000).contains(&decision.reason.chars().count())
            || !seen.insert(decision.sha256.clone())
        {
            return Err("Invalid security decision");
        }
    }
    Ok(VerifiedSecurity {
        revision: payload.revision,
        canonical: encoded,
        expires,
        decisions: payload.decisions,
    })
}

pub fn verify_release(
    bytes: &[u8],
    keys: &VerifiedKeyset,
    security: &VerifiedSecurity,
    expected_mod_id: super::ModId,
    expected_release_id: super::ReleaseId,
    now: OffsetDateTime,
) -> Result<VerifiedRelease, &'static str> {
    if security.expires <= now {
        return Err("Registry security snapshot has expired");
    }
    let envelope = signed_document(bytes)?;
    let signed = &envelope["signed"];
    fields(
        signed,
        &[
            "type",
            "schemaVersion",
            "revision",
            "issuedAt",
            "expiresAt",
            "securityRevision",
            "release",
        ],
    )?;
    let payload: ReleasePayload =
        serde_json::from_value(signed.clone()).map_err(|_| "Invalid release manifest")?;
    if payload.kind != "starframe-release"
        || ![1, 2].contains(&payload.schema_version)
        || !(1..=MAX_SAFE_INTEGER).contains(&payload.revision)
        || payload.security_revision != security.revision
        || !payload.release.valid()
    {
        return Err("Invalid or mismatched release manifest");
    }
    freshness(
        &payload.issued_at,
        &payload.expires_at,
        now,
        time::Duration::days(1),
    )?;
    let (mod_id, release_id, reported_block) = match &payload.release {
        super::ReleaseResult::Release(release) => {
            if (payload.schema_version == 2) != release.metadata.installation.is_some() {
                return Err("Release schema and installation declaration do not match");
            }
            if release.security.revision > security.revision {
                return Err("Invalid manifest security revision");
            }
            let block = if matches!(
                release.security.status,
                super::wire::SecurityStatus::Blocked
            ) {
                let reason = release
                    .security
                    .reason
                    .as_ref()
                    .ok_or("Missing block reason")?;
                if !(1..=security.revision).contains(&release.security.revision) {
                    return Err("Invalid manifest security revision");
                }
                Some(Decision {
                    sha256: release.artifact.sha256.clone(),
                    revision: release.security.revision,
                    status: DecisionStatus::Blocked,
                    reason: reason.clone(),
                })
            } else {
                None
            };
            (release.mod_id, release.release_id, block)
        }
        super::ReleaseResult::Tombstone(tombstone) => {
            if tombstone.security_revision > security.revision {
                return Err("Invalid tombstone security revision");
            }
            (tombstone.mod_id, tombstone.release_id, None)
        }
    };
    if mod_id != expected_mod_id || release_id != expected_release_id {
        return Err("Wrong exact release manifest");
    }
    let canonical = delegated_signature(&envelope, keys, now)?;
    Ok(VerifiedRelease {
        revision: payload.revision,
        canonical,
        security_revision: payload.security_revision,
        release: payload.release,
        reported_block,
    })
}

pub fn verify_keys(
    bytes: &[u8],
    root_public_key: &[u8; 32],
    now: OffsetDateTime,
) -> Result<VerifiedKeyset, &'static str> {
    let envelope = signed_document(bytes)?;
    let signed = &envelope["signed"];
    fields(
        signed,
        &[
            "type",
            "schemaVersion",
            "revision",
            "issuedAt",
            "expiresAt",
            "keys",
        ],
    )?;
    let keyset: Keyset = serde_json::from_value(signed.clone()).map_err(|_| "Invalid keyset")?;
    if keyset.kind != "starframe-registry-keys"
        || keyset.schema_version != 1
        || !(1..=MAX_SAFE_INTEGER).contains(&keyset.revision)
        || !(1..=8).contains(&keyset.keys.len())
    {
        return Err("Invalid keyset");
    }
    let expires = freshness(
        &keyset.issued_at,
        &keyset.expires_at,
        now,
        time::Duration::days(365),
    )?;
    let signatures = envelope["signatures"]
        .as_array()
        .ok_or("Invalid signature list")?;
    if signatures.len() != 1 {
        return Err("Invalid root signature count");
    }
    let signature: Signature =
        serde_json::from_value(signatures[0].clone()).map_err(|_| "Invalid root signature")?;
    if signature.algorithm != "ed25519" {
        return Err("Invalid signature algorithm");
    }
    let root_id = format!("{:x}", Sha256::digest(root_public_key));
    if signature.key_id != root_id {
        return Err("Untrusted root signature");
    }
    let encoded = canonical_text(signed)?;
    UnparsedPublicKey::new(&ED25519, root_public_key)
        .verify(encoded.as_bytes(), &decode64(&signature.signature, 64)?)
        .map_err(|_| "Invalid root signature")?;
    let mut ids = HashSet::new();
    let mut keys = Vec::new();
    for key in keyset.keys {
        let decoded = decode64(&key.public_key, 32)?;
        let public_key: [u8; 32] = decoded.try_into().map_err(|_| "Invalid signing key")?;
        let actual_id = format!("{:x}", Sha256::digest(public_key));
        if key.key_id != actual_id || !ids.insert(key.key_id.clone()) {
            return Err("Invalid or duplicate signing key ID");
        }
        keys.push((public_key, key.key_id));
    }
    Ok(VerifiedKeyset {
        revision: keyset.revision,
        canonical: encoded,
        expires,
        keys,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::signature::{Ed25519KeyPair, KeyPair};

    #[test]
    fn website_signed_keyset_fixture_verifies_in_rust() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/registry-keys-v1.json"
        ))
        .unwrap();
        assert_eq!(
            fixture["websiteContractRevision"],
            super::super::CONTRACT_REVISION
        );
        let root: [u8; 32] = decode64(fixture["rootPublicKey"].as_str().unwrap(), 32)
            .unwrap()
            .try_into()
            .unwrap();
        let now = OffsetDateTime::parse("2026-09-24T12:00:00Z", &Rfc3339).unwrap();
        let envelope = serde_json::to_vec(&fixture["envelope"]).unwrap();
        let accepted = verify_keys(&envelope, &root, now).unwrap();
        assert_eq!(accepted.canonical, fixture["canonical"].as_str().unwrap());
        assert_eq!(accepted.keys.len(), 1);
        let security_envelope = serde_json::to_vec(&fixture["securityEnvelope"]).unwrap();
        let security = verify_security(&security_envelope, &accepted, now).unwrap();
        assert_eq!(
            security.canonical,
            fixture["securityCanonical"].as_str().unwrap()
        );
        assert_eq!(security.decisions.len(), 1);
        let release_envelope = serde_json::to_vec(&fixture["releaseEnvelope"]).unwrap();
        let mod_id = super::super::ModId::try_from(1).unwrap();
        let release_id = super::super::ReleaseId(
            uuid::Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap(),
        );
        let release = verify_release(
            &release_envelope,
            &accepted,
            &security,
            mod_id,
            release_id,
            now,
        )
        .unwrap();
        assert_eq!(
            release.canonical,
            fixture["releaseCanonical"].as_str().unwrap()
        );
        assert!(release.reported_block.is_none());
        assert!(
            verify_release(
                &release_envelope,
                &accepted,
                &security,
                super::super::ModId::try_from(2).unwrap(),
                release_id,
                now,
            )
            .is_err()
        );
        let mut changed = fixture["releaseEnvelope"].clone();
        changed["signed"]["release"]["artifact"]["bytes"] = 1.into();
        assert!(
            verify_release(
                &serde_json::to_vec(&changed).unwrap(),
                &accepted,
                &security,
                mod_id,
                release_id,
                now,
            )
            .is_err()
        );
        let blocked_id = super::super::ReleaseId(
            uuid::Uuid::parse_str("22222222-2222-4222-8222-222222222222").unwrap(),
        );
        let blocked = verify_release(
            &serde_json::to_vec(&fixture["blockedReleaseEnvelope"]).unwrap(),
            &accepted,
            &security,
            mod_id,
            blocked_id,
            now,
        )
        .unwrap();
        assert_eq!(
            blocked.canonical,
            fixture["blockedReleaseCanonical"].as_str().unwrap()
        );
        assert_eq!(
            blocked.reported_block.unwrap().sha256.as_str(),
            "c".repeat(64)
        );
    }

    #[test]
    fn website_schema_two_signs_each_installation_and_rejects_missing_or_changed_plans() {
        let fixtures: Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/registry-installation-v1.json"
        ))
        .unwrap();
        let old: Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/registry-keys-v1.json"
        ))
        .unwrap();
        let root: [u8; 32] = decode64(old["rootPublicKey"].as_str().unwrap(), 32)
            .unwrap()
            .try_into()
            .unwrap();
        let now = OffsetDateTime::parse("2026-09-24T12:00:00Z", &Rfc3339).unwrap();
        let keys = verify_keys(&serde_json::to_vec(&old["envelope"]).unwrap(), &root, now).unwrap();
        let security = verify_security(
            &serde_json::to_vec(&old["securityEnvelope"]).unwrap(),
            &keys,
            now,
        )
        .unwrap();
        let online = Ed25519KeyPair::from_seed_unchecked(&[9u8; 32]).unwrap();
        let mod_id = super::super::ModId::try_from(1).unwrap();
        let release_id = super::super::ReleaseId(
            uuid::Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap(),
        );
        let check = |envelope: &Value| {
            verify_release(
                &serde_json::to_vec(envelope).unwrap(),
                &keys,
                &security,
                mod_id,
                release_id,
                now,
            )
        };
        let sign = |envelope: &mut Value| {
            envelope["signatures"][0]["signature"] = STANDARD
                .encode(
                    online
                        .sign(canonical_text(&envelope["signed"]).unwrap().as_bytes())
                        .as_ref(),
                )
                .into();
        };
        for example in fixtures["releases"].as_array().unwrap() {
            let envelope = &example["envelope"];
            let verified = check(envelope).unwrap();
            assert_eq!(verified.canonical, example["canonical"].as_str().unwrap());
            let super::super::ReleaseResult::Release(release) = verified.release else {
                panic!("full fixture")
            };
            assert!(release.metadata.installation.unwrap().valid());

            for dependencies in [
                serde_json::json!([{"kind":"exact","modId":1,"releaseId":"22222222-2222-4222-8222-222222222222"}]),
                serde_json::json!([{"kind":"range","modId":2,"minimum":null,"before":null,"includePrerelease":false}]),
                serde_json::json!([{"kind":"range","modId":2,"minimum":"2.0.0","before":"1.0.0","includePrerelease":false}]),
                serde_json::json!([{"kind":"range","modId":2,"minimum":"1.0.0","includePrerelease":false}]),
                serde_json::json!([
                    {"kind":"exact","modId":2,"releaseId":"22222222-2222-4222-8222-222222222222"},
                    {"kind":"range","modId":2,"minimum":"1.0.0","before":null,"includePrerelease":false}
                ]),
            ] {
                let mut changed = envelope.clone();
                changed["signed"]["release"]["metadata"]["dependencies"] = dependencies;
                sign(&mut changed);
                assert!(check(&changed).is_err());
            }

            let mut changed = envelope.clone();
            changed["signed"]["release"]["metadata"]["installation"]["schemaVersion"] = 2.into();
            sign(&mut changed);
            assert!(check(&changed).is_err());
            let mut changed = envelope.clone();
            changed["signed"]["release"]["metadata"]["installation"] = Value::Null;
            sign(&mut changed);
            assert!(check(&changed).is_err());
            changed["signed"]["release"]["metadata"]
                .as_object_mut()
                .unwrap()
                .remove("installation");
            sign(&mut changed);
            assert!(check(&changed).is_err());
            for version in [1, 3] {
                let mut changed = envelope.clone();
                changed["signed"]["schemaVersion"] = version.into();
                sign(&mut changed);
                assert!(check(&changed).is_err());
            }
            let mut changed = envelope.clone();
            changed["signed"]["release"]["metadata"]["installation"] =
                fixtures["cases"]["valid"][0]["plan"].clone();
            if changed != *envelope {
                assert!(
                    check(&changed).is_err(),
                    "unsigned replacement plan accepted"
                );
            }
        }
    }

    #[test]
    fn canonical_json_matches_website_integer_and_unicode_contract() {
        let value =
            serde_json::json!({"z": [1.0, -0.0, 9007199254740991u64], "á": "€$\u{000f}\nA'B\"\\/"});
        assert_eq!(
            canonical_text(&value).unwrap(),
            "{\"z\":[1,0,9007199254740991],\"á\":\"€$\\u000f\\nA'B\\\"\\\\/\"}"
        );
        assert!(canonical_text(&serde_json::json!(9007199254740992u64)).is_err());
        let utf16 = serde_json::json!({"\u{E000}":1,"\u{10000}":2});
        assert_eq!(canonical_text(&utf16).unwrap(), "{\"𐀀\":2,\"\":1}");
    }

    #[test]
    fn root_signed_keyset_verifies_and_rejects_tamper_and_wrong_root() {
        let root = Ed25519KeyPair::from_seed_unchecked(&[7u8; 32]).unwrap();
        let online = Ed25519KeyPair::from_seed_unchecked(&[9u8; 32]).unwrap();
        let issued = "2026-09-24T00:00:00.000Z";
        let expires = "2026-10-24T00:00:00.000Z";
        let signed = serde_json::json!({
            "type":"starframe-registry-keys","schemaVersion":1,"revision":1,
            "issuedAt":issued,"expiresAt":expires,
            "keys":[{"keyId":format!("{:x}", Sha256::digest(online.public_key().as_ref())),
                "publicKey":STANDARD.encode(online.public_key().as_ref())}]
        });
        let message = canonical_text(&signed).unwrap();
        let envelope = serde_json::json!({"signed":signed,"signatures":[{
            "keyId":format!("{:x}", Sha256::digest(root.public_key().as_ref())),
            "algorithm":"ed25519","signature":STANDARD.encode(root.sign(message.as_bytes()).as_ref())
        }]});
        let bytes = serde_json::to_vec(&envelope).unwrap();
        let now = OffsetDateTime::parse("2026-09-24T12:00:00Z", &Rfc3339).unwrap();
        let root_public: [u8; 32] = root.public_key().as_ref().try_into().unwrap();
        let accepted = verify_keys(&bytes, &root_public, now).unwrap();
        assert_eq!(accepted.revision, 1);
        assert_eq!(accepted.keys.len(), 1);
        let wrong_root: [u8; 32] = online.public_key().as_ref().try_into().unwrap();
        assert!(verify_keys(&bytes, &wrong_root, now).is_err());
        let mut changed = envelope;
        changed["signed"]["revision"] = 2.into();
        assert!(verify_keys(&serde_json::to_vec(&changed).unwrap(), &root_public, now).is_err());
        assert!(
            verify_keys(
                br#"{"signed":{},"signed":{},"signatures":[]}"#,
                &root_public,
                now
            )
            .is_err()
        );
        assert!(verify_keys(&bytes, &root_public, now + time::Duration::days(31)).is_err());
    }

    #[test]
    fn signed_security_retains_blocks_until_newer_explicit_clear() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/registry-keys-v1.json"
        ))
        .unwrap();
        let root: [u8; 32] = decode64(fixture["rootPublicKey"].as_str().unwrap(), 32)
            .unwrap()
            .try_into()
            .unwrap();
        let now = OffsetDateTime::parse("2026-09-24T12:00:00Z", &Rfc3339).unwrap();
        let keys = verify_keys(
            &serde_json::to_vec(&fixture["envelope"]).unwrap(),
            &root,
            now,
        )
        .unwrap();
        let online = Ed25519KeyPair::from_seed_unchecked(&[9u8; 32]).unwrap();
        let hash = "a".repeat(64);
        let signed = |revision: u64, decisions: Vec<Value>| {
            let payload = serde_json::json!({
                "type":"starframe-security","schemaVersion":1,"revision":revision,
                "issuedAt":"2026-09-24T00:00:00.000Z","expiresAt":"2026-09-25T00:00:00.000Z",
                "complete":true,"decisions":decisions
            });
            let signature = STANDARD.encode(
                online
                    .sign(canonical_text(&payload).unwrap().as_bytes())
                    .as_ref(),
            );
            serde_json::to_vec(&serde_json::json!({"signed":payload,"signatures":[{
                "keyId":keys.keys[0].1,"algorithm":"ed25519","signature":signature
            }]}))
            .unwrap()
        };
        let mut state = SecurityState::default();
        let first = signed(
            1,
            vec![
                serde_json::json!({"sha256":hash,"revision":1,"status":"blocked","reason":"Fixture block"}),
            ],
        );
        state
            .accept(verify_security(&first, &keys, now).unwrap())
            .unwrap();
        let hash = super::super::Sha256::try_from(hash).unwrap();
        assert!(state.blocked(&hash));
        let omitted = signed(2, vec![]);
        state
            .accept(verify_security(&omitted, &keys, now).unwrap())
            .unwrap();
        assert!(state.blocked(&hash));
        assert!(
            state
                .accept(verify_security(&first, &keys, now).unwrap())
                .is_err()
        );
        let changed = signed(
            2,
            vec![
                serde_json::json!({"sha256":"b".repeat(64),"revision":2,"status":"blocked","reason":"Different snapshot"}),
            ],
        );
        assert!(
            state
                .accept(verify_security(&changed, &keys, now).unwrap())
                .is_err()
        );
        let clear = signed(
            3,
            vec![
                serde_json::json!({"sha256":hash.as_str(),"revision":3,"status":"cleared","reason":"Reviewed and cleared"}),
            ],
        );
        state
            .accept(verify_security(&clear, &keys, now).unwrap())
            .unwrap();
        assert!(!state.blocked(&hash));
        let mut tampered: Value = serde_json::from_slice(&clear).unwrap();
        tampered["signed"]["revision"] = 4.into();
        assert!(verify_security(&serde_json::to_vec(&tampered).unwrap(), &keys, now).is_err());
    }

    #[test]
    fn root_signed_rotation_accepts_overlap_then_retires_the_old_online_key() {
        let root = Ed25519KeyPair::from_seed_unchecked(&[7u8; 32]).unwrap();
        let old = Ed25519KeyPair::from_seed_unchecked(&[9u8; 32]).unwrap();
        let replacement = Ed25519KeyPair::from_seed_unchecked(&[10u8; 32]).unwrap();
        let root_public: [u8; 32] = root.public_key().as_ref().try_into().unwrap();
        let now = OffsetDateTime::parse("2026-09-24T12:00:00Z", &Rfc3339).unwrap();
        let keyset = |revision: u64, signers: &[&Ed25519KeyPair]| {
            let keys: Vec<_> = signers
                .iter()
                .map(|signer| {
                    serde_json::json!({
                        "keyId":format!("{:x}", Sha256::digest(signer.public_key().as_ref())),
                        "publicKey":STANDARD.encode(signer.public_key().as_ref())
                    })
                })
                .collect();
            let payload = serde_json::json!({
                "type":"starframe-registry-keys","schemaVersion":1,"revision":revision,
                "issuedAt":"2026-09-24T00:00:00.000Z","expiresAt":"2026-10-24T00:00:00.000Z",
                "keys":keys
            });
            let signature = root.sign(canonical_text(&payload).unwrap().as_bytes());
            serde_json::to_vec(&serde_json::json!({"signed":payload,"signatures":[{
                "keyId":format!("{:x}", Sha256::digest(root.public_key().as_ref())),
                "algorithm":"ed25519","signature":STANDARD.encode(signature.as_ref())
            }]}))
            .unwrap()
        };
        let first = verify_keys(&keyset(1, &[&old]), &root_public, now).unwrap();
        let overlap = verify_keys(&keyset(2, &[&old, &replacement]), &root_public, now).unwrap();
        let retired = verify_keys(&keyset(3, &[&replacement]), &root_public, now).unwrap();
        let payload = serde_json::json!({
            "type":"starframe-security","schemaVersion":1,"revision":1,
            "issuedAt":"2026-09-24T00:00:00.000Z","expiresAt":"2026-09-25T00:00:00.000Z",
            "complete":true,"decisions":[]
        });
        let signature = old.sign(canonical_text(&payload).unwrap().as_bytes());
        let signed = serde_json::to_vec(&serde_json::json!({"signed":payload,"signatures":[{
            "keyId":format!("{:x}", Sha256::digest(old.public_key().as_ref())),
            "algorithm":"ed25519","signature":STANDARD.encode(signature.as_ref())
        }]}))
        .unwrap();
        assert!(verify_security(&signed, &first, now).is_ok());
        assert!(verify_security(&signed, &overlap, now).is_ok());
        assert!(verify_security(&signed, &retired, now).is_err());
    }
}
