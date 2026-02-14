//! # Matrix Typing 路由
//!
//! 实现 Typing API 端点。

use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Json,
};

use crate::matrix::error::MatrixResult;
use crate::matrix::routes::auth::authenticate;
use crate::matrix::routes::AppState;
use crate::matrix::typing::TypingRequest;

/// PUT /_matrix/client/v3/rooms/{roomId}/typing/{userId}
///
/// 设置用户的输入状态。
///
/// ## 规范
///
/// - [PUT /_matrix/client/v3/rooms/{roomId}/typing/{userId}](https://spec.matrix.org/v1.11/client-server-api/#put_matrixclientv3roomsroomidtypinguserid)
///
/// ## 请求体
///
/// ```json
/// {
///   "typing": true,
///   "timeout": 30000
/// }
/// ```
///
/// ## 说明
///
/// - `typing`: true 表示正在输入,false 表示停止输入
/// - `timeout`: 可选,超时时间 (毫秒),默认 30000 (30 秒)
pub async fn send_typing(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path((room_id, user_id)): Path<(String, String)>,
    Json(payload): Json<TypingRequest>,
) -> MatrixResult<Json<()>> {
    // 认证
    let user = authenticate(&headers, &state.social_store)?;

    // 验证 user_id 匹配当前用户
    if user_id != user.user_id {
        return Err(crate::matrix::error::MatrixError::Forbidden(
            "Cannot set typing status for other users".into(),
        ));
    }

    // 设置输入状态
    state
        .typing_service
        .set_typing(&user_id, &room_id, payload)
        .await?;

    Ok(Json(()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};

    // 注意: 这些测试需要完整的测试环境设置
}
