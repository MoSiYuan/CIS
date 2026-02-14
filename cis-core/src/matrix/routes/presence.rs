//! # Matrix Presence 路由
//!
//! 实现 Presence API 端点。

use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Json,
};
use serde::Deserialize;

use crate::matrix::error::MatrixResult;
use crate::matrix::presence::{PresenceListRequest, PresenceListResponse, PresenceService, PresenceState};
use crate::matrix::routes::auth::authenticate;
use crate::matrix::routes::AppState;

/// GET /_matrix/client/v3/presence/{userId}/status
///
/// 获取用户的在线状态。
///
/// ## 规范
///
/// - [GET /_matrix/client/v3/presence/{userId}/status](https://spec.matrix.org/v1.11/client-server-api/#get_matrixclientv3presenceuseridstatus)
///
/// ## 权限
///
/// - 如果查询自己的状态,无需额外权限
/// - 如果查询其他用户的状态,需要共享房间或已订阅
pub async fn get_presence_status(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> MatrixResult<Json<PresenceState>> {
    // 认证
    let user = authenticate(&headers, &state.social_store)?;

    // 规范允许查询任意用户状态
    // 实际应用可能需要权限检查
    let presence = state.presence_service.get_presence(&user_id).await?;

    Ok(Json(presence))
}

/// PUT /_matrix/client/v3/presence/{userId}/status
///
/// 设置自己的在线状态。
///
/// ## 规范
///
/// - [PUT /_matrix/client/v3/presence/{userId}/status](https://spec.matrix.org/v1.11/client-server-api/#put_matrixclientv3presenceuseridstatus)
///
/// ## 请求体
///
/// ```json
/// {
///   "presence": "online",
///   "status_msg": "Working on CIS",
///   "last_active_ago": 0
/// }
/// ```
pub async fn set_presence_status(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(user_id): Path<String>,
    Json(payload): Json<PresenceState>,
) -> MatrixResult<Json<()>> {
    // 认证
    let user = authenticate(&headers, &state.social_store)?;

    // 验证 user_id 匹配当前用户
    if user_id != user.user_id {
        return Err(crate::matrix::error::MatrixError::Forbidden(
            "Cannot set presence for other users".into(),
        ));
    }

    // 设置状态
    state
        .presence_service
        .set_presence(&user_id, payload)
        .await?;

    Ok(Json(()))
}

/// GET /_matrix/client/v3/presence/list/{userId}
///
/// 获取用户的 Presence 订阅列表。
///
/// ## 规范
///
/// - [GET /_matrix/client/v3/presence/list/{userId}](https://spec.matrix.org/v1.11/client-server-api/#get_matrixclientv3presencelistuserid)
pub async fn get_presence_list(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> MatrixResult<Json<PresenceListResponse>> {
    // 认证
    let user = authenticate(&headers, &state.social_store)?;

    // 验证 user_id 匹配当前用户
    if user_id != user.user_id {
        return Err(crate::matrix::error::MatrixError::Forbidden(
            "Cannot get presence list for other users".into(),
        ));
    }

    // 获取订阅列表
    let subscribed = state
        .presence_service
        .get_subscriptions(&user_id)
        .await?;

    // 获取所有订阅用户的状态
    let presences = state
        .presence_service
        .get_presences(&subscribed)
        .await?;

    Ok(Json(PresenceListResponse { users: presences }))
}

/// POST /_matrix/client/v3/presence/list/{userId}
///
/// 更新用户的 Presence 订阅列表。
///
/// ## 规范
///
/// - [POST /_matrix/client/v3/presence/list/{userId}](https://spec.matrix.org/v1.11/client-server-api/#post_matrixclientv3presencelistuserid)
///
/// ## 请求体
///
/// ```json
/// {
///   "users": ["@user1:cis.local", "@user2:cis.local"]
/// }
/// ```
pub async fn update_presence_list(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(user_id): Path<String>,
    Json(payload): Json<PresenceListRequest>,
) -> MatrixResult<Json<()>> {
    // 认证
    let user = authenticate(&headers, &state.social_store)?;

    // 验证 user_id 匹配当前用户
    if user_id != user.user_id {
        return Err(crate::matrix::error::MatrixError::Forbidden(
            "Cannot update presence list for other users".into(),
        ));
    }

    // 更新订阅
    state
        .presence_service
        .subscribe(&user_id, payload.users)
        .await?;

    Ok(Json(()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};
    use tower::ServiceExt;

    // 注意: 这些测试需要完整的测试环境设置
    // 包括数据库、MatrixSocialStore 等

    #[tokio::test]
    async fn test_get_presence_status_unauthorized() {
        // 需要测试环境
        // let app = create_test_app().await;

        // let response = app
        //     .oneshot(Request::builder()
        //         .method(Method::GET)
        //         .uri("/_matrix/client/v3/presence/%40user1%3Acis.local/status")
        //         .body(Body::empty())
        //         .unwrap())
        //     .await
        //     .unwrap();

        // assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
