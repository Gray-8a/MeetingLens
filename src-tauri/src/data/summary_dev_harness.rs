#![cfg(debug_assertions)]

use std::path::Path;

use rusqlite::Connection;
use uuid::Uuid;

use super::error::{DataError, DataResult};
use super::migration;
use super::paths::StoragePaths;

const SMOKE_ROOT_PREFIX: &str = "meetinglens-native-summary-smoke-";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SummarySmokeV3Seed {
    pub meeting_id: String,
    pub source_id: String,
    pub segment_id: String,
}

/// Creates a synthetic schema-v3 app-data directory for the native summary smoke test.
///
/// This helper is unavailable in release builds and accepts only a new direct child of the
/// operating-system temporary directory with the dedicated summary smoke prefix.
pub fn seed_summary_smoke_v3(app_data_root: impl AsRef<Path>) -> DataResult<SummarySmokeV3Seed> {
    let app_data_root = app_data_root.as_ref();
    let root_name = app_data_root
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(DataError::InvalidInput)?;
    let parent = app_data_root
        .parent()
        .ok_or(DataError::InvalidInput)?
        .canonicalize()
        .map_err(|_| DataError::InvalidInput)?;
    let temp_root = std::env::temp_dir()
        .canonicalize()
        .map_err(|_| DataError::InvalidInput)?;
    if !root_name.starts_with(SMOKE_ROOT_PREFIX) || parent != temp_root || app_data_root.exists() {
        return Err(DataError::InvalidInput);
    }

    let paths = StoragePaths::initialize(app_data_root)?;
    let mut connection = Connection::open(paths.database()).map_err(DataError::from)?;
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .map_err(DataError::from)?;
    migration::run_through_v3(&mut connection)?;

    let meeting_id = Uuid::new_v4().to_string();
    let source_id = Uuid::new_v4().to_string();
    let segment_id = Uuid::new_v4().to_string();
    connection
        .execute(
            "
            INSERT INTO meetings(
                id, title, status, primary_language, created_at, updated_at
            ) VALUES (
                ?1, 'Synthetic summary smoke upgrade', 'draft', 'en-AU',
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            )
            ",
            [&meeting_id],
        )
        .map_err(DataError::from)?;
    connection
        .execute(
            "
            INSERT INTO transcript_sources(
                id, meeting_id, source_type, display_name, content_sha256,
                language, parser_name, parser_version, imported_at, created_at, updated_at
            ) VALUES (
                ?1, ?2, 'pasted_text', 'Synthetic v3 source',
                'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
                'en-AU', 'plain_text', '1',
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            )
            ",
            rusqlite::params![&source_id, &meeting_id],
        )
        .map_err(DataError::from)?;
    connection
        .execute(
            "
            INSERT INTO transcript_segments(
                id, meeting_id, source_id, ordinal, speaker_label, original_text,
                language, is_final, revision, created_at
            ) VALUES (
                ?1, ?2, ?3, 0, 'Synthetic speaker', 'Synthetic legacy v3 transcript point',
                'en-AU', 1, 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            )
            ",
            rusqlite::params![&segment_id, &meeting_id, &source_id],
        )
        .map_err(DataError::from)?;

    let schema_version: i64 = connection
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .map_err(DataError::from)?;
    if schema_version != 3 {
        return Err(DataError::Database);
    }

    Ok(SummarySmokeV3Seed {
        meeting_id,
        source_id,
        segment_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_rejects_a_matching_name_outside_the_direct_temp_root() {
        let nested = std::env::temp_dir()
            .join(format!(
                "meetinglens-nested-summary-smoke-{}",
                Uuid::new_v4()
            ))
            .join(format!("{SMOKE_ROOT_PREFIX}{}", Uuid::new_v4()));
        assert_eq!(seed_summary_smoke_v3(nested), Err(DataError::InvalidInput));
    }
}
