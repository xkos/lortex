//! A2A Client — communicates with remote A2A-compatible agents.

use reqwest::Client;
use serde_json::Value;
use tracing::info;

use super::types::*;

/// Client for communicating with A2A-compatible agents.
pub struct A2AClient {
    client: Client,
}

impl A2AClient {
    /// Create a new A2A client.
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    /// Discover an agent's capabilities by fetching its Agent Card.
    pub async fn discover(&self, agent_url: &str) -> Result<AgentCard, String> {
        let card_url = format!(
            "{}/.well-known/agent.json",
            agent_url.trim_end_matches('/')
        );

        let resp = self
            .client
            .get(&card_url)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch agent card: {}", e))?;

        resp.json::<AgentCard>()
            .await
            .map_err(|e| format!("Failed to parse agent card: {}", e))
    }

    /// Send a task to a remote agent.
    pub async fn send_task(
        &self,
        agent_url: &str,
        message: A2AMessage,
    ) -> Result<A2ATask, String> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": uuid::Uuid::new_v4().to_string(),
            "method": "tasks/send",
            "params": {
                "id": uuid::Uuid::new_v4().to_string(),
                "message": message,
            }
        });

        let resp = self
            .client
            .post(agent_url)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("Failed to send task: {}", e))?;

        let body: Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        let task = body
            .get("result")
            .ok_or("No result in response")?;

        serde_json::from_value(task.clone()).map_err(|e| format!("Failed to parse task: {}", e))
    }

    /// Get the status of a task.
    pub async fn get_task(&self, agent_url: &str, task_id: &str) -> Result<A2ATask, String> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": uuid::Uuid::new_v4().to_string(),
            "method": "tasks/get",
            "params": {
                "id": task_id,
            }
        });

        let resp = self
            .client
            .post(agent_url)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("Failed to get task: {}", e))?;

        let body: Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        let task = body.get("result").ok_or("No result in response")?;

        serde_json::from_value(task.clone()).map_err(|e| format!("Failed to parse task: {}", e))
    }

    /// Cancel a task.
    pub async fn cancel_task(&self, agent_url: &str, task_id: &str) -> Result<A2ATask, String> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": uuid::Uuid::new_v4().to_string(),
            "method": "tasks/cancel",
            "params": {
                "id": task_id,
            }
        });

        let resp = self
            .client
            .post(agent_url)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("Failed to cancel task: {}", e))?;

        let body: Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        let task = body.get("result").ok_or("No result in response")?;

        serde_json::from_value(task.clone()).map_err(|e| format!("Failed to parse task: {}", e))
    }
}

impl Default for A2AClient {
    fn default() -> Self {
        Self::new()
    }
}
