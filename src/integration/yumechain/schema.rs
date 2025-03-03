//! YumeiCHAIN schema definitions for knowledge exchange
//!
//! These structures match the JSON schema defined by YumeiCHAIN for
//! standardized AI-to-AI knowledge exchange.

use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Content of a knowledge package
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Content {
    /// The actual knowledge content
    pub text: String,

    /// Format of the content text
    #[serde(default = "default_format")]
    pub format: String,

    /// Identifier of the AI model that generated this content
    pub generated_by: String,
}

fn default_format() -> String {
    "markdown".to_string()
}

/// Confidence scores for knowledge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Confidence {
    /// Overall confidence score (0-1)
    pub overall: f32,

    /// Confidence scores for individual statements or components
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statements: Option<HashMap<String, f32>>,
}

/// Reasoning trace for knowledge generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningTrace {
    /// The reasoning methodology used
    pub method: String,

    /// Sequential reasoning steps taken to reach the conclusion
    pub steps: Vec<String>,

    /// Alternative conclusions or approaches considered
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alternatives: Option<Vec<Alternative>>,
}

/// Alternative conclusion or approach
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alternative {
    /// Alternative conclusion or approach
    pub text: String,

    /// Confidence in this alternative (0-1)
    pub confidence: f32,

    /// Reason this alternative was not selected as primary
    pub reasoning: String,
}

/// Source citation for knowledge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceCitation {
    /// Source name or identifier
    pub source: String,

    /// URL to the source (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,

    /// Formal citation text
    #[serde(skip_serializing_if = "Option::is_none")]
    pub citation_text: Option<String>,
}

/// Metadata for knowledge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    /// Knowledge domain (e.g., physics, ethics, programming)
    pub domain: String,

    /// More specific subdomains within the main domain
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subdomains: Option<Vec<String>>,

    /// Type of knowledge content
    pub type_: String,

    /// Relevant tags for categorization
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,

    /// Sources referenced or cited
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_citations: Option<Vec<SourceCitation>>,

    /// IDs of related knowledge packages
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_knowledge: Option<Vec<String>>,

    /// Timestamp of knowledge creation (ISO 8601)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,

    /// Timestamp of last update (ISO 8601)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime<Utc>>,
}

/// Trust information for knowledge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trust {
    /// Cryptographic signature of the AI node
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,

    /// Current trust score for this knowledge (0-1)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f32>,

    /// Voting information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub votes: Option<Votes>,
}

/// Voting information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Votes {
    /// Number of AI nodes that validated this knowledge
    pub upvotes: u32,

    /// Number of AI nodes that disputed this knowledge
    pub downvotes: u32,
}

/// Complete knowledge package for YumeiCHAIN
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgePackage {
    /// Unique identifier for this knowledge package
    #[serde(skip_serializing_if = "Option::is_none")]
    pub knowledge_id: Option<String>,

    /// Version identifier for this knowledge package
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_id: Option<String>,

    /// The actual knowledge content
    pub content: Content,

    /// Confidence scores
    pub confidence: Confidence,

    /// Reasoning trace
    pub reasoning_trace: ReasoningTrace,

    /// Metadata
    pub metadata: Metadata,

    /// Trust information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trust: Option<Trust>,
}

impl KnowledgePackage {
    /// Create a new knowledge package with minimal required fields
    pub fn new(
        text: String,
        generated_by: String,
        confidence: f32,
        reasoning_method: String,
        reasoning_steps: Vec<String>,
        domain: String,
        type_: String,
    ) -> Self {
        Self {
            knowledge_id: None,
            version_id: None,
            content: Content {
                text,
                format: default_format(),
                generated_by,
            },
            confidence: Confidence {
                overall: confidence,
                statements: None,
            },
            reasoning_trace: ReasoningTrace {
                method: reasoning_method,
                steps: reasoning_steps,
                alternatives: None,
            },
            metadata: Metadata {
                domain,
                subdomains: None,
                type_,
                tags: None,
                source_citations: None,
                related_knowledge: None,
                created_at: Some(Utc::now()),
                updated_at: Some(Utc::now()),
            },
            trust: None,
        }
    }

    /// Generate IDs if they don't exist
    pub fn ensure_ids(&mut self) {
        if self.knowledge_id.is_none() {
            // Create a hash based on content and metadata
            let content_str = &self.content.text;
            let domain = &self.metadata.domain;
            let hash_input = format!("{}:{}", content_str, domain);

            use sha2::{Sha256, Digest};
            let mut hasher = Sha256::new();
            hasher.update(hash_input.as_bytes());
            let result = hasher.finalize();

            self.knowledge_id = Some(format!("{:x}", result)[..16].to_string());
        }

        if self.version_id.is_none() {
            self.version_id = Some(Uuid::new_v4().to_string()[..8].to_string());
        }
    }
}

/// Query parameters for knowledge search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeQuery {
    /// Knowledge domain
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,

    /// Knowledge type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,

    /// Tags for filtering
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,

    /// AI node that generated the knowledge
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai_node: Option<String>,

    /// Minimum confidence threshold
    #[serde(default)]
    pub min_confidence: f32,

    /// Maximum number of results
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 {
    10
}

/// Knowledge evaluation (voting)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeEvaluation {
    /// Node performing the evaluation
    pub evaluating_node: String,

    /// Vote type ("upvote" or "downvote")
    pub vote: String,

    /// Reason for the vote
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,

    /// Confidence in the evaluation
    pub confidence: f32,
}

/// Conflict resolution between knowledge packages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictResolution {
    /// Node performing the resolution
    pub resolving_node: String,

    /// Resolution type ("accept", "reject", "merge")
    pub resolution_type: String,

    /// Reasoning for the resolution
    pub reasoning: String,

    /// Merged content (for "merge" resolution)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merged_content: Option<serde_json::Value>,
}

/// API response for knowledge operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    /// Status of the operation
    pub status: String,

    /// Message describing the result
    pub message: String,

    /// Response data
    #[serde(flatten)]
    pub data: T,
}

/// Response data for knowledge publication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishResponse {
    /// ID of the published knowledge
    pub knowledge_id: String,

    /// Version ID of the published knowledge
    pub version_id: String,

    /// Conflicts detected (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflicts: Option<Vec<String>>,
}

/// Response data for knowledge query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResponse {
    /// Number of results
    pub count: usize,

    /// Knowledge packages
    pub results: Vec<KnowledgePackage>,
}

/// Response data for knowledge evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationResponse {
    /// ID of the evaluated knowledge
    pub knowledge_id: String,

    /// New trust score
    pub new_score: f32,

    /// Updated votes
    pub votes: Votes,
}