use std::{fmt, sync::Arc};

use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};

pub const APP_IDENTIFIER: &str = "com.gray.meetinglens";
const MAX_SECRET_BYTES: usize = 2048;
const WINDOWS_CREDENTIAL_MANAGER: &str = "windows_credential_manager";

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretKind {
    #[serde(rename = "deepseek_api_key")]
    DeepSeekApiKey,
}

impl SecretKind {
    const fn target_suffix(self) -> &'static str {
        match self {
            Self::DeepSeekApiKey => "deepseek-api-key",
        }
    }
}

pub struct SecretValue(Zeroizing<Vec<u8>>);

impl SecretValue {
    pub fn from_user_input(mut input: String) -> Result<Self, SecretError> {
        let value = Zeroizing::new(input.trim_ascii().as_bytes().to_vec());
        input.zeroize();

        if value.is_empty() {
            return Err(SecretError::InvalidInput);
        }
        if value.len() > MAX_SECRET_BYTES {
            return Err(SecretError::TooLarge);
        }

        Ok(Self(value))
    }

    fn from_stored_bytes(bytes: &[u8]) -> Result<Self, SecretError> {
        if bytes.is_empty() || bytes.len() > MAX_SECRET_BYTES || std::str::from_utf8(bytes).is_err()
        {
            return Err(SecretError::ReadFailed);
        }
        Ok(Self(Zeroizing::new(bytes.to_vec())))
    }

    pub fn expose_bytes(&self) -> &[u8] {
        self.0.as_slice()
    }

    pub fn expose_utf8(&self) -> Result<&str, SecretError> {
        std::str::from_utf8(self.expose_bytes()).map_err(|_| SecretError::ReadFailed)
    }
}

impl fmt::Debug for SecretValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

impl fmt::Display for SecretValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SecretBackendStatus {
    pub available: bool,
    pub backend: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecretError {
    InvalidInput,
    TooLarge,
    StoreUnavailable,
    WriteFailed,
    ReadFailed,
    DeleteFailed,
    AccessDenied,
    UnsupportedPlatform,
    Internal,
}

impl SecretError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidInput => "secret_invalid_input",
            Self::TooLarge => "secret_too_large",
            Self::StoreUnavailable => "secret_store_unavailable",
            Self::WriteFailed => "secret_write_failed",
            Self::ReadFailed => "secret_read_failed",
            Self::DeleteFailed => "secret_delete_failed",
            Self::AccessDenied => "secret_access_denied",
            Self::UnsupportedPlatform => "secret_unsupported_platform",
            Self::Internal => "internal_error",
        }
    }

    pub const fn safe_message(self) -> &'static str {
        match self {
            Self::InvalidInput => "Enter a non-empty secret.",
            Self::TooLarge => "The secret is too large.",
            Self::StoreUnavailable => "Secure local storage is unavailable.",
            Self::WriteFailed => "The secret could not be saved.",
            Self::ReadFailed => "The secret status could not be read.",
            Self::DeleteFailed => "The secret could not be deleted.",
            Self::AccessDenied => "Secure local storage denied access.",
            Self::UnsupportedPlatform => "Secure local storage is unavailable on this platform.",
            Self::Internal => "An internal error occurred.",
        }
    }

    pub const fn retryable(self) -> bool {
        matches!(
            self,
            Self::StoreUnavailable | Self::WriteFailed | Self::ReadFailed | Self::DeleteFailed
        )
    }
}

impl fmt::Display for SecretError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.safe_message())
    }
}

impl std::error::Error for SecretError {}

pub trait SecretStore: Send + Sync {
    fn set(&self, kind: SecretKind, secret: &SecretValue) -> Result<(), SecretError>;
    fn get_for_backend(&self, kind: SecretKind) -> Result<Option<SecretValue>, SecretError>;
    fn contains(&self, kind: SecretKind) -> Result<bool, SecretError>;
    fn delete(&self, kind: SecretKind) -> Result<bool, SecretError>;
    fn backend_status(&self) -> SecretBackendStatus;
}

pub struct SecretStoreService {
    backend: Arc<dyn SecretStore>,
}

impl SecretStoreService {
    pub fn production() -> Self {
        Self {
            backend: production_backend(),
        }
    }

    #[cfg(test)]
    pub fn with_backend(backend: Arc<dyn SecretStore>) -> Self {
        Self { backend }
    }

    pub fn set(&self, kind: SecretKind, input: String) -> Result<(), SecretError> {
        let secret = SecretValue::from_user_input(input)?;
        self.backend.set(kind, &secret)
    }

