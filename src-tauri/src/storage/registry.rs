use super::{Error, Result, Storage};
use crate::registry::{
    ExactReference, ModId, ReceiptClaim, ReleaseId, Sha256, VerifiedArchive,
    trust::{self, Decision, DownloadIdentity, VerifiedKeyset, VerifiedRelease, VerifiedSecurity},
};
use rusqlite::{OptionalExtension, Transaction, params};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RegistryDisplay {
    pub name: String,
    pub author: String,
}

impl RegistryDisplay {
    pub fn valid(&self) -> bool {
        (1..=120).contains(&self.name.len())
            && !self.name.trim().is_empty()
            && self.author.len() <= 120
            && !self
                .name
                .chars()
                .chain(self.author.chars())
                .any(char::is_control)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RegistryLibraryEntry {
    pub reference: ExactReference,
    pub display: RegistryDisplay,
    pub version_label: String,
    pub tested_game_build: String,
    pub metadata_revision: u64,
    pub installation: crate::registry::installation::Installation,
    pub dependencies: Vec<crate::registry::Dependency>,
}

impl RegistryLibraryEntry {
    fn valid(&self) -> bool {
        self.display.valid()
            && (1..=100).contains(&self.version_label.len())
            && (1..=100).contains(&self.tested_game_build.len())
            && (1..=crate::registry::MAX_SAFE_INTEGER).contains(&self.metadata_revision)
            && self.installation.valid()
            && self.dependencies.len() <= 100
    }
}

impl Storage {
    pub fn accept_registry_keys(
        &mut self,
        envelope: &[u8],
        root: &[u8; 32],
        now: OffsetDateTime,
    ) -> Result<VerifiedKeyset> {
        let verified = trust::verify_keys(envelope, root, now)
            .map_err(|error| Error::Invalid(error.into()))?;
        let tx = self.conn.transaction()?;
        save_stream(
            &tx,
            "keys",
            verified.revision,
            &verified.canonical,
            envelope,
        )?;
        tx.commit()?;
        Ok(verified)
    }

    pub fn accept_registry_security(
        &mut self,
        envelope: &[u8],
        root: &[u8; 32],
        now: OffsetDateTime,
    ) -> Result<VerifiedSecurity> {
        let key_envelope = self
            .registry_document("keys")?
            .ok_or_else(|| Error::Invalid("No verified registry signing keys are saved.".into()))?;
        let keys = trust::verify_keys(&key_envelope, root, now)
            .map_err(|error| Error::Invalid(error.into()))?;
        let verified = trust::verify_security(envelope, &keys, now)
            .map_err(|error| Error::Invalid(error.into()))?;
        let tx = self.conn.transaction()?;
        check_stream(&tx, "security", verified.revision, &verified.canonical)?;
        for decision in &verified.decisions {
            save_decision(&tx, decision, true)?;
        }
        save_stream(
            &tx,
            "security",
            verified.revision,
            &verified.canonical,
            envelope,
        )?;
        tx.commit()?;
        Ok(verified)
    }

    pub fn accept_registry_release(
        &mut self,
        envelope: &[u8],
        root: &[u8; 32],
        expected_mod_id: ModId,
        expected_release_id: ReleaseId,
        now: OffsetDateTime,
    ) -> Result<VerifiedRelease> {
        let key_envelope = self
            .registry_document("keys")?
            .ok_or_else(|| Error::Invalid("No verified registry signing keys are saved.".into()))?;
        let keys = trust::verify_keys(&key_envelope, root, now)
            .map_err(|error| Error::Invalid(error.into()))?;
        let security_envelope = self
            .registry_document("security")?
            .ok_or_else(|| Error::Invalid("No verified registry security is saved.".into()))?;
        let security = trust::verify_security(&security_envelope, &keys, now)
            .map_err(|error| Error::Invalid(error.into()))?;
        let verified = trust::verify_release(
            envelope,
            &keys,
            &security,
            expected_mod_id,
            expected_release_id,
            now,
        )
        .map_err(|error| Error::Invalid(error.into()))?;
        let stream = format!("release:{}", expected_release_id.0);
        let tx = self.conn.transaction()?;
        check_stream(&tx, &stream, verified.revision, &verified.canonical)?;
        if let Some(block) = &verified.reported_block {
            save_decision(&tx, block, false)?;
        }
        save_stream(
            &tx,
            &stream,
            verified.revision,
            &verified.canonical,
            envelope,
        )?;
        tx.commit()?;
        Ok(verified)
    }

    pub fn ready_registry_download(
        &self,
        root: &[u8; 32],
        expected_mod_id: ModId,
        expected_release_id: ReleaseId,
        now: OffsetDateTime,
    ) -> Result<DownloadIdentity> {
        self.ready_registry_release(root, expected_mod_id, expected_release_id, now)?
            .download_identity()
            .ok_or_else(|| Error::Invalid("Registry release is unavailable or blocked.".into()))
    }

    pub fn ready_registry_release(
        &self,
        root: &[u8; 32],
        expected_mod_id: ModId,
        expected_release_id: ReleaseId,
        now: OffsetDateTime,
    ) -> Result<VerifiedRelease> {
        let keys = self
            .registry_document("keys")?
            .ok_or_else(|| Error::Invalid("No verified registry signing keys are saved.".into()))?;
        let keys =
            trust::verify_keys(&keys, root, now).map_err(|error| Error::Invalid(error.into()))?;
        let security = self
            .registry_document("security")?
            .ok_or_else(|| Error::Invalid("No verified registry security is saved.".into()))?;
        let security = trust::verify_security(&security, &keys, now)
            .map_err(|error| Error::Invalid(error.into()))?;
        let stream = format!("release:{}", expected_release_id.0);
        let manifest = self
            .registry_document(&stream)?
            .ok_or_else(|| Error::Invalid("No verified registry release is saved.".into()))?;
        let release = trust::verify_release(
            &manifest,
            &keys,
            &security,
            expected_mod_id,
            expected_release_id,
            now,
        )
        .map_err(|error| Error::Invalid(error.into()))?;
        let identity = release
            .download_identity()
            .ok_or_else(|| Error::Invalid("Registry release is unavailable or blocked.".into()))?;
        if self.registry_hash_blocked(&identity.reference.sha256)? {
            return Err(Error::Invalid(
                "Registry archive is blocked by a retained security decision.".into(),
            ));
        }
        Ok(release)
    }

    pub fn confirm_registry_download(
        &self,
        root: &[u8; 32],
        original: &DownloadIdentity,
        now: OffsetDateTime,
    ) -> Result<()> {
        let current = self.ready_registry_download(
            root,
            original.reference.mod_id,
            original.reference.release_id,
            now,
        )?;
        if current != *original {
            return Err(Error::Invalid(
                "Registry approval changed during download. Keep the verified bytes; refresh the exact release before installing.".into(),
            ));
        }
        Ok(())
    }

    pub fn registry_document(&self, name: &str) -> Result<Option<Vec<u8>>> {
        if name != "keys" && name != "security" && !name.starts_with("release:") {
            return Err(Error::Invalid("Unknown registry trust stream.".into()));
        }
        self.conn
            .query_row(
                "SELECT envelope FROM registry_trust_streams WHERE name=?",
                [name],
                |row| row.get(0),
            )
            .optional()
            .map_err(Error::from)
    }

    pub fn registry_decisions(&self) -> Result<Vec<Decision>> {
        self.conn
            .prepare("SELECT sha256,record FROM registry_decisions ORDER BY sha256")?
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .map(|row| {
                let (hash, record) = row?;
                let decision: Decision = serde_json::from_str(&record)
                    .map_err(|_| Error::Invalid("Saved registry decision is invalid.".into()))?;
                if decision.sha256.as_str() != hash {
                    return Err(Error::Invalid(
                        "Saved registry decision hash changed.".into(),
                    ));
                }
                Ok(decision)
            })
            .collect()
    }

    pub fn registry_hash_blocked(&self, hash: &Sha256) -> Result<bool> {
        let record: Option<String> = self
            .conn
            .query_row(
                "SELECT record FROM registry_decisions WHERE sha256=?",
                [hash.as_str()],
                |row| row.get(0),
            )
            .optional()?;
        let Some(record) = record else {
            return Ok(false);
        };
        let decision: Decision = serde_json::from_str(&record)
            .map_err(|_| Error::Invalid("Saved registry decision is invalid.".into()))?;
        if decision.sha256 != *hash {
            return Err(Error::Invalid(
                "Saved registry decision hash changed.".into(),
            ));
        }
        Ok(matches!(decision.status, trust::DecisionStatus::Blocked))
    }

    pub fn require_registry_unblocked_hash(&self, hash: &str) -> Result<()> {
        let hash =
            Sha256::try_from(hash.to_owned()).map_err(|error| Error::Invalid(error.into()))?;
        if self.registry_hash_blocked(&hash)? {
            return Err(Error::Invalid(
                "This archive is blocked by a signed registry security decision.".into(),
            ));
        }
        Ok(())
    }

    pub fn record_registry_receipt(&mut self, verified: &VerifiedArchive) -> Result<ReceiptClaim> {
        let claim = verified.receipt_claim();
        if !claim.valid() {
            return Err(Error::Invalid("Invalid verified registry receipt.".into()));
        }
        let record = serde_json::to_string(&claim)
            .map_err(|_| Error::Invalid("Invalid verified registry receipt.".into()))?;
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT OR IGNORE INTO registry_receipt_attempts(download_id,account_id,record) VALUES (?,?,?)",
            params![claim.download_id.to_string(), claim.account_id.to_string(), record],
        )?;
        let saved: (String, String) = tx.query_row(
            "SELECT account_id,record FROM registry_receipt_attempts WHERE download_id=?",
            [claim.download_id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        if saved != (claim.account_id.to_string(), record) {
            return Err(Error::Invalid("Registry receipt identity changed.".into()));
        }
        tx.commit()?;
        Ok(claim)
    }

    pub fn pending_registry_receipts(&self, account_id: Uuid) -> Result<Vec<ReceiptClaim>> {
        self.conn
            .prepare("SELECT download_id,record FROM registry_receipt_attempts WHERE account_id=? ORDER BY rowid")?
            .query_map([account_id.to_string()], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .map(|row| {
                let (id, record) = row?;
                let claim: ReceiptClaim = serde_json::from_str(&record)
                    .map_err(|_| Error::Invalid("Saved registry receipt is invalid.".into()))?;
                if claim.download_id.to_string() != id
                    || claim.account_id != account_id
                    || !claim.valid()
                {
                    return Err(Error::Invalid("Saved registry receipt identity changed.".into()));
                }
                Ok(claim)
            })
            .collect()
    }

    pub fn clear_registry_receipt(&mut self, claim: &ReceiptClaim) -> Result<()> {
        let record = serde_json::to_string(claim)
            .map_err(|_| Error::Invalid("Invalid verified registry receipt.".into()))?;
        self.conn.execute(
            "DELETE FROM registry_receipt_attempts WHERE download_id=? AND account_id=? AND record=?",
            params![claim.download_id.to_string(), claim.account_id.to_string(), record],
        )?;
        Ok(())
    }

    pub fn save_registry_reference(&mut self, reference: &ExactReference) -> Result<()> {
        let mod_id = u64::from(reference.mod_id) as i64;
        let release_id = reference.release_id.0.to_string();
        let hash = reference.sha256.as_str();
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT OR IGNORE INTO registry_library (mod_id, release_id, sha256) VALUES (?, ?, ?)",
            params![mod_id, release_id, hash],
        )?;
        let saved: String = tx.query_row(
            "SELECT sha256 FROM registry_library WHERE mod_id=? AND release_id=?",
            params![mod_id, release_id],
            |row| row.get(0),
        )?;
        if saved != hash {
            return Err(Error::Invalid(
                "A saved registry release has a different SHA-256. Keep the existing exact reference."
                    .into(),
            ));
        }
        tx.commit()?;
        Ok(())
    }

    pub fn registry_references(&self) -> Result<Vec<ExactReference>> {
        self.conn
            .prepare("SELECT mod_id, release_id, sha256 FROM registry_library ORDER BY mod_id, release_id")?
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
            })?
            .map(|row| {
                let (mod_id, release_id, hash) = row?;
                Ok(ExactReference {
                    mod_id: ModId::try_from(mod_id as u64)
                        .map_err(|error| Error::Invalid(error.into()))?,
                    release_id: ReleaseId(Uuid::parse_str(&release_id).map_err(|_| {
                        Error::Invalid("Saved registry ReleaseID is invalid.".into())
                    })?),
                    sha256: Sha256::try_from(hash)
                        .map_err(|error| Error::Invalid(error.into()))?,
                })
            })
            .collect()
    }

    pub fn installed_registry_releases(&self) -> Result<Vec<RegistryLibraryEntry>> {
        self.conn.prepare("SELECT mod_id, release_id, sha256, installation_record FROM registry_library WHERE installation_record IS NOT NULL ORDER BY mod_id,release_id")?
            .query_map([], |row| Ok((row.get::<_,i64>(0)?, row.get::<_,String>(1)?, row.get::<_,String>(2)?, row.get::<_,String>(3)?)))?
            .map(|row| {
                let (mod_id, release_id, hash, record) = row?;
                let entry: RegistryLibraryEntry = serde_json::from_str(&record).map_err(|error| Error::Invalid(format!("Invalid installed registry release: {error}")))?;
                if !entry.valid() || u64::from(entry.reference.mod_id) != mod_id as u64
                    || entry.reference.release_id.0.to_string() != release_id || entry.reference.sha256.as_str() != hash {
                    return Err(Error::Invalid("Installed registry release identity changed.".into()));
                }
                Ok(entry)
            }).collect()
    }

    pub fn complete_registry_install(
        &mut self,
        root: &[u8; 32],
        original: &DownloadIdentity,
        display: &RegistryDisplay,
        prepared: &crate::packages::Prepared,
        operation: &crate::packages::Operation,
        now: OffsetDateTime,
    ) -> Result<RegistryLibraryEntry> {
        self.confirm_registry_download(root, original, now)?;
        let verified = self.ready_registry_release(
            root,
            original.reference.mod_id,
            original.reference.release_id,
            now,
        )?;
        let crate::registry::ReleaseResult::Release(release) = verified.release else {
            return Err(Error::Invalid("Registry release is unavailable.".into()));
        };
        let installation = release.metadata.installation.ok_or_else(|| {
            Error::Invalid("The release has no approved installation declaration.".into())
        })?;
        installation
            .validate_files(&prepared.files)
            .map_err(Error::Invalid)?;
        if !display.valid()
            || prepared.hash != original.reference.sha256.as_str()
            || operation.hash != prepared.hash
            || operation.release_id != original.reference.release_id.0.to_string()
            || operation.status != crate::packages::Status::Completed
            || operation.kind != crate::packages::Kind::RegistryInstall
            || operation.total_bytes != original.bytes
        {
            return Err(Error::Invalid(
                "The registry installation does not match its approved release and operation."
                    .into(),
            ));
        }
        if self
            .pending_removals()?
            .iter()
            .any(|(hash, _)| hash == &prepared.hash)
        {
            return Err(Error::Invalid(
                "Finish pending artifact cleanup before installing this release.".into(),
            ));
        }
        let entry = RegistryLibraryEntry {
            reference: original.reference.clone(),
            display: display.clone(),
            version_label: release.version_label,
            tested_game_build: release.metadata.tested_game_build,
            metadata_revision: release.metadata.revision,
            installation,
            dependencies: release.metadata.dependencies,
        };
        let entry_record =
            serde_json::to_string(&entry).map_err(|error| Error::Invalid(error.to_string()))?;
        if !entry.valid() || entry_record.len() > 131072 {
            return Err(Error::Invalid("Invalid registry library metadata.".into()));
        }
        let prepared_record = super::packages::prepared_record(prepared)?;
        let operation_record = super::packages::operation_record(operation)?;
        let tx = self
            .conn
            .transaction_with_behavior(super::TransactionBehavior::Immediate)?;
        let previous: Option<String> = tx
            .query_row(
                "SELECT record FROM prepared_artifacts WHERE hash=?",
                [&prepared.hash],
                |row| row.get(0),
            )
            .optional()?;
        if previous.is_some_and(|record| record != prepared_record) {
            return Err(Error::Invalid(
                "Prepared content identity changed. Retain it for repair.".into(),
            ));
        }
        let previous: Option<(String,Option<String>)> = tx.query_row("SELECT sha256,installation_record FROM registry_library WHERE mod_id=? AND release_id=?", params![u64::from(entry.reference.mod_id) as i64, entry.reference.release_id.0.to_string()], |row| Ok((row.get(0)?,row.get(1)?))).optional()?;
        if let Some((hash, record)) = previous {
            let mut previous = record
                .map(|record| {
                    serde_json::from_str::<RegistryLibraryEntry>(&record).map_err(|error| {
                        Error::Invalid(format!("Invalid saved registry installation: {error}"))
                    })
                })
                .transpose()?;
            if let Some(previous) = &mut previous {
                previous.display = entry.display.clone();
            }
            if hash != prepared.hash || previous.is_some_and(|previous| previous != entry) {
                return Err(Error::Invalid(
                    "The installed registry approval changed. Keep the existing exact release."
                        .into(),
                ));
            }
        }
        tx.execute(
            "INSERT INTO prepared_artifacts(hash,record) VALUES (?,?) ON CONFLICT(hash) DO NOTHING",
            params![prepared.hash, prepared_record],
        )?;
        tx.execute("INSERT INTO registry_library(mod_id,release_id,sha256,installation_record) VALUES (?,?,?,?) ON CONFLICT(mod_id,release_id) DO UPDATE SET installation_record=excluded.installation_record",params![u64::from(entry.reference.mod_id) as i64,entry.reference.release_id.0.to_string(),prepared.hash,entry_record])?;
        if tx.execute(
            "UPDATE package_operations SET record=? WHERE id=? AND request_id=?",
            params![operation_record, operation.id, operation.request_id],
        )? != 1
        {
            return Err(Error::Invalid(
                "Registry installation operation is missing.".into(),
            ));
        }
        super::bump(&tx)?;
        tx.commit()?;
        Ok(entry)
    }

    pub fn has_registry_reference(&self, reference: &ExactReference) -> Result<bool> {
        let saved: Option<String> = self
            .conn
            .query_row(
                "SELECT sha256 FROM registry_library WHERE mod_id=? AND release_id=?",
                params![
                    u64::from(reference.mod_id) as i64,
                    reference.release_id.0.to_string()
                ],
                |row| row.get(0),
            )
            .optional()?;
        Ok(saved.is_some_and(|hash| hash == reference.sha256.as_str()))
    }
}

fn check_stream(tx: &Transaction<'_>, name: &str, revision: u64, canonical: &str) -> Result<()> {
    let old: Option<(i64, String)> = tx
        .query_row(
            "SELECT revision,canonical FROM registry_trust_streams WHERE name=?",
            [name],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    if let Some((previous, payload)) = old
        && (revision < previous as u64 || (revision == previous as u64 && canonical != payload))
    {
        return Err(Error::Invalid(
            "Registry metadata rollback or equal-revision conflict.".into(),
        ));
    }
    Ok(())
}

fn save_decision(tx: &Transaction<'_>, decision: &Decision, strict: bool) -> Result<()> {
    let record = serde_json::to_string(decision)
        .map_err(|_| Error::Invalid("Invalid registry decision.".into()))?;
    let old: Option<(i64, String)> = tx
        .query_row(
            "SELECT revision, record FROM registry_decisions WHERE sha256=?",
            [decision.sha256.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    if let Some((revision, previous)) = old {
        if decision.revision < revision as u64 && !strict {
            return Ok(());
        }
        if decision.revision < revision as u64
            || (decision.revision == revision as u64 && record != previous)
        {
            return Err(Error::Invalid(
                "Registry security decision rollback or conflict.".into(),
            ));
        }
    }
    tx.execute(
        "INSERT INTO registry_decisions (sha256,revision,record) VALUES (?,?,?) ON CONFLICT(sha256) DO UPDATE SET revision=excluded.revision,record=excluded.record",
        params![decision.sha256.as_str(), decision.revision as i64, record],
    )?;
    Ok(())
}

fn save_stream(
    tx: &Transaction<'_>,
    name: &str,
    revision: u64,
    canonical: &str,
    envelope: &[u8],
) -> Result<()> {
    check_stream(tx, name, revision, canonical)?;
    tx.execute(
        "INSERT INTO registry_trust_streams (name,revision,canonical,envelope) VALUES (?,?,?,?) ON CONFLICT(name) DO UPDATE SET revision=excluded.revision,canonical=excluded.canonical,envelope=excluded.envelope",
        params![name, revision as i64, canonical, envelope],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use ring::signature::Ed25519KeyPair;
    use time::format_description::well_known::Rfc3339;

    #[test]
    fn registry_install_commit_rolls_back_retries_deduplicates_and_binds_the_plan() {
        use crate::packages::{Kind, Operation, Prepared, PreparedFile, Status};
        use serde_json::Value;
        let temp = tempfile::tempdir().unwrap();
        let mut store = Storage::open(temp.path()).unwrap();
        let keys: Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/registry-keys-v1.json"
        ))
        .unwrap();
        let plans: Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/registry-installation-v1.json"
        ))
        .unwrap();
        let root: [u8; 32] = STANDARD
            .decode(keys["rootPublicKey"].as_str().unwrap())
            .unwrap()
            .try_into()
            .unwrap();
        let now = OffsetDateTime::parse("2026-09-24T12:00:00Z", &Rfc3339).unwrap();
        store
            .accept_registry_keys(&serde_json::to_vec(&keys["envelope"]).unwrap(), &root, now)
            .unwrap();
        store
            .accept_registry_security(
                &serde_json::to_vec(&keys["securityEnvelope"]).unwrap(),
                &root,
                now,
            )
            .unwrap();
        let envelope = &plans["releases"][2]["envelope"];
        let mod_id = ModId::try_from(1).unwrap();
        let release_id =
            ReleaseId(Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap());
        store
            .accept_registry_release(
                &serde_json::to_vec(envelope).unwrap(),
                &root,
                mod_id,
                release_id,
                now,
            )
            .unwrap();
        let identity = store
            .ready_registry_download(&root, mod_id, release_id, now)
            .unwrap();
        let prepared = Prepared {
            hash: identity.reference.sha256.as_str().into(),
            files: plans["cases"]["valid"][2]["inventory"]
                .as_array()
                .unwrap()
                .iter()
                .map(|file| PreparedFile {
                    path: file["path"].as_str().unwrap().into(),
                    size_bytes: file["bytes"].as_u64().unwrap(),
                    sha256: file["sha256"].as_str().unwrap().into(),
                })
                .collect(),
        };
        let mut operation = Operation {
            id: Uuid::new_v4().to_string(),
            request_id: Uuid::new_v4().to_string(),
            release_id: release_id.0.to_string(),
            hash: prepared.hash.clone(),
            kind: Kind::RegistryInstall,
            receipt_id: None,
            status: Status::Preparing,
            message: "Fixture preparation".into(),
            received_bytes: identity.bytes,
            total_bytes: identity.bytes,
        };
        store.save_package(&operation).unwrap();
        let display = RegistryDisplay {
            name: "Fixture map".into(),
            author: "Fixture author".into(),
        };
        store.conn.execute_batch("CREATE TRIGGER reject_registry_completion BEFORE UPDATE ON package_operations WHEN json_extract(NEW.record,'$.status')='completed' BEGIN SELECT RAISE(ABORT,'fixture failure'); END;").unwrap();
        operation.status = Status::Completed;
        assert!(
            store
                .complete_registry_install(&root, &identity, &display, &prepared, &operation, now)
                .is_err()
        );
        assert!(store.installed_registry_releases().unwrap().is_empty());
        assert!(store.registry_references().unwrap().is_empty());
        assert!(store.prepared_artifact(&prepared.hash).unwrap().is_none());
        assert_eq!(
            store.package_operations().unwrap()[0].status,
            Status::Preparing
        );
        store
            .conn
            .execute_batch("DROP TRIGGER reject_registry_completion")
            .unwrap();
        let first = store
            .complete_registry_install(&root, &identity, &display, &prepared, &operation, now)
            .unwrap();
        let display = RegistryDisplay {
            name: "Renamed map".into(),
            author: "Fixture author".into(),
        };
        let renamed = store
            .complete_registry_install(&root, &identity, &display, &prepared, &operation, now)
            .unwrap();
        assert_eq!(renamed.reference, first.reference);
        assert_eq!(renamed.display, display);

        let online = Ed25519KeyPair::from_seed_unchecked(&[9u8; 32]).unwrap();
        let sign = |mut envelope: Value| {
            envelope["signatures"][0]["signature"] = STANDARD
                .encode(
                    online
                        .sign(&serde_json::to_vec(&envelope["signed"]).unwrap())
                        .as_ref(),
                )
                .into();
            serde_json::to_vec(&envelope).unwrap()
        };
        let second_mod = ModId::try_from(2).unwrap();
        let second_release =
            ReleaseId(Uuid::parse_str("22222222-2222-4222-8222-222222222222").unwrap());
        let mut second = envelope.clone();
        second["signed"]["release"]["modId"] = 2.into();
        second["signed"]["release"]["releaseId"] = second_release.0.to_string().into();
        store
            .accept_registry_release(
                &sign(second.clone()),
                &root,
                second_mod,
                second_release,
                now,
            )
            .unwrap();
        let second_identity = store
            .ready_registry_download(&root, second_mod, second_release, now)
            .unwrap();
        let mut second_operation = operation.clone();
        second_operation.id = Uuid::new_v4().to_string();
        second_operation.request_id = Uuid::new_v4().to_string();
        second_operation.release_id = second_release.0.to_string();
        second_operation.status = Status::Preparing;
        store.save_package(&second_operation).unwrap();
        second_operation.status = Status::Completed;
        store
            .complete_registry_install(
                &root,
                &second_identity,
                &display,
                &prepared,
                &second_operation,
                now,
            )
            .unwrap();
        assert_eq!(store.installed_registry_releases().unwrap().len(), 2);
        assert_eq!(
            store
                .conn
                .query_row("SELECT count(*) FROM prepared_artifacts", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );

        second["signed"]["revision"] = 2.into();
        second["signed"]["release"]["metadata"]["installation"]["sourceRoot"] = "Maps/Other".into();
        store
            .accept_registry_release(&sign(second), &root, second_mod, second_release, now)
            .unwrap();
        assert!(
            store
                .confirm_registry_download(&root, &second_identity, now)
                .is_err()
        );
        assert!(
            store
                .complete_registry_install(
                    &root,
                    &second_identity,
                    &display,
                    &prepared,
                    &second_operation,
                    now
                )
                .is_err()
        );
        let installed = store.installed_registry_releases().unwrap();
        drop(store);
        let store = Storage::open(temp.path()).unwrap();
        assert_eq!(store.installed_registry_releases().unwrap(), installed);
        assert_eq!(store.package_operations().unwrap().len(), 2);
    }

    #[test]
    fn exact_registry_reference_survives_restart_and_cannot_change_hash() {
        let root = tempfile::tempdir().unwrap();
        let reference = ExactReference {
            mod_id: ModId::try_from(9_007_199_254_740_991).unwrap(),
            release_id: ReleaseId(Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap()),
            sha256: Sha256::try_from("a".repeat(64)).unwrap(),
        };
        {
            let mut store = Storage::open(root.path()).unwrap();
            store.save_registry_reference(&reference).unwrap();
            store.save_registry_reference(&reference).unwrap();
            let mut conflicting = reference.clone();
            conflicting.sha256 = Sha256::try_from("b".repeat(64)).unwrap();
            assert!(store.save_registry_reference(&conflicting).is_err());
            assert_eq!(
                store.registry_references().unwrap(),
                vec![reference.clone()]
            );
        }
        let store = Storage::open(root.path()).unwrap();
        assert!(store.has_registry_reference(&reference).unwrap());
        assert_eq!(store.registry_references().unwrap(), vec![reference]);
    }

    #[test]
    fn signed_security_blocks_survive_restart_omission_and_sign_out_state() {
        let root = tempfile::tempdir().unwrap();
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/registry-keys-v1.json"
        ))
        .unwrap();
        let root_public: [u8; 32] = STANDARD
            .decode(fixture["rootPublicKey"].as_str().unwrap())
            .unwrap()
            .try_into()
            .unwrap();
        let keys = serde_json::to_vec(&fixture["envelope"]).unwrap();
        let online = Ed25519KeyPair::from_seed_unchecked(&[9u8; 32]).unwrap();
        let now = OffsetDateTime::parse("2026-09-24T12:00:00Z", &Rfc3339).unwrap();
        let hash = "a".repeat(64);
        let security = |revision: u64, decisions: Vec<serde_json::Value>| {
            let signed = serde_json::json!({
                "type":"starframe-security","schemaVersion":1,"revision":revision,
                "issuedAt":"2026-09-24T00:00:00.000Z","expiresAt":"2026-09-25T00:00:00.000Z",
                "complete":true,"decisions":decisions
            });
            let canonical = serde_json::to_vec(&signed).unwrap();
            serde_json::to_vec(&serde_json::json!({"signed":signed,"signatures":[{
                "keyId":fixture["envelope"]["signed"]["keys"][0]["keyId"],
                "algorithm":"ed25519","signature":STANDARD.encode(online.sign(&canonical).as_ref())
            }]}))
            .unwrap()
        };
        {
            let mut store = Storage::open(root.path()).unwrap();
            store
                .accept_registry_keys(&keys, &root_public, now)
                .unwrap();
            store
                .accept_registry_security(
                    &security(
                        1,
                        vec![serde_json::json!({
                            "sha256":hash,"revision":1,"status":"blocked","reason":"Fixture block"
                        })],
                    ),
                    &root_public,
                    now,
                )
                .unwrap();
        }
        {
            let mut store = Storage::open(root.path()).unwrap();
            let decisions = store.registry_decisions().unwrap();
            assert_eq!(decisions.len(), 1);
            let hash = Sha256::try_from(hash.clone()).unwrap();
            assert!(store.registry_hash_blocked(&hash).unwrap());
            assert!(matches!(
                decisions[0].status,
                trust::DecisionStatus::Blocked
            ));
            assert!(
                store
                    .accept_registry_security(
                        &security(
                            1,
                            vec![serde_json::json!({
                                "sha256":hash.as_str(),"revision":1,"status":"cleared",
                                "reason":"Conflicting same-revision decision"
                            })],
                        ),
                        &root_public,
                        now,
                    )
                    .is_err()
            );
            assert!(store.registry_hash_blocked(&hash).unwrap());
            store
                .accept_registry_security(&security(2, vec![]), &root_public, now)
                .unwrap();
            assert!(matches!(
                store.registry_decisions().unwrap()[0].status,
                trust::DecisionStatus::Blocked
            ));
            assert!(
                store
                    .accept_registry_security(&security(1, vec![]), &root_public, now)
                    .is_err()
            );
            store.accept_registry_security(&security(3, vec![serde_json::json!({
                "sha256":hash,"revision":3,"status":"cleared","reason":"Reviewed and cleared"
            })]), &root_public, now).unwrap();
            assert!(!store.registry_hash_blocked(&hash).unwrap());
        }
        let store = Storage::open(root.path()).unwrap();
        assert!(matches!(
            store.registry_decisions().unwrap()[0].status,
            trust::DecisionStatus::Cleared
        ));
    }

