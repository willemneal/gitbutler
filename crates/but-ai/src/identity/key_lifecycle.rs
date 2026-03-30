//! Key provisioning, rotation, compromise, and decommission.
//!
//! Manages the full lifecycle of agent signing keys with
//! survival-based rotation scheduling: keys are rotated when their
//! survival probability drops below a threshold.

use crate::types::AgentId;

use super::signing::{KeyAuditLog, KeyLifecycleEvent};

/// Status of a signing key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyStatus {
    /// Key is active and can sign.
    Active,
    /// Key is pending rotation (still valid but rotation is scheduled).
    PendingRotation,
    /// Key has been compromised and must not be used.
    Compromised,
    /// Key has been retired after successful rotation.
    Decommissioned,
}

/// A signing key record.
#[derive(Debug, Clone)]
pub struct KeyRecord {
    /// The key identifier.
    pub key_id: String,
    /// The agent this key belongs to.
    pub agent: AgentId,
    /// Current status.
    pub status: KeyStatus,
    /// ISO-8601 provisioning timestamp.
    pub provisioned_at: String,
    /// ISO-8601 timestamp when rotation is recommended.
    pub rotation_due_at: Option<String>,
}

/// Key lifecycle manager.
///
/// Tracks all keys for all agents and manages transitions between
/// key states.
pub struct KeyManager {
    keys: Vec<KeyRecord>,
    audit_log: KeyAuditLog,
}

impl KeyManager {
    pub fn new() -> Self {
        Self {
            keys: Vec::new(),
            audit_log: KeyAuditLog::new(),
        }
    }

    /// Provision a new key for an agent.
    pub fn provision(
        &mut self,
        agent: AgentId,
        key_id: String,
        timestamp: String,
    ) -> &KeyRecord {
        self.audit_log.record(KeyLifecycleEvent::Provisioned {
            agent: agent.clone(),
            key_id: key_id.clone(),
            timestamp: timestamp.clone(),
        });

        self.keys.push(KeyRecord {
            key_id,
            agent,
            status: KeyStatus::Active,
            provisioned_at: timestamp,
            rotation_due_at: None,
        });

        self.keys.last().unwrap()
    }

    /// Schedule rotation for a key by setting its rotation_due_at.
    pub fn schedule_rotation(&mut self, key_id: &str, rotation_due_at: String) -> bool {
        if let Some(key) = self.keys.iter_mut().find(|k| k.key_id == key_id) {
            key.status = KeyStatus::PendingRotation;
            key.rotation_due_at = Some(rotation_due_at);
            true
        } else {
            false
        }
    }

    /// Rotate a key: decommission the old one and provision a new one.
    pub fn rotate(
        &mut self,
        old_key_id: &str,
        new_key_id: String,
        timestamp: String,
    ) -> anyhow::Result<()> {
        let agent = {
            let old = self
                .keys
                .iter_mut()
                .find(|k| k.key_id == old_key_id)
                .ok_or_else(|| anyhow::anyhow!("Key not found: {old_key_id}"))?;
            old.status = KeyStatus::Decommissioned;
            old.agent.clone()
        };

        self.audit_log.record(KeyLifecycleEvent::Rotated {
            agent: agent.clone(),
            old_key_id: old_key_id.to_string(),
            new_key_id: new_key_id.clone(),
            timestamp: timestamp.clone(),
        });

        self.keys.push(KeyRecord {
            key_id: new_key_id,
            agent,
            status: KeyStatus::Active,
            provisioned_at: timestamp,
            rotation_due_at: None,
        });

        Ok(())
    }

    /// Mark a key as compromised. This is an emergency operation.
    pub fn compromise(
        &mut self,
        key_id: &str,
        timestamp: String,
        reason: String,
    ) -> anyhow::Result<()> {
        let key = self
            .keys
            .iter_mut()
            .find(|k| k.key_id == key_id)
            .ok_or_else(|| anyhow::anyhow!("Key not found: {key_id}"))?;

        key.status = KeyStatus::Compromised;

        self.audit_log.record(KeyLifecycleEvent::Compromised {
            agent: key.agent.clone(),
            key_id: key_id.to_string(),
            timestamp,
            reason,
        });

        Ok(())
    }

    /// Decommission a key (normal end of life after rotation).
    pub fn decommission(&mut self, key_id: &str, timestamp: String) -> anyhow::Result<()> {
        let key = self
            .keys
            .iter_mut()
            .find(|k| k.key_id == key_id)
            .ok_or_else(|| anyhow::anyhow!("Key not found: {key_id}"))?;

        key.status = KeyStatus::Decommissioned;

        self.audit_log.record(KeyLifecycleEvent::Decommissioned {
            agent: key.agent.clone(),
            key_id: key_id.to_string(),
            timestamp,
        });

        Ok(())
    }

    /// Determine if a key should be rotated based on the survival probability
    /// of the memory entries signed by this agent.
    ///
    /// The idea: if an agent's mean patch survival drops below a threshold,
    /// the key might be contributing to low-quality work and should be
    /// rotated (forcing a re-evaluation of the agent's identity).
    pub fn should_rotate_based_on_survival(
        mean_patch_survival_days: f64,
        rotation_threshold_days: f64,
    ) -> bool {
        mean_patch_survival_days < rotation_threshold_days
    }

    /// Get the active key for an agent.
    pub fn active_key(&self, agent: &AgentId) -> Option<&KeyRecord> {
        self.keys
            .iter()
            .find(|k| k.agent == *agent && k.status == KeyStatus::Active)
    }

    /// Get all keys for an agent.
    pub fn keys_for(&self, agent: &AgentId) -> Vec<&KeyRecord> {
        self.keys.iter().filter(|k| k.agent == *agent).collect()
    }

    /// Get the audit log.
    pub fn audit_log(&self) -> &KeyAuditLog {
        &self.audit_log
    }
}

impl Default for KeyManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provision_and_retrieve() {
        let mut mgr = KeyManager::new();
        let agent = AgentId("vassiliev".to_string());
        mgr.provision(agent.clone(), "key-001".to_string(), "2026-01-01T00:00:00Z".to_string());

        let active = mgr.active_key(&agent).unwrap();
        assert_eq!(active.key_id, "key-001");
        assert_eq!(active.status, KeyStatus::Active);
    }

    #[test]
    fn rotate_key() {
        let mut mgr = KeyManager::new();
        let agent = AgentId("petrov".to_string());
        mgr.provision(agent.clone(), "key-001".to_string(), "2026-01-01T00:00:00Z".to_string());
        mgr.rotate("key-001", "key-002".to_string(), "2026-02-01T00:00:00Z".to_string())
            .unwrap();

        let active = mgr.active_key(&agent).unwrap();
        assert_eq!(active.key_id, "key-002");

        let all = mgr.keys_for(&agent);
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].status, KeyStatus::Decommissioned);
    }

    #[test]
    fn compromise_key() {
        let mut mgr = KeyManager::new();
        let agent = AgentId("test".to_string());
        mgr.provision(agent.clone(), "key-001".to_string(), "2026-01-01T00:00:00Z".to_string());
        mgr.compromise("key-001", "2026-01-15T00:00:00Z".to_string(), "leaked".to_string())
            .unwrap();

        let key = mgr.keys_for(&agent)[0];
        assert_eq!(key.status, KeyStatus::Compromised);
    }

    #[test]
    fn survival_based_rotation() {
        assert!(KeyManager::should_rotate_based_on_survival(10.0, 30.0));
        assert!(!KeyManager::should_rotate_based_on_survival(60.0, 30.0));
    }
}
