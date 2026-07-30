use serde::{Deserialize, Serialize};
use tauri::State;

use crate::secret_store::{SecretError, SecretKind, SecretStoreService};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetSecretRequest {
    pub kind: SecretKind,
    pub value: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SecretStatusRequest {
    pub kind: SecretKind,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeleteSecretRequest {
    pub kind: SecretKind,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretStatusResponse {
    pub kind: SecretKind,
    pub configured: bool,
    pub available: bool,
    pub backend: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteSecretResponse {
    pub kind: SecretKind,
    pub deleted: bool,
    pub configured: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretCommandError {
    pub code: &'static str,
    pub message: &'static str,
    pub retryable: bool,
}

impl From<SecretError> for SecretCommandError {
    fn from(error: SecretError) -> Self {
        Self {
            code: error.code(),
            message: error.safe_message(),
            retryable: error.retryable(),
        }
    }
}

type CommandResult<T> = Result<T, SecretCommandError>;

#[tauri::command]
pub fn get_secret_status(
    request: SecretStatusRequest,
    service: State<'_, SecretStoreService>,
) -> CommandResult<SecretStatusResponse> {
    get_secret_status_with_service(&service, request)
}

#[tauri::command]
pub fn set_secret(
    request: SetSecretRequest,
    service: State<'_, SecretStoreService>,
) -> CommandResult<SecretStatusResponse> {
    set_secret_with_service(&service, request)
}

#[tauri::command]
pub fn delete_secret(
    request: DeleteSecretRequest,
    service: State<'_, SecretStoreService>,
) -> CommandResult<DeleteSecretResponse> {
    delete_secret_with_service(&service, request)
}

fn get_secret_status_with_service(
    service: &SecretStoreService,
    request: SecretStatusRequest,
) -> CommandResult<SecretStatusResponse> {
    status_for(service, request.kind).map_err(SecretCommandError::from)
}

fn set_secret_with_service(
    service: &SecretStoreService,
    request: SetSecretRequest,
) -> CommandResult<SecretStatusResponse> {
    service
        .set(request.kind, request.value)
        .map_err(SecretCommandError::from)?;
    status_for(service, request.kind).map_err(SecretCommandError::from)
}

fn delete_secret_with_service(
    service: &SecretStoreService,
    request: DeleteSecretRequest,
) -> CommandResult<DeleteSecretResponse> {
    let deleted = service
        .delete(request.kind)
        .map_err(SecretCommandError::from)?;
    Ok(DeleteSecretResponse {
        kind: request.kind,
        deleted,
        configured: false,
    })
}

fn status_for(
    service: &SecretStoreService,
    kind: SecretKind,
) -> Result<SecretStatusResponse, SecretError> {
    let backend = service.backend_status();
    let configured = if backend.available {
        service.contains(kind)?
    } else {
        false
    };
    Ok(SecretStatusResponse {
        kind,
        configured,
        available: backend.available,
        backend: backend.backend,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;
    use uuid::Uuid;

    use crate::secret_store::test_support::InMemorySecretStore;

    use super::*;

    fn service() -> SecretStoreService {
        SecretStoreService::with_backend(Arc::new(InMemorySecretStore::default()))
    }

    #[test]
    fn commands_support_status_set_overwrite_and_idempotent_delete() {
        let service = service();
        let kind = SecretKind::DeepSeekApiKey;
        let initial = get_secret_status_with_service(&service, SecretStatusRequest { kind })
            .expect("initial status should succeed");
        assert!(!initial.configured);
        assert!(initial.available);

        let first = Uuid::new_v4().to_string();
        let saved = set_secret_with_service(&service, SetSecretRequest { kind, value: first })
            .expect("set should succeed");
        assert!(saved.configured);

        let second = Uuid::new_v4().to_string();
        let overwritten = set_secret_with_service(
            &service,
            SetSecretRequest {
                kind,
                value: second.clone(),
            },
        )
        .expect("overwrite should succeed");
        assert!(overwritten.configured);
        let stored = service
            .get_for_backend(kind)
            .expect("trusted read should succeed")
            .expect("trusted read should return a value");
        assert!(
            stored.expose_bytes() == second.as_bytes(),
            "trusted backend did not receive the overwritten value"
        );

        let deleted = delete_secret_with_service(&service, DeleteSecretRequest { kind })
            .expect("delete should succeed");
        assert!(deleted.deleted);
        assert!(!deleted.configured);
        let missing = delete_secret_with_service(&service, DeleteSecretRequest { kind })
            .expect("missing delete should remain idempotent");
        assert!(!missing.deleted);
        assert!(!missing.configured);
    }

    #[test]
    fn command_responses_never_serialize_secret_material_or_target_names() {
        let response = SecretStatusResponse {
            kind: SecretKind::DeepSeekApiKey,
            configured: true,
            available: true,
            backend: "windows_credential_manager",
        };
        let value = serde_json::to_value(response).expect("response should serialize");
        assert_eq!(
            value,
            json!({
                "kind": "deepseek_api_key",
                "configured": true,
                "available": true,
                "backend": "windows_credential_manager"
            })
        );
        let serialized = value.to_string();
        assert!(!serialized.contains("value"));
        assert!(!serialized.contains("target"));
        assert!(!serialized.contains("com.gray"));
    }

    #[test]
    fn command_errors_use_only_stable_safe_fields() {
        let service = service();
        let error = set_secret_with_service(
            &service,
            SetSecretRequest {
                kind: SecretKind::DeepSeekApiKey,
                value: " \t".to_string(),
            },
        )
        .expect_err("blank secret should fail");
        assert_eq!(error.code, "secret_invalid_input");
        let serialized = serde_json::to_string(&error).expect("error should serialize");
        assert!(!serialized.contains("target"));
        assert!(!serialized.contains("Win32"));
        assert!(!serialized.contains("com.gray"));
    }

    #[test]
    fn only_three_non_revealing_secret_commands_are_registered_and_allowed() {
        let commands_source = include_str!("secret_commands.rs");
        let production_commands = commands_source
            .split("#[cfg(test)]")
            .next()
            .expect("production command source should exist");
        let app_source = include_str!("lib.rs");
        let build_source = include_str!("../build.rs");
        let permissions = include_str!("../permissions/ui-commands.toml");
        let capability = include_str!("../capabilities/default.json");

        for command in ["get_secret_status", "set_secret", "delete_secret"] {
            assert!(production_commands.contains(&format!("pub fn {command}")));
            assert!(app_source.contains(&format!("secret_commands::{command}")));
            assert!(build_source.contains(&format!("\"{command}\"")));
            assert!(permissions.contains(&format!("allow-{}", command.replace('_', "-"))));
        }
        for forbidden in [
            "pub fn get_secret(",
            "pub fn reveal_secret(",
            "pub fn export_secret(",
            "pub fn copy_secret(",
            "list_all_credentials",
            "enumerate_windows_credentials",
        ] {
            assert!(!production_commands.contains(forbidden));
            assert!(!permissions.contains(forbidden));
        }
        assert!(capability.contains("\"windows\": [\"main\"]"));
        assert!(capability.contains("\"local\": true"));
        assert!(!capability.contains("\"windows\": [\"*\"]"));
        assert!(!capability.contains("remote"));
        assert!(!capability.contains("shell:"));
        assert!(!capability.contains("fs:"));
        assert!(!capability.contains("http:"));
        assert!(!capability.contains("process:"));
    }

    #[test]
    fn command_layer_uses_the_service_instead_of_win32_apis() {
        let production = include_str!("secret_commands.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("production source should exist");
        assert!(production.contains("SecretStoreService"));
        for forbidden in ["CredWrite", "CredRead", "CredDelete", "CredentialBlob"] {
            assert!(!production.contains(forbidden));
        }
    }
}
