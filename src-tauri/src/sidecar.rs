use serde::{Deserialize, Serialize};
use std::time::Duration;

const SIDECAR_HEALTH_URL: &str = "http://127.0.0.1:8765/health";
const EXPECTED_SERVICE: &str = "meetinglens-sidecar";
const EXPECTED_API_VERSION: &str = "v1";

#[derive(Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SidecarHealth {
    status: String,
    service: String,
    version: String,
    api_version: String,
}

fn validate_health(health: SidecarHealth) -> Result<SidecarHealth, String> {
    if health.status != "ok"
        || health.service != EXPECTED_SERVICE
        || health.api_version != EXPECTED_API_VERSION
        || health.version.trim().is_empty()
    {
        return Err("sidecar_invalid_response".to_string());
    }

    Ok(health)
}

#[tauri::command]
pub async fn check_sidecar_health() -> Result<SidecarHealth, String> {
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(1))
        .timeout(Duration::from_secs(2))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "sidecar_client_error".to_string())?;

    let response = client
        .get(SIDECAR_HEALTH_URL)
        .send()
        .await
        .map_err(|_| "sidecar_unreachable".to_string())?;

    if !response.status().is_success() {
        return Err("sidecar_unhealthy".to_string());
    }

    let health = response
        .json::<SidecarHealth>()
        .await
        .map_err(|_| "sidecar_invalid_response".to_string())?;

    validate_health(health)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_health() -> SidecarHealth {
        SidecarHealth {
            status: "ok".to_string(),
            service: EXPECTED_SERVICE.to_string(),
            version: "0.1.0".to_string(),
            api_version: EXPECTED_API_VERSION.to_string(),
        }
    }

    #[test]
    fn accepts_the_expected_contract() {
        assert_eq!(validate_health(valid_health()), Ok(valid_health()));
    }

    #[test]
    fn rejects_an_unexpected_service() {
        let mut health = valid_health();
        health.service = "another-process".to_string();

        assert_eq!(
            validate_health(health),
            Err("sidecar_invalid_response".to_string())
        );
    }
}
