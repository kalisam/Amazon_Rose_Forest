//! CRDT (Conflict-free Replicated Data Types) for knowledge management
//! Enables conflict resolution in distributed knowledge updates

use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, SystemTime};
use serde::{Serialize, Deserialize};
use crate::knowledge::representation::{Knowledge, KnowledgeMetadata};
use thiserror::Error;

/// Agent identifier for CRDT operations
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AgentId(pub String);

/// Version vector for tracking causality
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionVector {
    /// Map of agent IDs to their version counters
    pub versions: BTreeMap<AgentId, u64>,
}

impl VersionVector {
    /// Create a new empty version vector
    pub fn new() -> Self {
        Self {
            versions: BTreeMap::new(),
        }
    }

    /// Increment the version for an agent
    pub fn increment(&mut self, agent: AgentId) -> u64 {
        let counter = self.versions.entry(agent).or_insert(0);
        *counter += 1;
        *counter
    }

    /// Check if this version vector is newer than another
    pub fn is_newer_than(&self, other: &Self) -> bool {
        // Check if any version in self is greater than the corresponding version in other
        for (agent, &version) in &self.versions {
            match other.versions.get(agent) {
                Some(&other_version) if version > other_version => return true,
                None if version > 0 => return true,
                _ => {}
            }
        }
        false
    }

    /// Check if this version vector is concurrent with another
    pub fn concurrent_with(&self, other: &Self) -> bool {
        // Two version vectors are concurrent if neither is newer than the other
        !self.is_newer_than(other) && !other.is_newer_than(self)
    }

    /// Merge this version vector with another
    pub fn merge(&mut self, other: &Self) {
        for (agent, &version) in &other.versions {
            let entry = self.versions.entry(agent.clone()).or_insert(0);
            *entry = (*entry).max(version);
        }
    }
}

/// Knowledge entry with CRDT capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CRDTKnowledge {
    /// The knowledge content
    pub knowledge: Knowledge,
    /// Version vector for tracking causality
    pub version: VersionVector,
    /// Tombstone flag for deletion
    pub tombstone: bool,
    /// Last update timestamp
    pub last_update: u64,
}

impl CRDTKnowledge {
    /// Create a new CRDT knowledge entry
    pub fn new(knowledge: Knowledge, agent: AgentId) -> Self {
        let mut version = VersionVector::new();
        version.increment(agent);

        Self {
            knowledge,
            version,
            tombstone: false,
            last_update: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or(Duration::from_secs(0))
                .as_secs(),
        }
    }

    /// Mark this knowledge as deleted
    pub fn delete(&mut self, agent: AgentId) {
        self.version.increment(agent);
        self.tombstone = true;
        self.last_update = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_secs();
    }

    /// Merge this knowledge with another
    pub fn merge(&mut self, other: &Self) -> bool {
        // If both are tombstones, merge version vectors
        if self.tombstone && other.tombstone {
            self.version.merge(&other.version);
            self.last_update = self.last_update.max(other.last_update);
            return false;
        }

        // If one is a tombstone, prefer the one with the higher version
        if self.tombstone || other.tombstone {
            if other.version.is_newer_than(&self.version) {
                *self = other.clone();
                return true;
            }
            return false;
        }

        // If versions are concurrent, merge the knowledge
        if self.version.concurrent_with(&other.version) {
            // Merge metadata
            self.merge_metadata(&other.knowledge.metadata);

            // Merge vectors (use the one with higher confidence)
            if other.knowledge.metadata.confidence > self.knowledge.metadata.confidence {
                self.knowledge.vectors = other.knowledge.vectors.clone();
            }

            // Merge version vectors
            self.version.merge(&other.version);
            self.last_update = self.last_update.max(other.last_update);

            return true;
        }

        // Otherwise, keep the one with the higher version
        if other.version.is_newer_than(&self.version) {
            *self = other.clone();
            return true;
        }

        false
    }

    /// Merge metadata from another knowledge entry
    fn merge_metadata(&mut self, other: &KnowledgeMetadata) {
        // Merge tags
        let mut tags = BTreeSet::new();
        for tag in &self.knowledge.metadata.tags {
            tags.insert(tag.clone());
        }
        for tag in &other.tags {
            tags.insert(tag.clone());
        }

        // Update metadata
        self.knowledge.metadata.tags = tags.into_iter().collect();

        // Use the newer timestamp
        if other.timestamp > self.knowledge.metadata.timestamp {
            self.knowledge.metadata.timestamp = other.timestamp;
        }

        // Use weighted average for confidence
        let total_confidence = self.knowledge.metadata.confidence + other.confidence;
        if total_confidence > 0.0 {
            self.knowledge.metadata.confidence =
                (self.knowledge.metadata.confidence * 0.5) + (other.confidence * 0.5);
        }
    }
}

/// A set of knowledge entries with CRDT capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CRDTKnowledgeSet {
    /// Map of knowledge IDs to CRDT knowledge entries
    pub entries: BTreeMap<String, CRDTKnowledge>,
}

const MAX_ENTRIES: usize = 10_000;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CRDTError {
    #[error("Storage limit reached")]
    StorageLimitReached,
    #[error("Invalid knowledge format")]
    InvalidKnowledge,
}

impl CRDTKnowledgeSet {
    /// Create a new empty CRDT knowledge set
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    fn insert(&mut self, knowledge: Knowledge, agent: AgentId) -> Result<(), CRDTError> {
        if self.entries.len() >= MAX_ENTRIES {
            return Err(CRDTError::StorageLimitReached);
        }
        let id = knowledge.id.clone();
        let entry = CRDTKnowledge::new(knowledge, agent);

        match self.entries.get_mut(&id) {
            Some(existing) => {
                existing.merge(&entry);
            }
            None => {
                self.entries.insert(id, entry);
            }
        }
        Ok(())
    }

    fn batch_insert(&mut self, knowledge_list: Vec<Knowledge>, agent: AgentId) -> Result<(), CRDTError> {
        if self.entries.len() + knowledge_list.len() > MAX_ENTRIES {
            return Err(CRDTError::StorageLimitReached);
        }
        
        for knowledge in knowledge_list {
            self.insert(knowledge, agent.clone())?;
        }
        Ok(())
    }

    /// Add or update a knowledge entry
    pub fn add(&mut self, knowledge: Knowledge, agent: AgentId) -> Result<(), CRDTError> {
        self.insert(knowledge, agent)
    }

    /// Delete a knowledge entry
    pub fn delete(&mut self, id: &str, agent: AgentId) {
        if let Some(entry) = self.entries.get_mut(id) {
            entry.delete(agent);
        }
    }

    /// Merge with another CRDT knowledge set
    pub fn merge(&mut self, other: &Self) {
        for (id, entry) in &other.entries {
            match self.entries.get_mut(id) {
                Some(existing) => {
                    existing.merge(entry);
                }
                None => {
                    self.entries.insert(id.clone(), entry.clone());
                }
            }
        }
    }

    /// Get all non-tombstone entries
    pub fn get_active_entries(&self) -> Vec<&Knowledge> {
        self.entries.values()
            .filter(|entry| !entry.tombstone)
            .map(|entry| &entry.knowledge)
            .collect()
    }
}

impl Default for CRDTKnowledgeSet {
    fn default() -> Self {
        Self::new()
    }
}