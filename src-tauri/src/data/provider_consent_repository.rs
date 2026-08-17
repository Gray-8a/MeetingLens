use rusqlite::{Connection, OptionalExtension, params};

use super::error::{DataError, DataResult};
use super::provider_consent_model::{ProviderConsentPolicy, ProviderConsentPreference};

pub(crate) fn get(
    connection: &Connection,
    provider: &str,
) -> DataResult<Option<ProviderConsentPreference>> {
    connection
        .query_row(
            "
            SELECT provider, policy_version, policy_fingerprint, consented_at, updated_at
            FROM provider_consent_preferences
            WHERE provider = ?1
            ",
            [provider],
            |row| {
                Ok(ProviderConsentPreference {
                    provider: row.get(0)?,
                    policy_version: row.get(1)?,
                    policy_fingerprint: row.get(2)?,
                    consented_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            },
        )
        .optional()
        .map_err(DataError::from)
}

pub(crate) fn upsert(
    connection: &Connection,
    policy: &ProviderConsentPolicy<'_>,
) -> DataResult<ProviderConsentPreference> {
    let fingerprint = policy.fingerprint();
    connection
        .execute(
            "
            INSERT INTO provider_consent_preferences(
                provider, policy_version, policy_fingerprint, consented_at, updated_at
            ) VALUES (
                ?1, ?2, ?3,
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            )
            ON CONFLICT(provider) DO UPDATE SET
                policy_version = excluded.policy_version,
                policy_fingerprint = excluded.policy_fingerprint,
                consented_at = excluded.consented_at,
                updated_at = excluded.updated_at
            ",
            params![policy.provider, policy.policy_version, fingerprint],
        )
        .map_err(DataError::from)?;
    get(connection, policy.provider)?.ok_or(DataError::Database)
}

pub(crate) fn delete(connection: &Connection, provider: &str) -> DataResult<bool> {
    connection
        .execute(
            "DELETE FROM provider_consent_preferences WHERE provider = ?1",
            [provider],
        )
        .map(|affected| affected > 0)
        .map_err(DataError::from)
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::*;
    use crate::data::migration;
    use crate::data::provider_consent_model::ProviderConsentPolicy;

    #[test]
    fn remembered_preference_round_trips_updates_and_deletes_without_private_data() {
        let mut connection = Connection::open_in_memory().expect("database should open");
        migration::run(&mut connection).expect("schema should apply");
        let policy = ProviderConsentPolicy::deepseek_transcript();

        assert_eq!(get(&connection, policy.provider).unwrap(), None);
        let first = upsert(&connection, &policy).expect("preference should persist");
        assert!(first.is_valid_for(&policy));
        assert!(first.consented_at.ends_with('Z'));
        assert_eq!(first.consented_at, first.updated_at);

        let changed = ProviderConsentPolicy {
            policy_version: "deepseek-transcript-external-v2",
            ..policy
        };
        let updated = upsert(&connection, &changed).expect("preference should update");
        assert!(updated.is_valid_for(&changed));
        assert!(!updated.is_valid_for(&policy));

        let row_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM provider_consent_preferences",
                [],
                |row| row.get(0),
            )
            .expect("row count should read");
        assert_eq!(row_count, 1);
        assert!(delete(&connection, policy.provider).unwrap());
        assert!(!delete(&connection, policy.provider).unwrap());
        assert_eq!(get(&connection, policy.provider).unwrap(), None);
    }
}
