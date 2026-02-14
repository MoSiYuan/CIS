//! MCP Resources Module
//!
//! Implements resource management with full CRUD operations,
//! subscriptions, and metadata support following MCP specification.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Resource definition with full metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub uri: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub mime_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ResourceMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<ResourceAnnotations>,
}

/// Resource metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom: Option<HashMap<String, Value>>,
}

/// Resource annotations for additional metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAnnotations {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom: Option<HashMap<String, Value>>,
}

/// Resource content
#[derive(Debug, Clone, Serialize)]
pub struct ResourceContent {
    pub uri: String,
    pub mime_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blob: Option<String>, // Base64 encoded binary data
}

/// Resource subscription
#[derive(Debug, Clone)]
pub struct ResourceSubscription {
    pub id: String,
    pub uri: String,
    pub subscriber_id: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Resource change event
#[derive(Debug, Clone, Serialize)]
pub struct ResourceChangeEvent {
    pub uri: String,
    pub change_type: ResourceChangeType,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_content: Option<ResourceContent>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ResourceChangeType {
    Created,
    Updated,
    Deleted,
}

/// Resource manager with full CRUD and subscription support
pub struct ResourceManager {
    resources: Arc<RwLock<HashMap<String, Resource>>>,
    content_store: Arc<RwLock<HashMap<String, ResourceContent>>>,
    subscriptions: Arc<RwLock<HashMap<String, ResourceSubscription>>>,
    subscribers: Arc<RwLock<HashMap<String, Vec<String>>>>, // uri -> subscriber_ids
}

impl Default for ResourceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceManager {
    pub fn new() -> Self {
        let manager = Self {
            resources: Arc::new(RwLock::new(HashMap::new())),
            content_store: Arc::new(RwLock::new(HashMap::new())),
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            subscribers: Arc::new(RwLock::new(HashMap::new())),
        };

        // Register built-in resources in background
        let manager_clone = Self {
            resources: manager.resources.clone(),
            content_store: manager.content_store.clone(),
            subscriptions: manager.subscriptions.clone(),
            subscribers: manager.subscribers.clone(),
        };

        tokio::spawn(async move {
            if let Err(e) = manager_clone.register_builtin_resources().await {
                warn!("Failed to register built-in resources: {}", e);
            }
        });

        manager
    }

    /// Register built-in resources
    async fn register_builtin_resources(&self) -> Result<()> {
        // Current context resource
        self.create_resource(Resource {
            uri: "context://current".to_string(),
            name: "Current Context".to_string(),
            description: Some("Current project context information".to_string()),
            mime_type: "application/json".to_string(),
            metadata: Some(ResourceMetadata {
                size: None,
                last_modified: None,
                created_at: Some(chrono::Utc::now().to_rfc3339()),
                version: Some("1.0".to_string()),
                custom: None,
            }),
            annotations: Some(ResourceAnnotations {
                role: Some("system".to_string()),
                priority: Some(0),
                tags: Some(vec!["context".to_string(), "dynamic".to_string()]),
                custom: None,
            }),
        })
        .await?;

        self.update_content(
            "context://current",
            ResourceContent {
                uri: "context://current".to_string(),
                mime_type: "application/json".to_string(),
                text: Some(json!({
                    "project_root": null,
                    "project_type": null,
                    "package_manager": null,
                    "git_branch": null
                }).to_string()),
                blob: None,
            }
        ).await?;

        // CIS configuration resource
        self.create_resource(Resource {
            uri: "cis://config".to_string(),
            name: "CIS Configuration".to_string(),
            description: Some("Current CIS configuration".to_string()),
            mime_type: "application/json".to_string(),
            metadata: Some(ResourceMetadata {
                size: None,
                last_modified: None,
                created_at: Some(chrono::Utc::now().to_rfc3339()),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
                custom: None,
            }),
            annotations: Some(ResourceAnnotations {
                role: Some("config".to_string()),
                priority: Some(1),
                tags: Some(vec!["config".to_string(), "system".to_string()]),
                custom: None,
            }),
        })
        .await?;

        self.update_content(
            "cis://config",
            ResourceContent {
                uri: "cis://config".to_string(),
                mime_type: "application/json".to_string(),
                text: Some(json!({
                    "version": env!("CARGO_PKG_VERSION"),
                    "capabilities": ["dag", "skills", "memory", "context"]
                }).to_string()),
                blob: None,
            }
        ).await?;

        info!("Built-in resources registered");
        Ok(())
    }

    /// List all resources
    pub async fn list_resources(&self) -> Result<Vec<Resource>> {
        let resources = self.resources.read().await;
        Ok(resources.values().cloned().collect())
    }

    /// Get a specific resource by URI
    pub async fn get_resource(&self, uri: &str) -> Result<Resource> {
        let resources = self.resources.read().await;
        resources
            .get(uri)
            .cloned()
            .ok_or_else(|| anyhow!("Resource not found: {}", uri))
    }

