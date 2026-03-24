//! A2A protocol types based on the Agent2Agent Protocol specification.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Agent Card — describes an agent's capabilities and endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCard {
    /// Agent name.
    pub name: String,

    /// Description of the agent's capabilities.
    pub description: String,

    /// URL where the agent can be reached.
    pub url: String,

    /// Agent version.
    #[serde(default)]
    pub version: Option<String>,

    /// Supported capabilities.
    #[serde(default)]
    pub capabilities: AgentCapabilities,

    /// Skills the agent has.
    #[serde(default)]
    pub skills: Vec<AgentSkill>,
}

/// Capabilities of an A2A agent.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentCapabilities {
    #[serde(default)]
    pub streaming: bool,

    #[serde(rename = "pushNotifications", default)]
    pub push_notifications: bool,

    #[serde(rename = "stateTransitionHistory", default)]
    pub state_transition_history: bool,
}

/// A skill that an agent can perform.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSkill {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// A2A Task — represents a unit of work between agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2ATask {
    pub id: String,

    #[serde(rename = "sessionId")]
    pub session_id: String,

    pub status: TaskStatus,

    #[serde(default)]
    pub messages: Vec<A2AMessage>,

    #[serde(default)]
    pub artifacts: Vec<A2AArtifact>,
}

/// Status of an A2A task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStatus {
    pub state: TaskState,
    #[serde(default)]
    pub message: Option<A2AMessage>,
}

/// Task states.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskState {
    Submitted,
    Working,
    InputRequired,
    Completed,
    Canceled,
    Failed,
}

/// A2A message — communication between agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2AMessage {
    pub role: String,
    pub parts: Vec<A2APart>,
}

/// A2A message part — the smallest unit of content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum A2APart {
    Text { text: String },
    File { file: A2AFile },
    Data { data: Value },
}

/// A file exchanged between agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2AFile {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(rename = "mimeType", default)]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub uri: Option<String>,
    #[serde(default)]
    pub bytes: Option<String>,
}

/// An artifact produced by a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2AArtifact {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub parts: Vec<A2APart>,
}
