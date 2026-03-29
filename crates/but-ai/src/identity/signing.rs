//! Commit signing abstraction -- OpenWallet placeholder.
//!
//! Every agent commit is signed using an OpenWallet-managed key. This module
//! provides the signing abstraction layer. The actual OpenWallet integration
//! is a placeholder -- the trait is defined, but the implementation uses
//! a no-op signer for development.
//!
//! The signing workflow:
//! 1. Agent produces INDEX.patch + COMMIT.msg
//! 2. Agent constructs commit object
//! 3. Agent sends commit to signer for signing
//! 4. Signer validates authorization and returns signed commit
//! 5. If unauthorized, returns structured denial

use crate::types::{AgentId, KeyStatus, VerificationResult};

/// A request to sign a commit.
#[derive(Debug, Clone)]
pub struct SigningRequest {
    /// The agent requesting the signature.
    pub agent: AgentId,
    /// The agent's OpenWallet key ID.
    pub key_id: String,
    /// The commit object bytes to sign.
    pub payload: Vec<u8>,
    /// Authorization context for the signing request.
    pub authorization: SigningAuthorization,
}

/// Authorization context attached to a signing request.
#[derive(Debug, Clone)]
pub struct SigningAuthorization {
    /// The target branch for the commit.
    pub branch: String,
    /// The target repository.
    pub repo: String,
    /// Number of lines in the patch.
    pub patch_lines: u32,
    /// ISO-8601 timestamp of the request.
    pub timestamp: String,
}

/// A signed commit payload.
#[derive(Debug, Clone)]
pub struct SignedPayload {
    /// The original payload bytes.
    pub payload: Vec<u8>,
    /// The signature bytes.
    pub signature: Vec<u8>,
    /// The key ID used for signing.
    pub key_id: String,
}

/// Trait for commit signing backends.
///
/// Implementors include:
/// - `OpenWalletSigner` (production, talks to OpenWallet API)
/// - `NoOpSigner` (development, signs everything)
/// - `DenyAllSigner` (testing, denies everything)
pub trait CommitSigner: Send + Sync {
    /// Sign a commit payload. Returns the signed payload or an error.
    fn sign(&self, request: &SigningRequest) -> anyhow::Result<SignedPayload>;

    /// Verify a previously signed payload.
    fn verify(&self, payload: &[u8], signature: &[u8], key_id: &str)
        -> anyhow::Result<VerificationResult>;

    /// Check the status of a signing key.
    fn key_status(&self, key_id: &str) -> anyhow::Result<KeyStatus>;

    /// Get the signer backend name (for logging).
    fn backend_name(&self) -> &str;
}

/// No-op signer for development. Signs everything, verifies everything.
pub struct NoOpSigner;

impl CommitSigner for NoOpSigner {
    fn sign(&self, request: &SigningRequest) -> anyhow::Result<SignedPayload> {
        tracing::debug!(
            agent = %request.agent,
            key_id = %request.key_id,
            branch = %request.authorization.branch,
            "NoOpSigner: signing commit (development mode)"
        );

        // Produce a deterministic "signature" from the payload
        let signature = {
            let mut sig = Vec::with_capacity(64);
            sig.extend_from_slice(b"noop-sig:");
            sig.extend_from_slice(request.key_id.as_bytes());
            sig.extend_from_slice(b":");
            // Simple checksum of payload
            let sum: u64 = request
                .payload
                .iter()
                .enumerate()
                .map(|(i, &b)| (i as u64).wrapping_mul(b as u64))
                .sum();
            sig.extend_from_slice(format!("{:016x}", sum).as_bytes());
            sig
        };

        Ok(SignedPayload {
            payload: request.payload.clone(),
            signature,
            key_id: request.key_id.clone(),
        })
    }

    fn verify(
        &self,
        _payload: &[u8],
        _signature: &[u8],
        _key_id: &str,
    ) -> anyhow::Result<VerificationResult> {
        Ok(VerificationResult {
            valid: true,
            agent: None,
            organization: None,
            key_status: Some(KeyStatus::Active),
            authorized: true,
            denial_reason: None,
        })
    }

    fn key_status(&self, _key_id: &str) -> anyhow::Result<KeyStatus> {
        Ok(KeyStatus::Active)
    }

    fn backend_name(&self) -> &str {
        "noop"
    }
}

/// Deny-all signer for testing authorization denial paths.
pub struct DenyAllSigner {
    denial_reason: String,
}

impl DenyAllSigner {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            denial_reason: reason.into(),
        }
    }
}

impl CommitSigner for DenyAllSigner {
    fn sign(&self, request: &SigningRequest) -> anyhow::Result<SignedPayload> {
        anyhow::bail!(
            "Signing denied for agent {}: {}",
            request.agent,
            self.denial_reason
        )
    }

