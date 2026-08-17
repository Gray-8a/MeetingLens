use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rusqlite::{Transaction, TransactionBehavior};
use uuid::Uuid;

use super::database::Database;
use super::error::{DataError, DataResult};
use super::provider_consent_model::{
    DEEPSEEK_CONSENT_PROVIDER, ProviderConsentPolicy, ProviderConsentPreference,
};
use super::provider_consent_repository;

const ONE_TIME_CONSENT_TTL: Duration = Duration::from_secs(10 * 60);

#[derive(Clone)]
pub(crate) struct ProviderConsentService {
    database: Database,
    one_time: Arc<Mutex<HashMap<String, OneTimeConsent>>>,
}

impl fmt::Debug for ProviderConsentService {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderConsentService")
            .field("database", &"[CONFIGURED]")
            .field("one_time", &"[IN_MEMORY]")
            .finish()
    }
}

struct OneTimeConsent {
    policy_fingerprint: String,
    expires_at: Instant,
}

pub(crate) enum ProviderConsentAuthorization {
    Remembered,
    OneTime(OneTimeConsentLease),
}

impl ProviderConsentAuthorization {
    pub(crate) fn commit(mut self) {
        if let Self::OneTime(lease) = &mut self {
            lease.committed = true;
        }
    }

    #[cfg(test)]
    fn is_one_time(&self) -> bool {
        matches!(self, Self::OneTime(_))
    }
}

pub(crate) struct OneTimeConsentLease {
    one_time: Arc<Mutex<HashMap<String, OneTimeConsent>>>,
    summary_id: String,
    consent: Option<OneTimeConsent>,
    committed: bool,
}

impl Drop for OneTimeConsentLease {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        let Some(consent) = self.consent.take() else {
            return;
        };
        if consent.expires_at <= Instant::now() {
            return;
        }
        self.one_time
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entry(self.summary_id.clone())
            .or_insert(consent);
    }
}