    /// Create a new resource
    pub async fn create_resource(&self, resource: Resource) -> Result<()> {
        let mut resources = self.resources.write().await;

        if resources.contains_key(&resource.uri) {
            return Err(anyhow!("Resource already exists: {}", resource.uri));
        }

        resources.insert(resource.uri.clone(), resource);
        info!("Resource created: {}", resource.uri);

        // Notify subscribers
        drop(resources);
        self.notify_subscribers(ResourceChangeEvent {
            uri: resource.uri.clone(),
            change_type: ResourceChangeType::Created,
            timestamp: chrono::Utc::now(),
            new_content: None,
        })
        .await;

        Ok(())
    }

    /// Update an existing resource
    pub async fn update_resource(&self, uri: &str, updates: Resource) -> Result<()> {
        let mut resources = self.resources.write().await;

        if !resources.contains_key(uri) {
            return Err(anyhow!("Resource not found: {}", uri));
        }

        resources.insert(uri.to_string(), updates.clone());
        info!("Resource updated: {}", uri);

        // Notify subscribers
        drop(resources);
        self.notify_subscribers(ResourceChangeEvent {
            uri: uri.to_string(),
            change_type: ResourceChangeType::Updated,
            timestamp: chrono::Utc::now(),
            new_content: None,
        })
        .await;

        Ok(())
    }

    /// Delete a resource
    pub async fn delete_resource(&self, uri: &str) -> Result<()> {
        let mut resources = self.resources.write().await;
        let mut content_store = self.content_store.write().await;

        if !resources.contains_key(uri) {
            return Err(anyhow!("Resource not found: {}", uri));
        }

        resources.remove(uri);
        content_store.remove(uri);
        info!("Resource deleted: {}", uri);

        // Notify subscribers
        drop(resources);
        self.notify_subscribers(ResourceChangeEvent {
            uri: uri.to_string(),
            change_type: ResourceChangeType::Deleted,
            timestamp: chrono::Utc::now(),
            new_content: None,
        })
        .await;

        Ok(())
    }

    /// Read resource content
    pub async fn read_resource(&self, uri: &str) -> Result<ResourceContent> {
        // Check if resource exists
        {
            let resources = self.resources.read().await;
            if !resources.contains_key(uri) {
                return Err(anyhow!("Resource not found: {}", uri));
            }
        }

        // Try to get from content store
        {
            let content_store = self.content_store.read().await;
            if let Some(content) = content_store.get(uri) {
                return Ok(content.clone());
            }
        }

        // If it's a file:// URI, read from filesystem
        if uri.starts_with("file://") {
            let path = uri.trim_start_matches("file://");
            return self.read_file_resource(path).await;
        }

        // Dynamic resources (context://, cis://, etc.)
        if uri.starts_with("context://") {
            return self.generate_context_resource(uri).await;
        }

        Err(anyhow!("Cannot read resource content: {}", uri))
    }

    /// Update resource content
    pub async fn update_content(&self, uri: &str, content: ResourceContent) -> Result<()> {
        let mut content_store = self.content_store.write().await;

        // Verify resource exists
        {
            let resources = self.resources.read().await;
            if !resources.contains_key(uri) {
                return Err(anyhow!("Resource not found: {}", uri));
            }
        }

        content_store.insert(uri.to_string(), content.clone());
        debug!("Content updated for: {}", uri);

        // Notify subscribers
        drop(content_store);
        self.notify_subscribers(ResourceChangeEvent {
            uri: uri.to_string(),
            change_type: ResourceChangeType::Updated,
            timestamp: chrono::Utc::now(),
            new_content: Some(content),
        })
        .await;

        Ok(())
    }

    /// Subscribe to resource changes
    pub async fn subscribe(&self, uri: &str, subscriber_id: &str) -> Result<String> {
        // Verify resource exists
        {
            let resources = self.resources.read().await;
            if !resources.contains_key(uri) {
                return Err(anyhow!("Resource not found: {}", uri));
            }
        }

        let subscription_id = format!("sub_{}_{}", uri, subscriber_id);
        let subscription = ResourceSubscription {
            id: subscription_id.clone(),
            uri: uri.to_string(),
            subscriber_id: subscriber_id.to_string(),
            created_at: chrono::Utc::now(),
        };

        let mut subscriptions = self.subscriptions.write().await;
        subscriptions.insert(subscription_id.clone(), subscription);

        let mut subscribers = self.subscribers.write().await;
        subscribers
            .entry(uri.to_string())
            .or_insert_with(Vec::new)
            .push(subscriber_id.to_string());

        info!("Subscription created: {} for {}", subscription_id, uri);
        Ok(subscription_id)
    }

