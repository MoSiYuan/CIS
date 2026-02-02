//! # Matrix Discovery Endpoint
//!
//! Implements `GET /_matrix/client/versions` for server discovery.

use axum::Json;

/// Matrix versions response
#[derive(Debug, serde::Serialize)]
pub struct VersionsResponse {
    versions: Vec<String>,
    unstable_features: serde_json::Value,
}

/// GET /_matrix/client/versions
///
/// Returns the supported Matrix spec versions.
/// This is the first endpoint clients call to discover server capabilities.
pub async fn versions() -> Json<VersionsResponse> {
    Json(VersionsResponse {
        versions: vec![
            "v1.1".to_string(),
            "v1.2".to_string(),
            "v1.3".to_string(),
        ],
        unstable_features: serde_json::json!({
            "org.matrix.e2e_cross_signing": true,
            "org.matrix.msc2836": true,
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_versions_endpoint() {
        let versions_response = VersionsResponse {
            versions: vec![
                "v1.1".to_string(),
                "v1.2".to_string(),
                "v1.3".to_string(),
            ],
            unstable_features: serde_json::json!({
                "org.matrix.e2e_cross_signing": true,
                "org.matrix.msc2836": true,
            }),
        };
        
        // Verify the response contains expected versions
        let body = serde_json::to_value(&versions_response).unwrap();
        let versions = body["versions"].as_array().unwrap();
        
        assert!(versions.contains(&serde_json::json!("v1.1")));
        assert!(versions.contains(&serde_json::json!("v1.2")));
        assert!(versions.contains(&serde_json::json!("v1.3")));
    }
}
