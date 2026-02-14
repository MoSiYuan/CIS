//! # Matrix Receipts 路由
//!
//! 实现 Receipts API 端点。

use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Json,
};
use serde_json::json;

use crate::matrix::error::MatrixResult;
use crate::matrix::receipts::ReceiptType;
use crate::matrix::routes::auth::authenticate;
use crate::matrix::routes::AppState;

/// POST /_matrix/client/v3/rooms/{roomId}/receipt/{receiptType}/{eventId}
///
/// 发送已读回执。
///
/// ## 规范
///
/// - [POST /_matrix/client/v3/rooms/{roomId}/receipt/{receiptType}/{eventId}](https://spec.matrix.org/v1.11/client-server-api/#post_matrixclientv3roomsroomidreceiptreceipttypeeventid)
///
/// ## 参数
///
/// - `roomId`: 房间 ID
/// - `receiptType`: Receipt 类型 ("m.read", "m.read.private", "m.fully_read")
/// - `eventId`: 事件 ID
///
/// ## 请求体 (可选)
///
/// ```json
/// {
///   "thread_id": "thread_id"  // 可选,用于线程化回执
/// }
/// ```
///
/// ## 说明
///
/// - `m.read`: 标记事件为已读 (显示给其他用户)
/// - `m.read.private`: 私有已读 (不显示给其他用户)
/// - `m.fully_read`: 标记用户已阅读至此点的所有消息
pub async fn send_receipt(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path((room_id, receipt_type, event_id)): Path<(String, String, String)>,
    body: Option<Json<serde_json::Value>>,
) -> MatrixResult<Json<()>> {
    // 认证
    let user = authenticate(&headers, &state.social_store)?;

    // 解析 Receipt 类型
    let receipt_type = ReceiptType::from_str(&receipt_type).ok_or_else(|| {
        MatrixError::InvalidArgument(format!("Unsupported receipt type: {}", receipt_type))
    })?;

    // 处理可选的请求体 (thread_id)
    if let Some(Json(payload)) = body {
        if payload.get("thread_id").is_some() {
            // TODO: 支持线程化回执
            tracing::warn!("Threaded receipts not yet supported");
        }
    }

    // 设置回执
    state
        .receipt_service
        .set_receipt(&user.user_id, &room_id, &event_id, receipt_type)
        .await?;

    tracing::debug!(
        "Receipt set: user={}, room={}, event={}, type={:?}",
        user.user_id,
        room_id,
        event_id,
        receipt_type
    );

    Ok(Json(()))
}

/// GET /_matrix/client/v3/rooms/{roomId}/receipts
///
/// 获取房间所有用户的 Receipt 列表 (可选,非 Matrix 标准端点)
///
/// ## 说明
///
/// 这是一个 CIS 自定义端点,用于查询房间的所有 Receipt。
/// Matrix 规范中 Receipt 信息通过 `/sync` 端点返回。
pub async fn get_receipts(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(room_id): Path<String>,
) -> MatrixResult<Json<serde_json::Value>> {
    // 认证
    let user = authenticate(&headers, &state.social_store)?;

    // 获取房间的所有已读回执
    let read_receipts = state
        .receipt_service
        .get_room_receipts(&room_id, ReceiptType::Read)
        .await;

    // 构建响应
    let mut receipts = serde_json::Map::new();

    for (user_id, receipt) in read_receipts {
        receipts.insert(
            user_id,
            json!({
                "event_id": receipt.event_id,
                "timestamp": receipt.timestamp,
            }),
        );
    }

    Ok(Json(json!({
        "room_id": room_id,
        "receipts": receipts,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};

    // 注意: 这些测试需要完整的测试环境设置
}