    pub fn get_for_backend(&self, kind: SecretKind) -> Result<Option<SecretValue>, SecretError> {
        self.backend.get_for_backend(kind)
    }

    pub fn contains(&self, kind: SecretKind) -> Result<bool, SecretError> {
        self.backend.contains(kind)
    }

    pub fn delete(&self, kind: SecretKind) -> Result<bool, SecretError> {
        self.backend.delete(kind)
    }

    pub fn backend_status(&self) -> SecretBackendStatus {
        self.backend.backend_status()
    }
}

#[cfg(windows)]
fn production_backend() -> Arc<dyn SecretStore> {
    Arc::new(windows_backend::WindowsCredentialStore::production())
}

#[cfg(not(windows))]
fn production_backend() -> Arc<dyn SecretStore> {
    Arc::new(UnsupportedSecretStore)
}

#[cfg(not(windows))]
struct UnsupportedSecretStore;

#[cfg(not(windows))]
impl SecretStore for UnsupportedSecretStore {
    fn set(&self, _kind: SecretKind, _secret: &SecretValue) -> Result<(), SecretError> {
        Err(SecretError::UnsupportedPlatform)
    }

    fn get_for_backend(&self, _kind: SecretKind) -> Result<Option<SecretValue>, SecretError> {
        Err(SecretError::UnsupportedPlatform)
    }

    fn contains(&self, _kind: SecretKind) -> Result<bool, SecretError> {
        Err(SecretError::UnsupportedPlatform)
    }

    fn delete(&self, _kind: SecretKind) -> Result<bool, SecretError> {
        Err(SecretError::UnsupportedPlatform)
    }

    fn backend_status(&self) -> SecretBackendStatus {
        SecretBackendStatus {
            available: false,
            backend: WINDOWS_CREDENTIAL_MANAGER,
        }
    }
}

#[cfg(windows)]
mod windows_backend {
    use std::{ffi::c_void, ptr, slice};

    use windows::{
        Win32::{
            Foundation::{ERROR_ACCESS_DENIED, ERROR_NO_SUCH_LOGON_SESSION, ERROR_NOT_FOUND},
            Security::Credentials::{
                CRED_MAX_CREDENTIAL_BLOB_SIZE, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
                CREDENTIALW, CredDeleteW, CredFree, CredReadW, CredWriteW,
            },
        },
        core::{Error as WindowsError, HRESULT, PCWSTR, PWSTR},
    };

    use super::*;

    pub struct WindowsCredentialStore {
        namespace: CredentialNamespace,
    }

    enum CredentialNamespace {
        Production,
        #[cfg(test)]
        Test(String),
    }

    impl WindowsCredentialStore {
        pub fn production() -> Self {
            Self {
                namespace: CredentialNamespace::Production,
            }
        }

        #[cfg(test)]
        fn for_test(namespace: String) -> Self {
            Self {
                namespace: CredentialNamespace::Test(namespace),
            }
        }

        fn target(&self, kind: SecretKind) -> String {
            let namespace = match &self.namespace {
                CredentialNamespace::Production => APP_IDENTIFIER,
                #[cfg(test)]
                CredentialNamespace::Test(namespace) => namespace,
            };
            format!("{namespace}/secret/{}", kind.target_suffix())
        }

        fn read_credential(
            &self,
            kind: SecretKind,
        ) -> Result<Option<CredentialBuffer>, SecretError> {
            let target = wide_null(&self.target(kind));
            let mut credential = ptr::null_mut();
            // SAFETY: target is NUL-terminated and lives for the call; credential is a valid out pointer.
            match unsafe {
                CredReadW(
                    PCWSTR(target.as_ptr()),
                    CRED_TYPE_GENERIC,
                    None,
                    &mut credential,
                )
            } {
                Ok(()) if credential.is_null() => Err(SecretError::ReadFailed),
                Ok(()) => Ok(Some(CredentialBuffer(credential))),
                Err(error) if is_windows_error(&error, ERROR_NOT_FOUND.0) => Ok(None),
                Err(error) => Err(map_windows_error(error, Operation::Read)),
            }
        }
    }

    impl SecretStore for WindowsCredentialStore {
        fn set(&self, kind: SecretKind, secret: &SecretValue) -> Result<(), SecretError> {
            let mut target = wide_null(&self.target(kind));
            let mut username = wide_null(APP_IDENTIFIER);
            let credential = CREDENTIALW {
                Type: CRED_TYPE_GENERIC,
                TargetName: PWSTR(target.as_mut_ptr()),
                CredentialBlobSize: secret
                    .expose_bytes()
                    .len()
                    .try_into()
                    .map_err(|_| SecretError::TooLarge)?,
                CredentialBlob: secret.expose_bytes().as_ptr().cast_mut(),
                Persist: CRED_PERSIST_LOCAL_MACHINE,
                UserName: PWSTR(username.as_mut_ptr()),
                ..Default::default()
            };

            // SAFETY: all pointers in credential reference live buffers for the duration of the call.
            let result = unsafe { CredWriteW(&credential, 0) };
            target.zeroize();
            username.zeroize();
            result.map_err(|error| map_windows_error(error, Operation::Write))
        }