    #[test]
    fn website_signed_manifest_is_bound_to_request_and_saved_per_release() {
        let root = tempfile::tempdir().unwrap();
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/registry-keys-v1.json"
        ))
        .unwrap();
        let root_public: [u8; 32] = STANDARD
            .decode(fixture["rootPublicKey"].as_str().unwrap())
            .unwrap()
            .try_into()
            .unwrap();
        let keys = serde_json::to_vec(&fixture["envelope"]).unwrap();
        let security = serde_json::to_vec(&fixture["securityEnvelope"]).unwrap();
        let manifest = serde_json::to_vec(&fixture["releaseEnvelope"]).unwrap();
        let blocked_manifest = serde_json::to_vec(&fixture["blockedReleaseEnvelope"]).unwrap();
        let now = OffsetDateTime::parse("2026-09-24T12:00:00Z", &Rfc3339).unwrap();
        let mod_id = ModId::try_from(1).unwrap();
        let release_id =
            ReleaseId(Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap());
        {
            let mut store = Storage::open(root.path()).unwrap();
            store
                .accept_registry_keys(&keys, &root_public, now)
                .unwrap();
            store
                .accept_registry_security(&security, &root_public, now)
                .unwrap();
            let accepted = store
                .accept_registry_release(&manifest, &root_public, mod_id, release_id, now)
                .unwrap();
            assert_eq!(accepted.revision, 1);
            assert!(accepted.reported_block.is_none());
            assert!(
                store
                    .accept_registry_release(
                        &manifest,
                        &root_public,
                        ModId::try_from(2).unwrap(),
                        release_id,
                        now,
                    )
                    .is_err()
            );
            let blocked_id =
                ReleaseId(Uuid::parse_str("22222222-2222-4222-8222-222222222222").unwrap());
            store
                .accept_registry_release(&blocked_manifest, &root_public, mod_id, blocked_id, now)
                .unwrap();
            assert!(store.registry_decisions().unwrap().iter().any(|decision| {
                decision.sha256.as_str() == "c".repeat(64)
                    && matches!(decision.status, trust::DecisionStatus::Blocked)
            }));
        }
        let store = Storage::open(root.path()).unwrap();
        let ready = store
            .ready_registry_download(&root_public, mod_id, release_id, now)
            .unwrap();
        assert_eq!(ready.reference.sha256.as_str(), "b".repeat(64));
        assert_eq!(ready.bytes, 2_147_483_648);
        assert_eq!(ready.metadata_revision, 1);
        assert_eq!(ready.security_revision, 1);
        store
            .confirm_registry_download(&root_public, &ready, now)
            .unwrap();
        assert!(
            store
                .ready_registry_download(
                    &root_public,
                    mod_id,
                    ReleaseId(Uuid::parse_str("22222222-2222-4222-8222-222222222222").unwrap()),
                    now,
                )
                .is_err()
        );
        assert!(
            store
                .ready_registry_download(
                    &root_public,
                    mod_id,
                    release_id,
                    now + time::Duration::days(2),
                )
                .is_err()
        );
        assert_eq!(
            store
                .registry_document(&format!("release:{}", release_id.0))
                .unwrap(),
            Some(manifest)
        );
        drop(store);
        let mut store = Storage::open(root.path()).unwrap();
        let changed = serde_json::json!({
            "type":"starframe-security","schemaVersion":1,"revision":2,
            "issuedAt":"2026-09-24T00:00:00.000Z","expiresAt":"2026-09-25T00:00:00.000Z",
            "complete":true,"decisions":[{
                "sha256":ready.reference.sha256.as_str(),"revision":2,
                "status":"blocked","reason":"Fixture withdrawal"
            }]
        });
        let online = Ed25519KeyPair::from_seed_unchecked(&[9u8; 32]).unwrap();
        let signature =
            STANDARD.encode(online.sign(&serde_json::to_vec(&changed).unwrap()).as_ref());
        let changed = serde_json::to_vec(&serde_json::json!({"signed":changed,"signatures":[{
            "keyId":fixture["envelope"]["signed"]["keys"][0]["keyId"],
            "algorithm":"ed25519","signature":signature
        }]}))
        .unwrap();
        store
            .accept_registry_security(&changed, &root_public, now)
            .unwrap();
        assert!(
            store
                .confirm_registry_download(&root_public, &ready, now)
                .is_err()
        );
        assert!(
            store
                .registry_hash_blocked(&ready.reference.sha256)
                .unwrap()
        );
    }
}
