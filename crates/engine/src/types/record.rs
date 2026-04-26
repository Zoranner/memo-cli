use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::MemoryLayer;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodeRecord {
    pub id: String,
    pub content: String,
    pub layer: MemoryLayer,
    pub confidence: f32,
    pub source_episode_id: Option<String>,
    pub session_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub archived_at: Option<DateTime<Utc>>,
    pub invalidated_at: Option<DateTime<Utc>>,
    pub hit_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityRecord {
    pub id: String,
    pub entity_type: String,
    pub canonical_name: String,
    pub aliases: Vec<String>,
    pub layer: MemoryLayer,
    pub confidence: f32,
    pub source_episode_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub archived_at: Option<DateTime<Utc>>,
    pub invalidated_at: Option<DateTime<Utc>>,
    pub hit_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactRecord {
    pub id: String,
    pub subject_entity_id: Option<String>,
    pub subject_text: String,
    pub predicate: String,
    pub object_entity_id: Option<String>,
    pub object_text: String,
    pub layer: MemoryLayer,
    pub confidence: f32,
    pub source_episode_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub valid_from: Option<DateTime<Utc>>,
    pub valid_to: Option<DateTime<Utc>>,
    pub archived_at: Option<DateTime<Utc>>,
    pub invalidated_at: Option<DateTime<Utc>>,
    pub hit_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeRecord {
    pub id: String,
    pub subject_entity_id: String,
    pub predicate: String,
    pub object_entity_id: String,
    pub weight: f32,
    pub source_episode_id: Option<String>,
    pub layer: MemoryLayer,
    pub valid_from: Option<DateTime<Utc>>,
    pub valid_to: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub archived_at: Option<DateTime<Utc>>,
    pub invalidated_at: Option<DateTime<Utc>>,
    pub hit_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoryRecord {
    Episode(EpisodeRecord),
    Entity(EntityRecord),
    Fact(FactRecord),
    Edge(EdgeRecord),
}

impl MemoryRecord {
    pub fn id(&self) -> &str {
        match self {
            Self::Episode(record) => &record.id,
            Self::Entity(record) => &record.id,
            Self::Fact(record) => &record.id,
            Self::Edge(record) => &record.id,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::Episode(_) => "episode",
            Self::Entity(_) => "entity",
            Self::Fact(_) => "fact",
            Self::Edge(_) => "edge",
        }
    }

    pub fn layer(&self) -> MemoryLayer {
        match self {
            Self::Episode(record) => record.layer,
            Self::Entity(record) => record.layer,
            Self::Fact(record) => record.layer,
            Self::Edge(record) => record.layer,
        }
    }

    pub fn hit_count(&self) -> u64 {
        match self {
            Self::Episode(record) => record.hit_count,
            Self::Entity(record) => record.hit_count,
            Self::Fact(record) => record.hit_count,
            Self::Edge(record) => record.hit_count,
        }
    }

    pub fn is_active(&self) -> bool {
        match self {
            Self::Episode(record) => {
                record.archived_at.is_none() && record.invalidated_at.is_none()
            }
            Self::Entity(record) => record.archived_at.is_none() && record.invalidated_at.is_none(),
            Self::Fact(record) => record.archived_at.is_none() && record.invalidated_at.is_none(),
            Self::Edge(record) => record.archived_at.is_none() && record.invalidated_at.is_none(),
        }
    }

    pub fn source_episode_id(&self) -> Option<&str> {
        match self {
            Self::Episode(_) => None,
            Self::Entity(record) => record.source_episode_id.as_deref(),
            Self::Fact(record) => record.source_episode_id.as_deref(),
            Self::Edge(record) => record.source_episode_id.as_deref(),
        }
    }

    pub fn source_key(&self) -> &str {
        self.source_episode_id().unwrap_or_else(|| self.id())
    }

    pub fn updated_at(&self) -> DateTime<Utc> {
        match self {
            Self::Episode(record) => record.updated_at,
            Self::Entity(record) => record.updated_at,
            Self::Fact(record) => record.updated_at,
            Self::Edge(record) => record.updated_at,
        }
    }

    pub fn activity_at(&self) -> DateTime<Utc> {
        match self {
            Self::Episode(record) => record.last_seen_at,
            Self::Entity(record) => record.last_seen_at,
            Self::Fact(record) => record.updated_at,
            Self::Edge(record) => record.updated_at,
        }
    }

    pub fn text_for_ranking(&self) -> String {
        match self {
            Self::Episode(record) => record.content.clone(),
            Self::Entity(record) => {
                format!("{} {}", record.canonical_name, record.aliases.join(" "))
            }
            Self::Fact(record) => {
                format!(
                    "{} {} {}",
                    record.subject_text, record.predicate, record.object_text
                )
            }
            Self::Edge(record) => {
                format!(
                    "{} {} {}",
                    record.subject_entity_id, record.predicate, record.object_entity_id
                )
            }
        }
    }
}