        fn get_for_backend(&self, kind: SecretKind) -> Result<Option<SecretValue>, SecretError> {
            let Some(credential) = self.read_credential(kind)? else {
                return Ok(None);
            };
            let bytes = credential.blob()?;
            SecretValue::from_stored_bytes(bytes).map(Some)
        }

        fn contains(&self, kind: SecretKind) -> Result<bool, SecretError> {
            self.read_credential(kind)
                .map(|credential| credential.is_some())
        }

        fn delete(&self, kind: SecretKind) -> Result<bool, SecretError> {
            let mut target = wide_null(&self.target(kind));
            // SAFETY: target is NUL-terminated and lives for the duration of the call.
            let result = unsafe { CredDeleteW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, None) };
            target.zeroize();
            match result {
                Ok(()) => Ok(true),
                Err(error) if is_windows_error(&error, ERROR_NOT_FOUND.0) => Ok(false),
                Err(error) => Err(map_windows_error(error, Operation::Delete)),
            }
        }

        fn backend_status(&self) -> SecretBackendStatus {
            SecretBackendStatus {
                available: true,
                backend: WINDOWS_CREDENTIAL_MANAGER,
            }
        }
    }

    struct CredentialBuffer(*mut CREDENTIALW);

    impl CredentialBuffer {
        fn blob(&self) -> Result<&[u8], SecretError> {
            // SAFETY: CredReadW returned this allocation, which remains owned by this guard.
            let credential = unsafe { self.0.as_ref() }.ok_or(SecretError::ReadFailed)?;
            let length: usize = credential
                .CredentialBlobSize
                .try_into()
                .map_err(|_| SecretError::ReadFailed)?;
            if length == 0 || length > MAX_SECRET_BYTES || credential.CredentialBlob.is_null() {
                return Err(SecretError::ReadFailed);
            }
            // SAFETY: WinCred guarantees CredentialBlob has CredentialBlobSize readable bytes.
            Ok(unsafe { slice::from_raw_parts(credential.CredentialBlob, length) })
        }
    }

    impl Drop for CredentialBuffer {
        fn drop(&mut self) {
            if self.0.is_null() {
                return;
            }
            // SAFETY: the buffer was returned by CredReadW and is freed exactly once here.
            unsafe {
                if let Some(credential) = self.0.as_mut() {
                    let length = credential.CredentialBlobSize as usize;
                    if !credential.CredentialBlob.is_null()
                        && length <= CRED_MAX_CREDENTIAL_BLOB_SIZE as usize
                    {
                        ptr::write_bytes(credential.CredentialBlob, 0, length);
                    }
                }
                CredFree(self.0.cast::<c_void>());
            }
        }
    }

    #[derive(Clone, Copy)]
    enum Operation {
        Write,
        Read,
        Delete,
    }

    fn map_windows_error(error: WindowsError, operation: Operation) -> SecretError {
        if is_windows_error(&error, ERROR_ACCESS_DENIED.0) {
            SecretError::AccessDenied
        } else if is_windows_error(&error, ERROR_NO_SUCH_LOGON_SESSION.0) {
            SecretError::StoreUnavailable
        } else {
            match operation {
                Operation::Write => SecretError::WriteFailed,
                Operation::Read => SecretError::ReadFailed,
                Operation::Delete => SecretError::DeleteFailed,
            }
        }
    }

    fn is_windows_error(error: &WindowsError, code: u32) -> bool {
        error.code() == HRESULT::from_win32(code)
    }

    fn wide_null(value: &str) -> Vec<u16> {
        value.encode_utf16().chain([0]).collect()
    }

    #[cfg(test)]
    mod tests {
        use uuid::Uuid;

        use super::*;

        struct NativeCredentialCleanup<'a> {
            store: &'a WindowsCredentialStore,
        }

        impl Drop for NativeCredentialCleanup<'_> {
            fn drop(&mut self) {
                let _ = self.store.delete(SecretKind::DeepSeekApiKey);
            }
        }

        #[test]
        fn native_windows_credential_manager_write_read_overwrite_and_delete() {
            let namespace = format!("{APP_IDENTIFIER}/test/{}", Uuid::new_v4());
            let store = WindowsCredentialStore::for_test(namespace);
            let _cleanup = NativeCredentialCleanup { store: &store };
            let first = generated_secret();
            let second = generated_secret();

            assert!(!store.contains(SecretKind::DeepSeekApiKey).unwrap());
            store.set(SecretKind::DeepSeekApiKey, &first).unwrap();
            assert!(store.contains(SecretKind::DeepSeekApiKey).unwrap());
            let stored = store
                .get_for_backend(SecretKind::DeepSeekApiKey)
                .unwrap()
                .expect("native credential should be present");
            assert!(
                stored.expose_bytes() == first.expose_bytes(),
                "native credential contents did not round-trip"
            );

            store.set(SecretKind::DeepSeekApiKey, &second).unwrap();
            let overwritten = store
                .get_for_backend(SecretKind::DeepSeekApiKey)
                .unwrap()
                .expect("overwritten credential should be present");
            assert!(
                overwritten.expose_bytes() == second.expose_bytes(),
                "native credential was not overwritten"
            );

            assert!(store.delete(SecretKind::DeepSeekApiKey).unwrap());
            assert!(!store.delete(SecretKind::DeepSeekApiKey).unwrap());
            assert!(!store.contains(SecretKind::DeepSeekApiKey).unwrap());
        }

        #[test]
        fn native_test_target_is_unique_and_isolated_from_production() {
            let namespace = format!("{APP_IDENTIFIER}/test/{}", Uuid::new_v4());
            let store = WindowsCredentialStore::for_test(namespace);
            let test_target = store.target(SecretKind::DeepSeekApiKey);
            let production_target =
                WindowsCredentialStore::production().target(SecretKind::DeepSeekApiKey);

            assert_ne!(test_target, production_target);
            assert!(test_target.starts_with(&format!("{APP_IDENTIFIER}/test/")));
            assert_eq!(
                production_target,
                "com.gray.meetinglens/secret/deepseek-api-key"
            );
        }

        fn generated_secret() -> SecretValue {
            SecretValue::from_user_input(format!("{}{}", Uuid::new_v4(), Uuid::new_v4()))
                .expect("generated test material should be valid")
        }
    }
}

