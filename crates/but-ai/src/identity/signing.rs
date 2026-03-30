//! Commit signing implementations.
//!
//! Provides `CommitSigner` trait implementations:
//! - `NoOpSigner`: signs everything (development mode).
//! - `DenyAllSigner`: denies everything (testing denial paths).
//!
//! Also provides a key audit log for tracking key lifecycle events.

use crate::types::{AgentId, CommitSigner};

/// No-op signer for development. Signs everything, verifies everything.
pub struct NoOpSigner;

impl CommitSigner for NoOpSigner {
    fn sign(&self, message: &[u8]) -> anyhow::Result<Vec<u8>> {
        // Produce a deterministic "signature" from the message.
        let mut sig = Vec::with_capacity(64);
        sig.extend_from_slice(b"noop-sig:");
        let sum: u64 = message
            .iter()
            .enumerate()
            .map(|(i, &b)| (i as u64).wrapping_mul(b as u64))
            .sum();
        sig.extend_from_slice(format!("{sum:016x}").as_bytes());
        Ok(sig)
    }

    fn verify(
        &self,
        _message: &[u8],
        _signature: &[u8],
        _agent: &AgentId,
    ) -> anyhow::Result<bool> {
        Ok(true)
    }
}

/// Deny-all signer for testing authorization denial paths.
pub struct DenyAllSigner {
    reason: String,
}

impl DenyAllSigner {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

impl CommitSigner for DenyAllSigner {
    fn sign(&self, _message: &[u8]) -> anyhow::Result<Vec<u8>> {
        anyhow::bail!("Signing denied: {}", self.reason)
    }

    fn verify(
        &self,
        _message: &[u8],
        _signature: &[u8],
        _agent: &AgentId,
    ) -> anyhow::Result<bool> {
        Ok(false)
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
    /// Key was decommissioned after rotation.
    Decommissioned {
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
            | Self::Decommissioned { agent, .. } => agent,
        }
    }

    /// Get the ISO-8601 timestamp of this event.
    pub fn timestamp(&self) -> &str {
        match self {
            Self::Provisioned { timestamp, .. }
            | Self::Rotated { timestamp, .. }
            | Self::Compromised { timestamp, .. }
            | Self::Decommissioned { timestamp, .. } => timestamp,
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

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
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
        let sig = signer.sign(b"commit payload").unwrap();
        assert!(!sig.is_empty());
    }

    #[test]
    fn noop_signer_verifies_everything() {
        let signer = NoOpSigner;
        let agent = AgentId("test".to_string());
        assert!(signer.verify(b"msg", b"sig", &agent).unwrap());
    }

    #[test]
    fn deny_all_signer_rejects() {
        let signer = DenyAllSigner::new("testing denial");
        assert!(signer.sign(b"payload").is_err());
    }

    #[test]
    fn deny_all_signer_fails_verification() {
        let signer = DenyAllSigner::new("testing");
        let agent = AgentId("test".to_string());
        assert!(!signer.verify(b"msg", b"sig", &agent).unwrap());
    }

    #[test]
    fn audit_log_tracks_events() {
        let mut log = KeyAuditLog::new();
        let agent = AgentId("vassiliev".to_string());

        log.record(KeyLifecycleEvent::Provisioned {
            agent: agent.clone(),
            key_id: "key-001".to_string(),
            timestamp: "2026-01-15T00:00:00Z".to_string(),
        });

        log.record(KeyLifecycleEvent::Rotated {
            agent: agent.clone(),
            old_key_id: "key-001".to_string(),
            new_key_id: "key-002".to_string(),
            timestamp: "2026-02-15T00:00:00Z".to_string(),
        });

        assert_eq!(log.events_for(&agent).len(), 2);
        assert_eq!(log.len(), 2);
    }
}