    /// Unsubscribe from resource changes
    pub async fn unsubscribe(&self, subscription_id: &str) -> Result<()> {
        let mut subscriptions = self.subscriptions.write().await;

        let subscription = subscriptions
            .remove(subscription_id)
            .ok_or_else(|| anyhow!("Subscription not found: {}", subscription_id))?;

        let uri = subscription.uri.clone();
        let subscriber_id = subscription.subscriber_id.clone();

        drop(subscriptions);

        let mut subscribers = self.subscribers.write().await;
        if let Some(subs) = subscribers.get_mut(&uri) {
            subs.retain(|s| s != &subscriber_id);
            if subs.is_empty() {
                subscribers.remove(&uri);
            }
        }

        info!("Unsubscribed: {}", subscription_id);
        Ok(())
    }

    /// List subscriptions for a resource
    pub async fn list_subscriptions(&self, uri: &str) -> Vec<ResourceSubscription> {
        let subscriptions = self.subscriptions.read().await;
        subscriptions
            .values()
            .filter(|s| s.uri == uri)
            .cloned()
            .collect()
    }

    /// Notify subscribers of resource changes
    async fn notify_subscribers(&self, event: ResourceChangeEvent) {
        let subscribers = self.subscribers.read().await;
        if let Some(subs) = subscribers.get(&event.uri) {
            debug!("Notifying {} subscribers for {}", subs.len(), event.uri);
            // In a real implementation, this would send notifications via WebSocket/SSE
            // For now, we just log
            for sub in subs {
                debug!("Notified subscriber {} of change on {}", sub, event.uri);
            }
        }
    }

    /// Read file resource from filesystem
    async fn read_file_resource(&self, path: &str) -> Result<ResourceContent> {
        let path_obj = Path::new(path);

        if !path_obj.exists() {
            return Err(anyhow!("File not found: {}", path));
        }

        // Determine MIME type
        let mime_type = mime_guess::from_path(path_obj)
            .first_or_octet_stream()
            .to_string();

        // Read content
        let content = tokio::fs::read(path).await?;

        // Check if it's text or binary
        let is_text = mime_type.starts_with("text/")
            || mime_type == "application/json"
            || mime_type == "application/xml";

        let resource_content = if is_text {
            let text = String::from_utf8_lossy(&content).to_string();
            ResourceContent {
                uri: format!("file://{}", path),
                mime_type,
                text: Some(text),
                blob: None,
            }
        } else {
            ResourceContent {
                uri: format!("file://{}", path),
                mime_type,
                text: None,
                blob: Some(base64::encode(&content)),
            }
        };

        Ok(resource_content)
    }

    /// Generate dynamic context resource
    async fn generate_context_resource(&self, uri: &str) -> Result<ResourceContent> {
        match uri {
            "context://current" => {
                // In a real implementation, this would query actual context
                let context = json!({
                    "project_root": std::env::current_dir().unwrap_or_default().display(),
                    "project_type": "rust",
                    "package_manager": "cargo",
                    "git_branch": "feature/1.1.5"
                });

                Ok(ResourceContent {
                    uri: uri.to_string(),
                    mime_type: "application/json".to_string(),
                    text: Some(context.to_string()),
                    blob: None,
                })
            }
            _ => Err(anyhow!("Unknown context resource: {}", uri)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_resource_crud() {
        let manager = ResourceManager::new();

        // Create
        let resource = Resource {
            uri: "test://resource".to_string(),
            name: "Test Resource".to_string(),
            description: Some("A test resource".to_string()),
            mime_type: "text/plain".to_string(),
            metadata: None,
            annotations: None,
        };

        assert!(manager.create_resource(resource.clone()).await.is_ok());

        // Read
        let retrieved = manager.get_resource("test://resource").await.unwrap();
        assert_eq!(retrieved.uri, "test://resource");

        // Update
        let updated = Resource {
            description: Some("Updated description".to_string()),
            ..resource.clone()
        };
        assert!(manager.update_resource("test://resource", updated).await.is_ok());

        // Delete
        assert!(manager.delete_resource("test://resource").await.is_ok());
    }

    #[tokio::test]
    async fn test_subscription() {
        let manager = ResourceManager::new();

        let resource = Resource {
            uri: "test://sub_resource".to_string(),
            name: "Subscription Test".to_string(),
            description: None,
            mime_type: "text/plain".to_string(),
            metadata: None,
            annotations: None,
        };

        manager.create_resource(resource).await.unwrap();

        // Subscribe
        let sub_id = manager.subscribe("test://sub_resource", "client1").await.unwrap();
        assert!(sub_id.starts_with("sub_"));

        // List subscriptions
        let subs = manager.list_subscriptions("test://sub_resource").await;
        assert_eq!(subs.len(), 1);

        // Unsubscribe
        assert!(manager.unsubscribe(&sub_id).await.is_ok());
    }
}
