use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use std::fmt::{self, Write as _};
use std::net::{Ipv4Addr, SocketAddrV4, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::State;

const SIDECAR_HOST: &str = "127.0.0.1";
const SIDECAR_PORT: u16 = 8765;
const SIDECAR_TOKEN_ENV: &str = "MEETINGLENS_SESSION_TOKEN";
const EXPECTED_SERVICE: &str = "meetinglens-sidecar";
const EXPECTED_API_VERSION: &str = "v1";

#[derive(Clone, PartialEq, Eq)]
struct SessionToken(String);

impl SessionToken {
    fn generate() -> Result<Self, SidecarCommandError> {
        let mut random = [0_u8; 32];
        getrandom::fill(&mut random)
            .map_err(|_| SidecarCommandError::new(SidecarErrorCode::InternalError))?;

        let mut encoded = String::with_capacity(random.len() * 2);
        for byte in random {
            write!(&mut encoded, "{byte:02x}")
                .map_err(|_| SidecarCommandError::new(SidecarErrorCode::InternalError))?;
        }
        Ok(Self(encoded))
    }

    #[cfg(test)]
    fn for_test(value: &str) -> Self {
        Self(value.to_owned())
    }

    fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SessionToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SessionToken([REDACTED])")
    }
}

#[derive(Clone)]
struct SidecarSession {
    token: SessionToken,
    endpoint: String,
}

impl SidecarSession {
    fn current(token: SessionToken) -> Self {
        Self {
            token,
            endpoint: format!("http://{SIDECAR_HOST}:{SIDECAR_PORT}/health"),
        }
    }

    #[cfg(test)]
    fn for_test(token: SessionToken, endpoint: String) -> Self {
        Self { token, endpoint }
    }
}

impl fmt::Debug for SidecarSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SidecarSession")
            .field("token", &self.token)
            .field("endpoint", &"[LOOPBACK]")
            .finish()
    }
}

#[derive(Clone)]
struct SidecarClient {
    http: reqwest::Client,
    session: SidecarSession,
}

impl SidecarClient {
    fn new(session: SidecarSession) -> Result<Self, SidecarCommandError> {
        Self::with_timeouts(session, Duration::from_secs(1), Duration::from_secs(2))
    }

    fn with_timeouts(
        session: SidecarSession,
        connect_timeout: Duration,
        request_timeout: Duration,
    ) -> Result<Self, SidecarCommandError> {
        let mut headers = HeaderMap::new();
        let mut authorization =
            HeaderValue::from_str(&format!("Bearer {}", session.token.expose()))
                .map_err(|_| SidecarCommandError::new(SidecarErrorCode::InternalError))?;
        authorization.set_sensitive(true);
        headers.insert(AUTHORIZATION, authorization);

        let http = reqwest::Client::builder()
            .connect_timeout(connect_timeout)
            .timeout(request_timeout)
            .redirect(reqwest::redirect::Policy::none())
            .default_headers(headers)
            .build()
            .map_err(|_| SidecarCommandError::new(SidecarErrorCode::InternalError))?;

        Ok(Self { http, session })
    }

    async fn check_health(&self) -> Result<SidecarStatus, SidecarCommandError> {
        let response = self
            .http
            .get(&self.session.endpoint)
            .send()
            .await
            .map_err(map_request_error)?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(SidecarCommandError::new(
                SidecarErrorCode::SidecarUnauthorized,
            ));
        }
        if !response.status().is_success() {
            return Err(SidecarCommandError::new(
                SidecarErrorCode::SidecarRequestFailed,
            ));
        }

        let health = response
            .json::<SidecarHealth>()
            .await
            .map_err(|_| SidecarCommandError::new(SidecarErrorCode::SidecarInvalidResponse))?;
        validate_health(&health)?;

        Ok(SidecarStatus {
            status: SidecarStatusKind::Authenticated,
            binding: SidecarBinding::Loopback,
            session: SidecarSessionKind::Current,
            secret_storage: SecretStorageStatus::NotImplemented,
            service: health.service,
            version: health.version,
            api_version: health.api_version,
        })
    }
}

