use serde::{Deserialize, Serialize};
use std::{ffi::c_void, ptr};
use windows::{
    Win32::{
        Foundation::ERROR_NOT_FOUND,
        Security::Credentials::{
            CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredDeleteW, CredFree,
            CredReadW, CredWriteW,
        },
    },
    core::{HRESULT, PCWSTR, PWSTR},
};

const TARGET: &str = "Starframe:api.starframemanager.com:manager:v1";

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoredToken {
    pub schema_version: u8,
    pub token: String,
    pub expires_at: String,
}

#[derive(Clone)]
pub struct CredentialStore {
    target: String,
}

struct OwnedCredential(*mut CREDENTIALW);
impl Drop for OwnedCredential {
    fn drop(&mut self) {
        unsafe { CredFree(self.0.cast::<c_void>()) };
    }
}

impl CredentialStore {
    pub fn production() -> Self {
        Self {
            target: TARGET.into(),
        }
    }

    #[cfg(debug_assertions)]
    pub fn isolated(root: &std::path::Path) -> Self {
        use sha2::{Digest, Sha256};
        let digest = Sha256::digest(root.to_string_lossy().as_bytes());
        let suffix: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        Self {
            target: format!("Starframe:fixture:{suffix}"),
        }
    }

    #[cfg(test)]
    pub(super) fn fixture() -> Self {
        Self {
            target: format!("Starframe:fixture:{}", uuid::Uuid::new_v4()),
        }
    }

    fn target(&self) -> Vec<u16> {
        self.target.encode_utf16().chain([0]).collect()
    }

    pub fn save(&self, token: &StoredToken) -> Result<(), &'static str> {
        if token.schema_version != 1
            || token.token.len() != 43
            || !token
                .token
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
        {
            return Err("Invalid manager credential.");
        }
        let mut bytes = serde_json::to_vec(token).map_err(|_| "Invalid manager credential.")?;
        let mut target = self.target();
        let record = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(target.as_mut_ptr()),
            CredentialBlobSize: bytes.len() as u32,
            CredentialBlob: bytes.as_mut_ptr(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            ..Default::default()
        };
        unsafe { CredWriteW(&record, 0) }
            .map_err(|_| "Windows could not save the Starframe manager session.")
    }

    pub fn load(&self) -> Result<Option<StoredToken>, &'static str> {
        let target = self.target();
        let mut record = ptr::null_mut();
        match unsafe {
            CredReadW(
                PCWSTR(target.as_ptr()),
                CRED_TYPE_GENERIC,
                None,
                &mut record,
            )
        } {
            Ok(()) => (),
            Err(error) if error.code() == HRESULT::from_win32(ERROR_NOT_FOUND.0) => {
                return Ok(None);
            }
            Err(_) => return Err("Windows could not read the Starframe manager session."),
        }
        let owned = OwnedCredential(record);
        if owned.0.is_null() {
            return Err("Saved manager session is invalid.");
        }
        let size = unsafe { (*owned.0).CredentialBlobSize as usize };
        if size == 0 || size > 2560 {
            return Err("Saved manager session is invalid.");
        }
        if unsafe { (*owned.0).CredentialBlob.is_null() } {
            return Err("Saved manager session is invalid.");
        }
        let bytes = unsafe { std::slice::from_raw_parts((*owned.0).CredentialBlob, size) };
        let token: StoredToken =
            serde_json::from_slice(bytes).map_err(|_| "Saved manager session is invalid.")?;
        if token.schema_version != 1
            || token.token.len() != 43
            || !token
                .token
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
        {
            return Err("Saved manager session is invalid.");
        }
        Ok(Some(token))
    }

    pub fn delete(&self) -> Result<(), &'static str> {
        let target = self.target();
        match unsafe { CredDeleteW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, None) } {
            Ok(()) => Ok(()),
            Err(error) if error.code() == HRESULT::from_win32(ERROR_NOT_FOUND.0) => Ok(()),
            Err(_) => Err("Windows could not remove the Starframe manager session."),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isolated_windows_credential_round_trip_and_removal() {
        let store = CredentialStore::fixture();
        struct Cleanup<'a>(&'a CredentialStore);
        impl Drop for Cleanup<'_> {
            fn drop(&mut self) {
                let _ = self.0.delete();
            }
        }
        let _cleanup = Cleanup(&store);
        assert!(store.load().unwrap().is_none());
        let token = StoredToken {
            schema_version: 1,
            token: "A".repeat(43),
            expires_at: "2099-01-01T00:00:00Z".into(),
        };
        store.save(&token).unwrap();
        assert_eq!(store.load().unwrap().unwrap().token, token.token);
        store.delete().unwrap();
        assert!(store.load().unwrap().is_none());
    }

    #[cfg(debug_assertions)]
    #[test]
    fn debug_data_roots_never_use_the_production_target() {
        let first = CredentialStore::isolated(std::path::Path::new("fixture-one"));
        let second = CredentialStore::isolated(std::path::Path::new("fixture-two"));
        assert_ne!(first.target, CredentialStore::production().target);
        assert_ne!(first.target, second.target);
    }
}