impl ProviderConsentService {
    pub(crate) fn new(database: Database) -> Self {
        Self {
            database,
            one_time: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub(crate) fn remember_with_transaction(
        &self,
        transaction: &Transaction<'_>,
        policy: &ProviderConsentPolicy<'_>,
    ) -> DataResult<ProviderConsentPreference> {
        validate_policy(policy)?;
        provider_consent_repository::upsert(transaction, policy)
    }

    pub(crate) fn remembered_with_transaction(
        &self,
        transaction: &Transaction<'_>,
        policy: &ProviderConsentPolicy<'_>,
    ) -> DataResult<Option<ProviderConsentPreference>> {
        validate_policy(policy)?;
        Ok(
            provider_consent_repository::get(transaction, policy.provider)?
                .filter(|preference| preference.is_valid_for(policy)),
        )
    }

    pub(crate) fn remembered(
        &self,
        policy: &ProviderConsentPolicy<'_>,
    ) -> DataResult<Option<ProviderConsentPreference>> {
        validate_policy(policy)?;
        let connection = self.database.connect()?;
        Ok(
            provider_consent_repository::get(&connection, policy.provider)?
                .filter(|preference| preference.is_valid_for(policy)),
        )
    }

    pub(crate) fn grant_one_time_with_transaction(
        &self,
        transaction: &Transaction<'_>,
        summary_id: &str,
        policy: &ProviderConsentPolicy<'_>,
    ) -> DataResult<()> {
        self.grant_one_time_with_ttl(transaction, summary_id, policy, ONE_TIME_CONSENT_TTL)
    }

    pub(crate) fn validate_and_consume_authorization(
        &self,
        transaction: &Transaction<'_>,
        summary_id: &str,
        policy: &ProviderConsentPolicy<'_>,
    ) -> DataResult<Option<ProviderConsentAuthorization>> {
        validate_summary_id(summary_id)?;
        validate_policy(policy)?;
        if self
            .remembered_with_transaction(transaction, policy)?
            .is_some()
        {
            return Ok(Some(ProviderConsentAuthorization::Remembered));
        }

        let mut one_time = self
            .one_time
            .lock()
            .map_err(|_| DataError::StorageUnavailable)?;
        let Some(consent) = one_time.remove(summary_id) else {
            return Ok(None);
        };
        if consent.expires_at <= Instant::now()
            || consent.policy_fingerprint != policy.fingerprint()
        {
            return Ok(None);
        }
        Ok(Some(ProviderConsentAuthorization::OneTime(
            OneTimeConsentLease {
                one_time: Arc::clone(&self.one_time),
                summary_id: summary_id.to_string(),
                consent: Some(consent),
                committed: false,
            },
        )))
    }

    pub(crate) fn one_time_valid(
        &self,
        summary_id: &str,
        policy: &ProviderConsentPolicy<'_>,
    ) -> DataResult<bool> {
        validate_summary_id(summary_id)?;
        validate_policy(policy)?;
        let expected_fingerprint = policy.fingerprint();
        let now = Instant::now();
        let mut one_time = self
            .one_time
            .lock()
            .map_err(|_| DataError::StorageUnavailable)?;
        let valid = one_time.get(summary_id).is_some_and(|consent| {
            consent.expires_at > now && consent.policy_fingerprint == expected_fingerprint
        });
        if !valid {
            one_time.remove(summary_id);
        }
        Ok(valid)
    }

    pub(crate) fn revoke(&self) -> DataResult<bool> {
        let guard = self.database.mutation_guard()?;
        let mut connection = self.database.connect()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(DataError::from)?;
        let deleted = provider_consent_repository::delete(&transaction, DEEPSEEK_CONSENT_PROVIDER)?;
        transaction.commit().map_err(DataError::from)?;
        let _ = self.clear_one_time()?;
        drop(connection);
        drop(guard);
        Ok(deleted)
    }

    pub(crate) fn clear_one_time(&self) -> DataResult<usize> {
        let mut one_time = self
            .one_time
            .lock()
            .map_err(|_| DataError::StorageUnavailable)?;
        let cleared = one_time.len();
        one_time.clear();
        Ok(cleared)
    }

    pub(crate) fn clear_one_time_for_summary(&self, summary_id: &str) -> DataResult<bool> {
        validate_summary_id(summary_id)?;
        self.one_time
            .lock()
            .map_err(|_| DataError::StorageUnavailable)
            .map(|mut one_time| one_time.remove(summary_id).is_some())
    }

    fn grant_one_time_with_ttl(
        &self,
        _transaction: &Transaction<'_>,
        summary_id: &str,
        policy: &ProviderConsentPolicy<'_>,
        ttl: Duration,
    ) -> DataResult<()> {
        validate_summary_id(summary_id)?;
        validate_policy(policy)?;
        let expires_at = Instant::now()
            .checked_add(ttl)
            .ok_or(DataError::InvalidInput)?;
        let mut one_time = self
            .one_time
            .lock()
            .map_err(|_| DataError::StorageUnavailable)?;
        let now = Instant::now();
        one_time.retain(|_, consent| consent.expires_at > now);
        one_time.insert(
            summary_id.to_string(),
            OneTimeConsent {
                policy_fingerprint: policy.fingerprint(),
                expires_at,
            },
        );
        Ok(())
    }
}

fn validate_policy(policy: &ProviderConsentPolicy<'_>) -> DataResult<()> {
    if policy.is_well_formed() {
        Ok(())
    } else {
        Err(DataError::InvalidInput)
    }
}

fn validate_summary_id(summary_id: &str) -> DataResult<()> {
    let parsed = Uuid::parse_str(summary_id).map_err(|_| DataError::InvalidId)?;
    if parsed.to_string() == summary_id {
        Ok(())
    } else {
        Err(DataError::InvalidId)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::data::provider_consent_model::ProviderConsentPolicy;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            Self(
                std::env::temp_dir()
                    .join(format!("meetinglens-provider-consent-{}", Uuid::new_v4())),
            )
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn transaction<T>(
        service: &ProviderConsentService,
        operation: impl FnOnce(&Transaction<'_>) -> DataResult<T>,
    ) -> DataResult<T> {
        let _guard = service.database.mutation_guard()?;
        let mut connection = service.database.connect()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(DataError::from)?;
        let result = operation(&transaction)?;
        transaction.commit().map_err(DataError::from)?;
        Ok(result)
    }

    #[test]
    fn remembered_consent_persists_across_service_instances_and_policy_changes_invalidate_it() {
        let directory = TestDirectory::new();
        let policy = ProviderConsentPolicy::deepseek_transcript();
        {
            let service = ProviderConsentService::new(
                Database::initialize(&directory.0).expect("database should initialize"),
            );
            let preference = transaction(&service, |transaction| {
                service.remember_with_transaction(transaction, &policy)
            })
            .expect("remembered consent should persist");
            assert!(preference.is_valid_for(&policy));
        }

        let reopened = ProviderConsentService::new(
            Database::initialize(&directory.0).expect("database should reopen"),
        );
        assert!(reopened.remembered(&policy).unwrap().is_some());
        let changed_model = ProviderConsentPolicy {
            model: "changed-model",
            ..policy
        };
        assert_eq!(reopened.remembered(&changed_model).unwrap(), None);
    }

    #[test]
    fn one_time_consent_is_run_bound_single_use_expiring_and_per_instance() {
        let directory = TestDirectory::new();
        let database = Database::initialize(&directory.0).expect("database should initialize");
        let service = ProviderConsentService::new(database.clone());
        let restarted = ProviderConsentService::new(database);
        let policy = ProviderConsentPolicy::deepseek_transcript();
        let summary_id = Uuid::new_v4().to_string();
        let other_summary_id = Uuid::new_v4().to_string();

        transaction(&service, |transaction| {
            service.grant_one_time_with_transaction(transaction, &summary_id, &policy)
        })
        .expect("one-time consent should be granted");
        assert!(service.one_time_valid(&summary_id, &policy).unwrap());
        assert!(
            service.one_time_valid(&summary_id, &policy).unwrap(),
            "read-only validation must not consume one-time consent"
        );
        assert!(!restarted.one_time_valid(&summary_id, &policy).unwrap());
        assert!(
            transaction(&restarted, |transaction| {
                restarted.validate_and_consume_authorization(transaction, &summary_id, &policy)
            })
            .unwrap()
            .is_none(),
            "a new service instance must not inherit one-time consent"
        );
        assert!(
            transaction(&service, |transaction| {
                service.validate_and_consume_authorization(transaction, &other_summary_id, &policy)
            })
            .unwrap()
            .is_none()
        );
        assert!(
            service.one_time_valid(&summary_id, &policy).unwrap(),
            "checking another run must not consume this run's authorization"
        );
        let authorization = transaction(&service, |transaction| {
            service.validate_and_consume_authorization(transaction, &summary_id, &policy)
        })
        .unwrap()
        .expect("one-time authorization should validate");
        assert!(authorization.is_one_time());
        drop(authorization);
        assert!(
            service.one_time_valid(&summary_id, &policy).unwrap(),
            "dropping an uncommitted authorization must restore it"
        );
        let authorization = transaction(&service, |transaction| {
            service.validate_and_consume_authorization(transaction, &summary_id, &policy)
        })
        .unwrap()
        .expect("restored one-time authorization should validate");
        authorization.commit();
        assert!(!service.one_time_valid(&summary_id, &policy).unwrap());
        assert!(
            transaction(&service, |transaction| {
                service.validate_and_consume_authorization(transaction, &summary_id, &policy)
            })
            .unwrap()
            .is_none()
        );

        let changed_policy_summary_id = Uuid::new_v4().to_string();
        transaction(&service, |transaction| {
            service.grant_one_time_with_transaction(
                transaction,
                &changed_policy_summary_id,
                &policy,
            )
        })
        .expect("one-time consent should bind the policy fingerprint");
        let changed_policy = ProviderConsentPolicy {
            policy_version: "deepseek-transcript-external-v2",
            ..policy
        };
        assert!(
            transaction(&service, |transaction| {
                service.validate_and_consume_authorization(
                    transaction,
                    &changed_policy_summary_id,
                    &changed_policy,
                )
            })
            .unwrap()
            .is_none()
        );
        assert!(
            transaction(&service, |transaction| {
                service.validate_and_consume_authorization(
                    transaction,
                    &changed_policy_summary_id,
                    &policy,
                )
            })
            .unwrap()
            .is_none(),
            "a mismatched one-time authorization must be consumed fail closed"
        );

        transaction(&service, |transaction| {
            service.grant_one_time_with_ttl(transaction, &summary_id, &policy, Duration::ZERO)
        })
        .expect("expired one-time consent should seed");
        assert!(!service.one_time_valid(&summary_id, &policy).unwrap());
        assert!(
            transaction(&service, |transaction| {
                service.validate_and_consume_authorization(transaction, &summary_id, &policy)
            })
            .unwrap()
            .is_none()
        );
    }

    #[test]
    fn revoke_removes_remembered_and_all_unconsumed_one_time_consent() {
        let directory = TestDirectory::new();
        let service = ProviderConsentService::new(
            Database::initialize(&directory.0).expect("database should initialize"),
        );
        let policy = ProviderConsentPolicy::deepseek_transcript();
        let first = Uuid::new_v4().to_string();
        let second = Uuid::new_v4().to_string();
        transaction(&service, |transaction| {
            service.remember_with_transaction(transaction, &policy)?;
            service.grant_one_time_with_transaction(transaction, &first, &policy)?;
            service.grant_one_time_with_transaction(transaction, &second, &policy)
        })
        .expect("consent state should seed");

        assert!(service.revoke().expect("revoke should succeed"));
        assert!(service.remembered(&policy).unwrap().is_none());
        assert_eq!(service.clear_one_time().unwrap(), 0);
        assert!(
            !service
                .revoke()
                .expect("repeated revoke should be idempotent")
        );
    }

    #[test]
    fn one_time_clear_for_summary_is_bounded_and_idempotent() {
        let directory = TestDirectory::new();
        let service = ProviderConsentService::new(
            Database::initialize(&directory.0).expect("database should initialize"),
        );
        let policy = ProviderConsentPolicy::deepseek_transcript();
        let summary_id = Uuid::new_v4().to_string();
        transaction(&service, |transaction| {
            service.grant_one_time_with_transaction(transaction, &summary_id, &policy)
        })
        .unwrap();

        assert!(service.clear_one_time_for_summary(&summary_id).unwrap());
        assert!(!service.clear_one_time_for_summary(&summary_id).unwrap());
        assert_eq!(
            service.clear_one_time_for_summary("not-a-uuid"),
            Err(DataError::InvalidId)
        );
    }
}
