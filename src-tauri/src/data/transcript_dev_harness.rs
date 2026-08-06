#![cfg(debug_assertions)]

use std::path::Path;

use rusqlite::Connection;
use uuid::Uuid;

use super::error::{DataError, DataResult};
use super::migration;
use super::paths::StoragePaths;

const SMOKE_ROOT_PREFIX: &str = "meetinglens-native-transcript-smoke-";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TranscriptSmokeV2Seed {
    pub meeting_id: String,
}

/// Creates a synthetic schema-v2 app-data directory for the native transcript smoke test.
///
/// This helper is unavailable in release builds and refuses roots outside the dedicated
/// smoke-test naming convention. It never accepts transcript content or an existing database.
pub fn seed_transcript_smoke_v2(
    app_data_root: impl AsRef<Path>,
) -> DataResult<TranscriptSmokeV2Seed> {
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
    migration::run_through_v2(&mut connection)?;

    let meeting_id = Uuid::new_v4().to_string();
    connection
        .execute(
            "
            INSERT INTO meetings(
                id, title, status, primary_language, created_at, updated_at
            ) VALUES (
                ?1, 'Synthetic transcript smoke upgrade', 'draft', 'en-AU',
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            )
            ",
            [&meeting_id],
        )
        .map_err(DataError::from)?;

    let schema_version: i64 = connection
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .map_err(DataError::from)?;
    if schema_version != 2 {
        return Err(DataError::Database);
    }

    Ok(TranscriptSmokeV2Seed { meeting_id })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_rejects_a_matching_name_outside_the_direct_temp_root() {
        let nested = std::env::temp_dir()
            .join(format!("meetinglens-nested-smoke-{}", Uuid::new_v4()))
            .join(format!("{SMOKE_ROOT_PREFIX}{}", Uuid::new_v4()));
        assert_eq!(
            seed_transcript_smoke_v2(nested),
            Err(DataError::InvalidInput)
        );
    }
}