impl fmt::Debug for SidecarClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SidecarClient")
            .field("session", &self.session)
            .field("http", &"[CONFIGURED]")
            .finish()
    }
}

fn map_request_error(error: reqwest::Error) -> SidecarCommandError {
    if error.is_timeout() {
        SidecarCommandError::new(SidecarErrorCode::SidecarTimeout)
    } else if error.is_connect() {
        SidecarCommandError::new(SidecarErrorCode::SidecarUnavailable)
    } else {
        SidecarCommandError::new(SidecarErrorCode::SidecarRequestFailed)
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct SidecarHealth {
    status: String,
    service: String,
    version: String,
    api_version: String,
}

fn validate_health(health: &SidecarHealth) -> Result<(), SidecarCommandError> {
    if health.status != "ok"
        || health.service != EXPECTED_SERVICE
        || health.api_version != EXPECTED_API_VERSION
        || health.version.trim().is_empty()
    {
        return Err(SidecarCommandError::new(
            SidecarErrorCode::SidecarInvalidResponse,
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum SidecarStatusKind {
    Authenticated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum SidecarBinding {
    Loopback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum SidecarSessionKind {
    Current,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum SecretStorageStatus {
    NotImplemented,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SidecarStatus {
    status: SidecarStatusKind,
    binding: SidecarBinding,
    session: SidecarSessionKind,
    secret_storage: SecretStorageStatus,
    service: String,
    version: String,
    api_version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SidecarErrorCode {
    SidecarUnauthorized,
    SidecarUnavailable,
    SidecarStartFailed,
    SidecarRequestFailed,
    SidecarTimeout,
    SidecarInvalidResponse,
    InternalError,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SidecarCommandError {
    code: SidecarErrorCode,
    message: &'static str,
    retryable: bool,
}

impl SidecarCommandError {
    fn new(code: SidecarErrorCode) -> Self {
        let (message, retryable) = match code {
            SidecarErrorCode::SidecarUnauthorized => {
                ("The local service could not authenticate.", false)
            }
            SidecarErrorCode::SidecarUnavailable => ("The local service is unavailable.", true),
            SidecarErrorCode::SidecarStartFailed => {
                ("The local service could not be started.", true)
            }
            SidecarErrorCode::SidecarRequestFailed => ("The local service request failed.", true),
            SidecarErrorCode::SidecarTimeout => {
                ("The local service did not respond in time.", true)
            }
            SidecarErrorCode::SidecarInvalidResponse => {
                ("The local service returned an invalid response.", false)
            }
            SidecarErrorCode::InternalError => ("An internal local service error occurred.", false),
        };
        Self {
            code,
            message,
            retryable,
        }
    }
}

pub(crate) struct SidecarRuntime {
    client: Option<SidecarClient>,
    startup_error: Option<SidecarCommandError>,
    child: Mutex<Option<Child>>,
}

impl SidecarRuntime {
    pub(crate) fn start() -> Self {
        let token = match SessionToken::generate() {
            Ok(token) => token,
            Err(error) => return Self::failed(error),
        };
        let session = SidecarSession::current(token);
        let client = match SidecarClient::new(session.clone()) {
            Ok(client) => client,
            Err(error) => return Self::failed(error),
        };

        match spawn_sidecar(&session) {
            Ok(child) => Self {
                client: Some(client),
                startup_error: None,
                child: Mutex::new(Some(child)),
            },
            Err(error) => Self {
                client: None,
                startup_error: Some(error),
                child: Mutex::new(None),
            },
        }
    }

    fn failed(error: SidecarCommandError) -> Self {
        Self {
            client: None,
            startup_error: Some(error),
            child: Mutex::new(None),
        }
    }

    async fn check_health(&self) -> Result<SidecarStatus, SidecarCommandError> {
        if let Some(error) = &self.startup_error {
            return Err(error.clone());
        }
        self.client
            .as_ref()
            .ok_or_else(|| SidecarCommandError::new(SidecarErrorCode::InternalError))?
            .check_health()
            .await
    }

    pub(crate) fn shutdown(&self) {
        let Ok(mut guard) = self.child.lock() else {
            return;
        };
        let Some(mut child) = guard.take() else {
            return;
        };
        if matches!(child.try_wait(), Ok(None)) {
            let _ = child.kill();
        }
        let _ = child.wait();
    }
}

impl Drop for SidecarRuntime {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn spawn_sidecar(session: &SidecarSession) -> Result<Child, SidecarCommandError> {
    let working_directory = sidecar_working_directory()
        .ok_or_else(|| SidecarCommandError::new(SidecarErrorCode::SidecarStartFailed))?;
    let mut command = sidecar_command(session, Some(&working_directory));
    let mut child = command
        .spawn()
        .map_err(|_| SidecarCommandError::new(SidecarErrorCode::SidecarStartFailed))?;
    wait_until_listening(&mut child)?;
    Ok(child)
}

fn sidecar_command(session: &SidecarSession, working_directory: Option<&Path>) -> Command {
    #[cfg(target_os = "windows")]
    let mut command = {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let mut command = Command::new("py");
        command.arg("-3.12");
        command.creation_flags(CREATE_NO_WINDOW);
        command
    };

    #[cfg(not(target_os = "windows"))]
    let mut command = Command::new("python3.12");

    command
        .args([
            "-m",
            "sidecar.meetinglens_sidecar.server",
            "--port",
            &SIDECAR_PORT.to_string(),
        ])
        .env(SIDECAR_TOKEN_ENV, session.token.expose())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(working_directory) = working_directory {
        command.current_dir(working_directory);
    }
    command
}

fn sidecar_working_directory() -> Option<PathBuf> {
    let current = std::env::current_dir().ok()?;
    [Some(current.as_path()), current.parent()]
        .into_iter()
        .flatten()
        .find(|candidate| {
            candidate
                .join("sidecar")
                .join("meetinglens_sidecar")
                .join("server.py")
                .is_file()
        })
        .map(Path::to_path_buf)
}

fn wait_until_listening(child: &mut Child) -> Result<(), SidecarCommandError> {
    let address = SocketAddrV4::new(Ipv4Addr::LOCALHOST, SIDECAR_PORT);
    let deadline = Instant::now() + Duration::from_secs(3);

    while Instant::now() < deadline {
        if child
            .try_wait()
            .map_err(|_| SidecarCommandError::new(SidecarErrorCode::SidecarStartFailed))?
            .is_some()
        {
            return Err(SidecarCommandError::new(
                SidecarErrorCode::SidecarStartFailed,
            ));
        }
        if TcpStream::connect_timeout(&address.into(), Duration::from_millis(50)).is_ok() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(25));
    }

    let _ = child.kill();
    let _ = child.wait();
    Err(SidecarCommandError::new(
        SidecarErrorCode::SidecarStartFailed,
    ))
}

#[tauri::command]
pub async fn check_sidecar_health(
    runtime: State<'_, SidecarRuntime>,
) -> Result<SidecarStatus, SidecarCommandError> {
    runtime.check_health().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;

    const TEST_TOKEN: &str = "synthetic-session-token-for-tests-only-0001";

    fn valid_health_body() -> &'static str {
        r#"{"status":"ok","service":"meetinglens-sidecar","version":"0.1.0","api_version":"v1"}"#
    }

    fn spawn_response_server(
        status: &str,
        body: &'static str,
        delay: Duration,
    ) -> (String, mpsc::Receiver<String>, thread::JoinHandle<()>) {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind test server");
        let address = listener.local_addr().expect("read test address");
        let (sender, receiver) = mpsc::channel();
        let status = status.to_owned();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept test request");
            let mut request = Vec::new();
            let mut buffer = [0_u8; 512];
            loop {
                let read = stream.read(&mut buffer).expect("read test request");
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..read]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            let _ = sender.send(String::from_utf8_lossy(&request).into_owned());
            thread::sleep(delay);
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        });
        (format!("http://{address}/health"), receiver, handle)
    }

    fn test_client(endpoint: String, timeout: Duration) -> SidecarClient {
        SidecarClient::with_timeouts(
            SidecarSession::for_test(SessionToken::for_test(TEST_TOKEN), endpoint),
            timeout,
            timeout,
        )
        .expect("build test client")
    }

    #[test]
    fn generated_token_uses_thirty_two_random_bytes() {
        let token = SessionToken::generate().expect("generate token");
        assert_eq!(token.expose().len(), 64);
        assert!(token.expose().bytes().all(|byte| byte.is_ascii_hexdigit()));
    }

    #[test]
    fn consecutive_sessions_use_different_tokens() {
        let first = SessionToken::generate().expect("generate first token");
        let second = SessionToken::generate().expect("generate second token");
        assert_ne!(first, second);
    }

    #[test]
    fn token_and_client_debug_output_are_redacted() {
        let session = SidecarSession::for_test(
            SessionToken::for_test(TEST_TOKEN),
            "http://127.0.0.1:1/health".to_owned(),
        );
        let client = SidecarClient::new(session.clone()).expect("build client");
        let output = format!("{session:?} {client:?}");
        assert!(!output.contains(TEST_TOKEN));
        assert!(!output.contains("127.0.0.1:1"));
    }

    #[test]
    fn process_arguments_do_not_contain_the_token() {
        let session = SidecarSession::current(SessionToken::for_test(TEST_TOKEN));
        let command = sidecar_command(&session, None);
        let arguments = command
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert!(
            arguments
                .iter()
                .all(|argument| !argument.contains(TEST_TOKEN))
        );
        assert!(command.get_envs().any(|(name, value)| {
            name == SIDECAR_TOKEN_ENV
                && value.is_some_and(|value| value.to_string_lossy() == TEST_TOKEN)
        }));
    }

    #[test]
    fn status_dto_contains_no_token_or_internal_address() {
        let status = SidecarStatus {
            status: SidecarStatusKind::Authenticated,
            binding: SidecarBinding::Loopback,
            session: SidecarSessionKind::Current,
            secret_storage: SecretStorageStatus::NotImplemented,
            service: EXPECTED_SERVICE.to_owned(),
            version: "0.1.0".to_owned(),
            api_version: EXPECTED_API_VERSION.to_owned(),
        };
        let serialized = serde_json::to_string(&status).expect("serialize status");
        assert!(!serialized.contains("token"));
        assert!(!serialized.contains("127.0.0.1"));
        assert!(!serialized.contains("8765"));
    }

    #[test]
    fn client_injects_the_bearer_header_centrally() {
        let (endpoint, receiver, handle) =
            spawn_response_server("200 OK", valid_health_body(), Duration::ZERO);
        let client = test_client(endpoint, Duration::from_secs(1));
        let status = tauri::async_runtime::block_on(client.check_health()).expect("health result");
        let request = receiver
            .recv()
            .expect("captured request")
            .to_ascii_lowercase();
        handle.join().expect("test server completed");

        assert_eq!(status.status, SidecarStatusKind::Authenticated);
        assert!(request.contains(&format!("authorization: bearer {}", TEST_TOKEN)));
    }

    #[test]
    fn unauthorized_response_maps_to_a_stable_error() {
        let private_body = r#"{"traceback":"private-path","header":"private-value"}"#;
        let (endpoint, _, handle) =
            spawn_response_server("401 Unauthorized", private_body, Duration::ZERO);
        let client = test_client(endpoint, Duration::from_secs(1));
        let error = tauri::async_runtime::block_on(client.check_health()).expect_err("reject 401");
        handle.join().expect("test server completed");

        assert_eq!(error.code, SidecarErrorCode::SidecarUnauthorized);
        let serialized = serde_json::to_string(&error).expect("serialize error");
        assert!(!serialized.contains("traceback"));
        assert!(!serialized.contains("private-value"));
        assert!(!serialized.contains(TEST_TOKEN));
    }

    #[test]
    fn timeout_maps_to_a_stable_retryable_error() {
        let (endpoint, _, handle) =
            spawn_response_server("200 OK", valid_health_body(), Duration::from_millis(150));
        let client = test_client(endpoint, Duration::from_millis(25));
        let error =
            tauri::async_runtime::block_on(client.check_health()).expect_err("timeout expected");
        handle.join().expect("test server completed");

        assert_eq!(error.code, SidecarErrorCode::SidecarTimeout);
        assert!(error.retryable);
    }

    #[test]
    fn invalid_payload_maps_without_returning_the_body() {
        let private_body = r#"{"status":"ok","private":"do-not-return"}"#;
        let (endpoint, _, handle) = spawn_response_server("200 OK", private_body, Duration::ZERO);
        let client = test_client(endpoint, Duration::from_secs(1));
        let error =
            tauri::async_runtime::block_on(client.check_health()).expect_err("invalid payload");
        handle.join().expect("test server completed");

        assert_eq!(error.code, SidecarErrorCode::SidecarInvalidResponse);
        assert!(!format!("{error:?}").contains("do-not-return"));
    }

    #[test]
    fn validate_health_accepts_only_the_expected_contract() {
        let health = SidecarHealth {
            status: "ok".to_owned(),
            service: EXPECTED_SERVICE.to_owned(),
            version: "0.1.0".to_owned(),
            api_version: EXPECTED_API_VERSION.to_owned(),
        };
        assert_eq!(validate_health(&health), Ok(()));
    }

    #[test]
    fn validate_health_rejects_an_unexpected_service() {
        let health = SidecarHealth {
            status: "ok".to_owned(),
            service: "another-process".to_owned(),
            version: "0.1.0".to_owned(),
            api_version: EXPECTED_API_VERSION.to_owned(),
        };
        assert_eq!(
            validate_health(&health),
            Err(SidecarCommandError::new(
                SidecarErrorCode::SidecarInvalidResponse
            ))
        );
    }

    #[test]
    fn stable_errors_never_include_caller_details() {
        for code in [
            SidecarErrorCode::SidecarUnauthorized,
            SidecarErrorCode::SidecarUnavailable,
            SidecarErrorCode::SidecarStartFailed,
            SidecarErrorCode::SidecarRequestFailed,
            SidecarErrorCode::SidecarTimeout,
            SidecarErrorCode::SidecarInvalidResponse,
            SidecarErrorCode::InternalError,
        ] {
            let serialized =
                serde_json::to_string(&SidecarCommandError::new(code)).expect("serialize error");
            assert!(!serialized.contains(TEST_TOKEN));
            assert!(!serialized.contains("Authorization"));
        }
    }

    #[test]
    fn missing_client_fails_closed() {
        let runtime = SidecarRuntime::failed(SidecarCommandError::new(
            SidecarErrorCode::SidecarStartFailed,
        ));
        let error = tauri::async_runtime::block_on(runtime.check_health())
            .expect_err("failed runtime must not call a service");
        assert_eq!(error.code, SidecarErrorCode::SidecarStartFailed);
    }

    #[test]
    fn a_previous_session_token_is_rejected_by_a_new_session() {
        let previous = "synthetic-previous-session-token-0001";
        let current = "synthetic-current-session-token-0002";
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind auth server");
        let address = listener.local_addr().expect("read auth server address");
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            let mut request = Vec::new();
            let mut buffer = [0_u8; 512];
            loop {
                let read = stream.read(&mut buffer).expect("read request");
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..read]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            let authorized = String::from_utf8_lossy(&request)
                .to_ascii_lowercase()
                .contains(&format!("authorization: bearer {current}"));
            let (status, body) = if authorized {
                ("200 OK", valid_health_body())
            } else {
                ("401 Unauthorized", r#"{"error":"sidecar_unauthorized"}"#)
            };
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        });

        let client = SidecarClient::with_timeouts(
            SidecarSession::for_test(
                SessionToken::for_test(previous),
                format!("http://{address}/health"),
            ),
            Duration::from_secs(1),
            Duration::from_secs(1),
        )
        .expect("build previous client");
        let error = tauri::async_runtime::block_on(client.check_health())
            .expect_err("previous token must fail");
        handle.join().expect("auth server completed");
        assert_eq!(error.code, SidecarErrorCode::SidecarUnauthorized);
    }
}