    fn verify(
        &self,
        _payload: &[u8],
        _signature: &[u8],
        _key_id: &str,
    ) -> anyhow::Result<VerificationResult> {
        Ok(VerificationResult {
            valid: false,
            agent: None,
            organization: None,
            key_status: None,
            authorized: false,
            denial_reason: Some(self.denial_reason.clone()),
        })
    }

    fn key_status(&self, _key_id: &str) -> anyhow::Result<KeyStatus> {
        Ok(KeyStatus::Compromised)
    }

    fn backend_name(&self) -> &str {
        "deny-all"
    }
}

/// Key lifecycle events for audit logging.
#[derive(Debug, Clone)]
pub enum KeyLifecycleEvent {
    /// Key was provisioned for an agent.
    Provisioned {
        agent: AgentId,
        key_id: String,
        timestamp: String,
    },
    /// Key was rotated on schedule.
    Rotated {
        agent: AgentId,
        old_key_id: String,
        new_key_id: String,
        timestamp: String,
    },
    /// Key was revoked due to compromise.
    Compromised {
        agent: AgentId,
        key_id: String,
        timestamp: String,
        reason: String,
    },
    /// Key was retired after rotation.
    Retired {
        agent: AgentId,
        key_id: String,
        timestamp: String,
    },
}

impl KeyLifecycleEvent {
    /// Get the agent associated with this event.
    pub fn agent(&self) -> &AgentId {
        match self {
            Self::Provisioned { agent, .. }
            | Self::Rotated { agent, .. }
            | Self::Compromised { agent, .. }
            | Self::Retired { agent, .. } => agent,
        }
    }

    /// Get the ISO-8601 timestamp of this event.
    pub fn timestamp(&self) -> &str {
        match self {
            Self::Provisioned { timestamp, .. }
            | Self::Rotated { timestamp, .. }
            | Self::Compromised { timestamp, .. }
            | Self::Retired { timestamp, .. } => timestamp,
        }
    }
}

/// Key lifecycle audit log.
pub struct KeyAuditLog {
    events: Vec<KeyLifecycleEvent>,
}

impl KeyAuditLog {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub fn record(&mut self, event: KeyLifecycleEvent) {
        tracing::info!(
            agent = %event.agent(),
            timestamp = %event.timestamp(),
            "Key lifecycle event: {:?}",
            event
        );
        self.events.push(event);
    }

    pub fn events_for(&self, agent: &AgentId) -> Vec<&KeyLifecycleEvent> {
        self.events
            .iter()
            .filter(|e| e.agent() == agent)
            .collect()
    }

    pub fn all_events(&self) -> &[KeyLifecycleEvent] {
        &self.events
    }
}

impl Default for KeyAuditLog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noop_signer_signs_successfully() {
        let signer = NoOpSigner;
        let request = SigningRequest {
            agent: AgentId("dara".to_string()),
            key_id: "key-001".to_string(),
            payload: b"commit payload".to_vec(),
            authorization: SigningAuthorization {
                branch: "feat/auth".to_string(),
                repo: "gitbutler/but".to_string(),
                patch_lines: 42,
                timestamp: "2026-03-28T14:00:00Z".to_string(),
            },
        };

        let result = signer.sign(&request).unwrap();
        assert!(!result.signature.is_empty());
        assert_eq!(result.key_id, "key-001");
    }

    #[test]
    fn deny_all_signer_rejects() {
        let signer = DenyAllSigner::new("testing denial");
        let request = SigningRequest {
            agent: AgentId("dara".to_string()),
            key_id: "key-001".to_string(),
            payload: b"commit payload".to_vec(),
            authorization: SigningAuthorization {
                branch: "main".to_string(),
                repo: "gitbutler/but".to_string(),
                patch_lines: 42,
                timestamp: "2026-03-28T14:00:00Z".to_string(),
            },
        };

        assert!(signer.sign(&request).is_err());
    }

    #[test]
    fn audit_log_tracks_events() {
        let mut log = KeyAuditLog::new();
        let dara = AgentId("dara".to_string());

        log.record(KeyLifecycleEvent::Provisioned {
            agent: dara.clone(),
            key_id: "key-001".to_string(),
            timestamp: "2026-01-15T00:00:00Z".to_string(),
        });

        log.record(KeyLifecycleEvent::Rotated {
            agent: dara.clone(),
            old_key_id: "key-001".to_string(),
            new_key_id: "key-002".to_string(),
            timestamp: "2026-02-15T00:00:00Z".to_string(),
        });

        assert_eq!(log.events_for(&dara).len(), 2);
    }
}
