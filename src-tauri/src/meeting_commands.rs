use serde::{Deserialize, Serialize};
use tauri::State;

use crate::data::{
    CreateMeeting, DataError, Meeting, MeetingService, MeetingStatus, UpdateMeeting,
};

#[cfg(test)]
const MEETING_COMMAND_NAMES: &[&str] = &[
    "create_meeting",
    "get_meeting",
    "list_meetings",
    "update_meeting",
    "start_meeting",
    "end_meeting",
    "delete_meeting",
];

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CreateMeetingRequest {
    pub title: String,
    pub primary_language: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMeetingRequest {
    pub id: String,
    pub title: Option<String>,
    pub primary_language: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct MeetingIdRequest {
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingResponse {
    pub id: String,
    pub title: String,
    pub status: MeetingStatus,
    pub primary_language: String,
    pub selected_window_json: Option<String>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<Meeting> for MeetingResponse {
    fn from(meeting: Meeting) -> Self {
        Self {
            id: meeting.id,
            title: meeting.title,
            status: meeting.status,
            primary_language: meeting.primary_language,
            selected_window_json: meeting.selected_window_json,
            started_at: meeting.started_at,
            ended_at: meeting.ended_at,
            created_at: meeting.created_at,
            updated_at: meeting.updated_at,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListMeetingsResponse {
    pub meetings: Vec<MeetingResponse>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteMeetingResponse {
    pub id: String,
    pub deleted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingCommandError {
    pub code: &'static str,
    pub message: &'static str,
    pub retryable: bool,
}

impl From<DataError> for MeetingCommandError {
    fn from(error: DataError) -> Self {
        Self {
            code: error.code(),
            message: error.safe_message(),
            retryable: error.retryable(),
        }
    }
}

type CommandResult<T> = Result<T, MeetingCommandError>;

#[tauri::command]
pub fn create_meeting(
    request: CreateMeetingRequest,
    service: State<'_, MeetingService>,
) -> CommandResult<MeetingResponse> {
    create_meeting_with_service(&service, request)
}

#[tauri::command]
pub fn get_meeting(
    request: MeetingIdRequest,
    service: State<'_, MeetingService>,
) -> CommandResult<MeetingResponse> {
    get_meeting_with_service(&service, request)
}

#[tauri::command]
pub fn list_meetings(service: State<'_, MeetingService>) -> CommandResult<ListMeetingsResponse> {
    list_meetings_with_service(&service)
}

#[tauri::command]
pub fn update_meeting(
    request: UpdateMeetingRequest,
    service: State<'_, MeetingService>,
) -> CommandResult<MeetingResponse> {
    update_meeting_with_service(&service, request)
}

#[tauri::command]
pub fn start_meeting(
    request: MeetingIdRequest,
    service: State<'_, MeetingService>,
) -> CommandResult<MeetingResponse> {
    start_meeting_with_service(&service, request)
}

#[tauri::command]
pub fn end_meeting(
    request: MeetingIdRequest,
    service: State<'_, MeetingService>,
) -> CommandResult<MeetingResponse> {
    end_meeting_with_service(&service, request)
}

#[tauri::command]
pub fn delete_meeting(
    request: MeetingIdRequest,
    service: State<'_, MeetingService>,
) -> CommandResult<DeleteMeetingResponse> {
    delete_meeting_with_service(&service, request)
}

fn create_meeting_with_service(
    service: &MeetingService,
    request: CreateMeetingRequest,
) -> CommandResult<MeetingResponse> {
    service
        .create(CreateMeeting {
            title: request.title,
            primary_language: request.primary_language,
        })
        .map(MeetingResponse::from)
        .map_err(MeetingCommandError::from)
}

fn get_meeting_with_service(
    service: &MeetingService,
    request: MeetingIdRequest,
) -> CommandResult<MeetingResponse> {
    service
        .get(&request.id)
        .map(MeetingResponse::from)
        .map_err(MeetingCommandError::from)
}

fn list_meetings_with_service(service: &MeetingService) -> CommandResult<ListMeetingsResponse> {
    service
        .list()
        .map(|meetings| ListMeetingsResponse {
            meetings: meetings.into_iter().map(MeetingResponse::from).collect(),
        })
        .map_err(MeetingCommandError::from)
}

fn update_meeting_with_service(
    service: &MeetingService,
    request: UpdateMeetingRequest,
) -> CommandResult<MeetingResponse> {
    service
        .update(
            &request.id,
            UpdateMeeting {
                title: request.title,
                primary_language: request.primary_language,
            },
        )
        .map(MeetingResponse::from)
        .map_err(MeetingCommandError::from)
}

fn start_meeting_with_service(
    service: &MeetingService,
    request: MeetingIdRequest,
) -> CommandResult<MeetingResponse> {
    service
        .start(&request.id)
        .map(MeetingResponse::from)
        .map_err(MeetingCommandError::from)
}

fn end_meeting_with_service(
    service: &MeetingService,
    request: MeetingIdRequest,
) -> CommandResult<MeetingResponse> {
    service
        .end(&request.id)
        .map(MeetingResponse::from)
        .map_err(MeetingCommandError::from)
}

fn delete_meeting_with_service(
    service: &MeetingService,
    request: MeetingIdRequest,
) -> CommandResult<DeleteMeetingResponse> {
    service
        .delete(&request.id)
        .map(|()| DeleteMeetingResponse {
            id: request.id,
            deleted: true,
        })
        .map_err(MeetingCommandError::from)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::json;
    use uuid::Uuid;

    use super::*;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!("meetinglens-command-api-{}", Uuid::new_v4())))
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn service(directory: &TestDirectory) -> MeetingService {
        MeetingService::initialize(&directory.0).expect("service should initialize")
    }

    fn create_request(title: &str) -> CreateMeetingRequest {
        CreateMeetingRequest {
            title: title.to_string(),
            primary_language: "en-AU".to_string(),
        }
    }

    #[test]
    fn commands_execute_the_complete_lifecycle_through_the_service() {
        let directory = TestDirectory::new();
        let service = service(&directory);
        let created =
            create_meeting_with_service(&service, create_request("Synthetic command lifecycle"))
                .expect("create should succeed");

        assert_eq!(created.status, MeetingStatus::Draft);
        assert_eq!(
            get_meeting_with_service(
                &service,
                MeetingIdRequest {
                    id: created.id.clone()
                }
            )
            .expect("get should succeed"),
            created
        );
        assert_eq!(
            list_meetings_with_service(&service)
                .expect("list should succeed")
                .meetings
                .len(),
            1
        );

        let updated = update_meeting_with_service(
            &service,
            UpdateMeetingRequest {
                id: created.id.clone(),
                title: Some("Synthetic updated command lifecycle".to_string()),
                primary_language: Some("zh-CN".to_string()),
            },
        )
        .expect("update should succeed");
        assert_eq!(updated.title, "Synthetic updated command lifecycle");
        assert_eq!(updated.primary_language, "zh-CN");
        assert_eq!(updated.status, MeetingStatus::Draft);

        let recording = start_meeting_with_service(
            &service,
            MeetingIdRequest {
                id: created.id.clone(),
            },
        )
        .expect("start should succeed");
        assert_eq!(recording.status, MeetingStatus::Recording);
        assert!(recording.started_at.is_some());
        assert_eq!(
            delete_meeting_with_service(
                &service,
                MeetingIdRequest {
                    id: created.id.clone()
                }
            )
            .expect_err("recording deletion should fail")
            .code,
            "meeting_invalid_state"
        );

        let ready = end_meeting_with_service(
            &service,
            MeetingIdRequest {
                id: created.id.clone(),
            },
        )
        .expect("end should succeed");
        assert_eq!(ready.status, MeetingStatus::Ready);
        assert!(ready.ended_at >= ready.started_at);

        let deleted = delete_meeting_with_service(
            &service,
            MeetingIdRequest {
                id: created.id.clone(),
            },
        )
        .expect("ready deletion should succeed");
        assert_eq!(
            deleted,
            DeleteMeetingResponse {
                id: created.id.clone(),
                deleted: true
            }
        );
        assert_eq!(
            get_meeting_with_service(&service, MeetingIdRequest { id: created.id })
                .expect_err("deleted meeting should not exist")
                .code,
            "meeting_not_found"
        );
    }

    #[test]
    fn commands_return_stable_validation_and_identifier_errors() {
        let directory = TestDirectory::new();
        let service = service(&directory);

        assert_eq!(
            create_meeting_with_service(&service, create_request(""))
                .expect_err("empty title should fail")
                .code,
            "validation_error"
        );
        assert_eq!(
            create_meeting_with_service(&service, create_request("   "))
                .expect_err("blank title should fail")
                .code,
            "validation_error"
        );
        assert_eq!(
            get_meeting_with_service(
                &service,
                MeetingIdRequest {
                    id: "not-a-uuid".to_string()
                }
            )
            .expect_err("invalid id should fail")
            .code,
            "invalid_meeting_id"
        );
        assert_eq!(
            get_meeting_with_service(
                &service,
                MeetingIdRequest {
                    id: Uuid::new_v4().to_string()
                }
            )
            .expect_err("missing meeting should fail")
            .code,
            "meeting_not_found"
        );
        assert_eq!(
            delete_meeting_with_service(
                &service,
                MeetingIdRequest {
                    id: Uuid::new_v4().to_string()
                }
            )
            .expect_err("deleting a missing meeting should fail")
            .code,
            "meeting_not_found"
        );
    }

    #[test]
    fn update_dto_cannot_modify_server_owned_lifecycle_fields() {
        let request: UpdateMeetingRequest = serde_json::from_value(json!({
            "id": Uuid::new_v4().to_string(),
            "status": "ready",
            "startedAt": "2026-01-01T00:00:00.000Z",
            "endedAt": "2026-01-01T00:00:01.000Z"
        }))
        .expect("unknown lifecycle fields should not enter the DTO");

        assert_eq!(request.title, None);
        assert_eq!(request.primary_language, None);
    }

    #[test]
    fn response_dtos_use_camel_case_and_null_times() {
        let directory = TestDirectory::new();
        let service = service(&directory);
        let meeting = create_meeting_with_service(&service, create_request("Synthetic DTO"))
            .expect("create should succeed");
        let value = serde_json::to_value(meeting).expect("response should serialize");

        assert!(value.get("primaryLanguage").is_some());
        assert!(value.get("selectedWindowJson").is_some());
        assert_eq!(value.get("startedAt"), Some(&serde_json::Value::Null));
        assert_eq!(value.get("endedAt"), Some(&serde_json::Value::Null));
        assert!(value.get("createdAt").is_some());
        assert!(value.get("updatedAt").is_some());
        assert!(value.get("created_at").is_none());
    }

    #[test]
    fn database_errors_are_converted_without_internal_details() {
        let error = MeetingCommandError::from(DataError::Database);
        let serialized = serde_json::to_string(&error).expect("error should serialize");
        let lowercase = serialized.to_ascii_lowercase();

        assert_eq!(error.code, "persistence_error");
        assert!(error.retryable);
        for forbidden in [
            "select ",
            "insert ",
            "update ",
            "delete ",
            "sqlite",
            "rusqlite",
            "backtrace",
            "c:\\\\users\\\\",
            "crate::",
        ] {
            assert!(!lowercase.contains(forbidden));
        }
    }

    #[test]
    fn every_data_error_maps_to_an_approved_command_code() {
        let mappings = [
            (DataError::InvalidInput, "validation_error"),
            (DataError::InvalidId, "invalid_meeting_id"),
            (DataError::NotFound, "meeting_not_found"),
            (DataError::InvalidState, "meeting_invalid_state"),
            (DataError::MeetingDeleteFailed, "meeting_delete_failed"),
            (DataError::MeetingDeletePending, "meeting_delete_pending"),
            (DataError::StorageUnavailable, "database_unavailable"),
            (DataError::Database, "persistence_error"),
            (DataError::MigrationConflict, "persistence_error"),
            (DataError::UnsafePath, "internal_error"),
        ];

        for (error, expected_code) in mappings {
            let command_error = MeetingCommandError::from(error);
            assert_eq!(command_error.code, expected_code);
            assert!(!command_error.message.is_empty());
        }
    }

    #[test]
    fn command_layer_contains_no_sql_and_all_commands_are_registered() {
        let source = include_str!("meeting_commands.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("production source should exist")
            .to_ascii_lowercase();
        assert!(!production.contains("rusqlite"));
        assert!(!production.contains("select "));
        assert!(!production.contains("insert "));
        assert!(!production.contains("update meetings"));
        assert!(!production.contains("delete from"));
        assert!(!production.contains("std::fs"));
        assert!(!production.contains("remove_file"));
        assert!(!production.contains("remove_dir"));
        assert!(!production.contains("fs::rename"));

        let app_builder = include_str!("lib.rs");
        for command in MEETING_COMMAND_NAMES {
            assert!(app_builder.contains(&format!("meeting_commands::{command}")));
        }
    }
}
