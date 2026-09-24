use base64::{Engine as _, engine::general_purpose::STANDARD};
use ring::signature::{ED25519, UnparsedPublicKey};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::HashSet, fmt::Write};
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
    pub revision: u64,
    pub canonical: String,
    pub expires: OffsetDateTime,
    pub keys: Vec<([u8; 32], String)>,
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

pub fn verify_keys(
    bytes: &[u8],
    root_public_key: &[u8; 32],
    now: OffsetDateTime,
) -> Result<VerifiedKeyset, &'static str> {
    if bytes.len() > MAX_SIGNED_BYTES {
        return Err("Signed metadata is too large");
    }
    let envelope =
        crate::runtime_contract::unique_json(bytes).map_err(|_| "Invalid signed JSON")?;
    fields(&envelope, &["signed", "signatures"])?;
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
}