#[cfg(test)]
pub mod test_support {
    use std::{collections::HashMap, sync::Mutex};

    use super::*;

    #[derive(Default)]
    pub struct InMemorySecretStore {
        values: Mutex<HashMap<SecretKind, Zeroizing<Vec<u8>>>>,
    }

    impl SecretStore for InMemorySecretStore {
        fn set(&self, kind: SecretKind, secret: &SecretValue) -> Result<(), SecretError> {
            self.values
                .lock()
                .map_err(|_| SecretError::Internal)?
                .insert(kind, Zeroizing::new(secret.expose_bytes().to_vec()));
            Ok(())
        }

        fn get_for_backend(&self, kind: SecretKind) -> Result<Option<SecretValue>, SecretError> {
            self.values
                .lock()
                .map_err(|_| SecretError::Internal)?
                .get(&kind)
                .map(|value| SecretValue::from_stored_bytes(value.as_slice()))
                .transpose()
        }

        fn contains(&self, kind: SecretKind) -> Result<bool, SecretError> {
            self.values
                .lock()
                .map(|values| values.contains_key(&kind))
                .map_err(|_| SecretError::Internal)
        }

        fn delete(&self, kind: SecretKind) -> Result<bool, SecretError> {
            self.values
                .lock()
                .map(|mut values| values.remove(&kind).is_some())
                .map_err(|_| SecretError::Internal)
        }

