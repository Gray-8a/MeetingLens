use std::fmt::Write as _;

use sha2::{Digest, Sha256};

pub(crate) use super::summary_provider::{
    DEEPSEEK_CONSENT_DATA_CATEGORIES, DEEPSEEK_CONSENT_POLICY_VERSION,
    DEEPSEEK_ENDPOINT_HOST as DEEPSEEK_CONSENT_ENDPOINT_HOST,
    DEEPSEEK_SUMMARY_MODEL as DEEPSEEK_CONSENT_MODEL,
    DEEPSEEK_SUMMARY_PROVIDER as DEEPSEEK_CONSENT_PROVIDER,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProviderConsentPolicy<'a> {
    pub(crate) provider: &'a str,
    pub(crate) endpoint_host: &'a str,
    pub(crate) model: &'a str,
    pub(crate) policy_version: &'a str,
    pub(crate) data_categories: &'a [&'a str],
}

impl ProviderConsentPolicy<'static> {
    pub(crate) const fn deepseek_transcript() -> Self {
        Self {
            provider: DEEPSEEK_CONSENT_PROVIDER,
            endpoint_host: DEEPSEEK_CONSENT_ENDPOINT_HOST,
            model: DEEPSEEK_CONSENT_MODEL,
            policy_version: DEEPSEEK_CONSENT_POLICY_VERSION,
            data_categories: &DEEPSEEK_CONSENT_DATA_CATEGORIES,
        }
    }
}

impl ProviderConsentPolicy<'_> {
    pub(crate) fn fingerprint(&self) -> String {
        let mut hasher = Sha256::new();
        update_fingerprint_field(&mut hasher, b"meetinglens-provider-consent-v1");
        update_fingerprint_field(&mut hasher, self.provider.as_bytes());
        update_fingerprint_field(&mut hasher, self.endpoint_host.as_bytes());
        update_fingerprint_field(&mut hasher, self.model.as_bytes());
        update_fingerprint_field(&mut hasher, self.policy_version.as_bytes());
        update_fingerprint_field(
            &mut hasher,
            &u64::try_from(self.data_categories.len())
                .unwrap_or(u64::MAX)
                .to_be_bytes(),
        );
        for category in self.data_categories {
            update_fingerprint_field(&mut hasher, category.as_bytes());
        }

        let mut fingerprint = String::with_capacity(64);
        for byte in hasher.finalize() {
            let _ = write!(&mut fingerprint, "{byte:02x}");
        }
        fingerprint
    }

    pub(crate) fn is_well_formed(&self) -> bool {
        self.provider == DEEPSEEK_CONSENT_PROVIDER
            && valid_policy_token(self.endpoint_host, 255)
            && valid_policy_token(self.model, 128)
            && valid_policy_token(self.policy_version, 64)
            && !self.data_categories.is_empty()
            && self.data_categories.len() <= 16
            && self
                .data_categories
                .iter()
                .all(|category| valid_policy_token(category, 64))
            && self
                .data_categories
                .iter()
                .enumerate()
                .all(|(index, category)| !self.data_categories[..index].contains(category))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProviderConsentPreference {
    pub(crate) provider: String,
    pub(crate) policy_version: String,
    pub(crate) policy_fingerprint: String,
    pub(crate) consented_at: String,
    pub(crate) updated_at: String,
}

impl ProviderConsentPreference {
    pub(crate) fn is_valid_for(&self, policy: &ProviderConsentPolicy<'_>) -> bool {
        policy.is_well_formed()
            && self.provider == policy.provider
            && self.policy_version == policy.policy_version
            && self.policy_fingerprint == policy.fingerprint()
    }
}

fn update_fingerprint_field(hasher: &mut Sha256, value: &[u8]) {
    hasher.update(u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(value);
}

fn valid_policy_token(value: &str, max_bytes: usize) -> bool {
    !value.is_empty()
        && value.trim() == value
        && value.len() <= max_bytes
        && !value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deepseek_policy_fingerprint_is_stable_bounded_and_scope_sensitive() {
        let policy = ProviderConsentPolicy::deepseek_transcript();
        let fingerprint = policy.fingerprint();

        assert!(policy.is_well_formed());
        assert_eq!(fingerprint.len(), 64);
        assert!(
            fingerprint
                .chars()
                .all(|character| character.is_ascii_hexdigit())
        );
        assert_eq!(fingerprint, fingerprint.to_ascii_lowercase());
        assert_eq!(fingerprint, policy.fingerprint());

        let changed_host = ProviderConsentPolicy {
            endpoint_host: "changed.example.invalid",
            ..policy
        };
        let changed_model = ProviderConsentPolicy {
            model: "changed-model",
            ..policy
        };
        let changed_version = ProviderConsentPolicy {
            policy_version: "deepseek-transcript-external-v2",
            ..policy
        };
        let changed_categories = ProviderConsentPolicy {
            data_categories: &[
                "transcript_text",
                "speaker_labels_and_names",
                "time_offsets",
                "evidence_ids",
                "summary_template",
            ],
            ..policy
        };

        for changed in [
            changed_host,
            changed_model,
            changed_version,
            changed_categories,
        ] {
            assert_ne!(fingerprint, changed.fingerprint());
        }
    }

    #[test]
    fn malformed_or_duplicate_policy_scope_is_rejected() {
        let policy = ProviderConsentPolicy::deepseek_transcript();
        assert!(
            !ProviderConsentPolicy {
                provider: "other",
                ..policy
            }
            .is_well_formed()
        );
        assert!(
            !ProviderConsentPolicy {
                endpoint_host: " api.deepseek.com",
                ..policy
            }
            .is_well_formed()
        );
        assert!(
            !ProviderConsentPolicy {
                data_categories: &["transcript_text", "transcript_text"],
                ..policy
            }
            .is_well_formed()
        );
    }
}
