use serde::{Deserialize, Serialize};

use super::error::{DataError, DataResult};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MeetingStatus {
    Archived,
    DeleteFailed,
    Deleting,
    Draft,
    Failed,
    Processing,
    Ready,
    Recording,
}

impl MeetingStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Archived => "archived",
            Self::DeleteFailed => "delete_failed",
            Self::Deleting => "deleting",
            Self::Draft => "draft",
            Self::Failed => "failed",
            Self::Processing => "processing",
            Self::Ready => "ready",
            Self::Recording => "recording",
        }
    }

    pub(crate) fn from_database(value: &str) -> DataResult<Self> {
        match value {
            "archived" => Ok(Self::Archived),
            "delete_failed" => Ok(Self::DeleteFailed),
            "deleting" => Ok(Self::Deleting),
            "draft" => Ok(Self::Draft),
            "failed" => Ok(Self::Failed),
            "processing" => Ok(Self::Processing),
            "ready" => Ok(Self::Ready),
            "recording" => Ok(Self::Recording),
            _ => Err(DataError::Database),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Meeting {
    pub id: String,
    pub title: String,
    pub status: MeetingStatus,
    pub primary_language: String,
    pub selected_window_json: Option<String>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deletion_error_code: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct CreateMeeting {
    pub title: String,
    pub primary_language: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct UpdateMeeting {
    pub title: Option<String>,
    pub primary_language: Option<String>,
}