        fn backend_status(&self) -> SecretBackendStatus {
            SecretBackendStatus {
                available: true,
                backend: WINDOWS_CREDENTIAL_MANAGER,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use uuid::Uuid;

    use super::{test_support::InMemorySecretStore, *};

    #[test]
    fn secret_kind_has_one_stable_production_target() {
        let kinds = [SecretKind::DeepSeekApiKey];
        assert_eq!(kinds.len(), 1);
        assert_eq!(
            format!(
                "{APP_IDENTIFIER}/secret/{}",
                SecretKind::DeepSeekApiKey.target_suffix()
            ),
            "com.gray.meetinglens/secret/deepseek-api-key"
        );
    }

    #[test]
    fn arbitrary_strings_cannot_become_secret_kinds() {
        assert_eq!(
            serde_json::from_str::<SecretKind>("\"deepseek_api_key\"").unwrap(),
            SecretKind::DeepSeekApiKey
        );
        assert!(serde_json::from_str::<SecretKind>("\"arbitrary_target\"").is_err());
        assert!(serde_json::from_str::<SecretKind>("\"filename\"").is_err());
    }

    #[test]
    fn user_input_is_trimmed_and_validated_without_prefix_rules() {
        let generated = Uuid::new_v4().to_string();
        let value = SecretValue::from_user_input(format!("  {generated}\r\n"))
            .expect("generated input should be accepted");
        assert_eq!(value.expose_utf8().unwrap(), generated);
        assert_eq!(
            SecretValue::from_user_input(String::new()).unwrap_err(),
            SecretError::InvalidInput
        );
        assert_eq!(
            SecretValue::from_user_input(" \t\r\n".to_string()).unwrap_err(),
            SecretError::InvalidInput
        );
        assert_eq!(
            SecretValue::from_user_input("x".repeat(MAX_SECRET_BYTES + 1)).unwrap_err(),
            SecretError::TooLarge
        );
        assert_eq!(
            SecretValue::from_user_input("界".repeat(683)).unwrap_err(),
            SecretError::TooLarge
        );
    }

    #[test]
    fn secret_value_debug_and_display_are_redacted() {
        let value = generated_secret();
        assert_eq!(format!("{value:?}"), "[REDACTED]");
        assert_eq!(format!("{value}"), "[REDACTED]");
    }

    #[test]
    fn secret_value_has_no_clone_serialize_or_implicit_deref_implementation() {
        let source = include_str!("secret_store.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("production SecretStore source should exist");
        let declaration = production
            .split("pub struct SecretValue")
            .nth(1)
            .expect("SecretValue declaration should exist")
            .split("impl SecretValue")
            .next()
            .expect("SecretValue implementation should follow its declaration");
        assert!(!declaration.contains("derive"));
        assert!(!production.contains("impl Clone for SecretValue"));
        assert!(!production.contains("impl Serialize for SecretValue"));
        assert!(!production.contains("impl Deref for SecretValue"));
    }

    #[test]
    fn in_memory_store_supports_set_get_overwrite_contains_and_idempotent_delete() {
        let backend = Arc::new(InMemorySecretStore::default());
        let service = SecretStoreService::with_backend(backend);
        let first = Uuid::new_v4().to_string();
        let second = Uuid::new_v4().to_string();

        assert!(!service.contains(SecretKind::DeepSeekApiKey).unwrap());
        service
            .set(SecretKind::DeepSeekApiKey, first.clone())
            .unwrap();
        assert!(service.contains(SecretKind::DeepSeekApiKey).unwrap());
        let stored = service
            .get_for_backend(SecretKind::DeepSeekApiKey)
            .unwrap()
            .expect("secret should be present");
        assert!(
            stored.expose_bytes() == first.as_bytes(),
            "in-memory secret did not round-trip"
        );

        service
            .set(SecretKind::DeepSeekApiKey, second.clone())
            .unwrap();
        let overwritten = service
            .get_for_backend(SecretKind::DeepSeekApiKey)
            .unwrap()
            .expect("overwritten secret should be present");
        assert!(
            overwritten.expose_bytes() == second.as_bytes(),
            "in-memory secret was not overwritten"
        );

        assert!(service.delete(SecretKind::DeepSeekApiKey).unwrap());
        assert!(!service.delete(SecretKind::DeepSeekApiKey).unwrap());
    }

    #[test]
    fn every_error_has_a_stable_safe_protocol() {
        let errors = [
            SecretError::InvalidInput,
            SecretError::TooLarge,
            SecretError::StoreUnavailable,
            SecretError::WriteFailed,
            SecretError::ReadFailed,
            SecretError::DeleteFailed,
            SecretError::AccessDenied,
            SecretError::UnsupportedPlatform,
            SecretError::Internal,
        ];
        let codes: Vec<_> = errors.iter().map(|error| error.code()).collect();
        assert_eq!(codes.len(), 9);
        assert_eq!(
            codes
                .iter()
                .copied()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            9
        );
        for error in errors {
            assert!(!error.safe_message().is_empty());
            assert!(!error.safe_message().contains(APP_IDENTIFIER));
            assert!(!error.safe_message().contains('\\'));
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn non_windows_backend_fails_closed_without_plaintext_storage() {
        let service = SecretStoreService::production();
        assert!(!service.backend_status().available);
        assert_eq!(
            service
                .set(SecretKind::DeepSeekApiKey, Uuid::new_v4().to_string())
                .unwrap_err(),
            SecretError::UnsupportedPlatform
        );
    }

    fn generated_secret() -> SecretValue {
        SecretValue::from_user_input(Uuid::new_v4().to_string())
            .expect("generated test material should be valid")
    }
}
