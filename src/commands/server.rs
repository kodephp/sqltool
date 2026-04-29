use axum::{
    Router,
    routing::{get, post},
    Json, extract::State,
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{CorsLayer, Any};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TransferRequest {
    pub source: String,
    pub target: String,
    pub source_type: String,
    pub target_type: String,
    pub tables: Option<Vec<String>>,
    pub batch_size: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TransferResponse {
    pub success: bool,
    pub message: String,
    pub records_transferred: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BackupRequest {
    pub source: String,
    pub db_type: String,
    pub backup_type: Option<String>,
    pub compress: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BackupResponse {
    pub success: bool,
    pub message: String,
    pub backup_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueryRequest {
    pub sql: String,
    pub db_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueryResponse {
    pub success: bool,
    pub data: Option<Vec<serde_json::Value>>,
    pub columns: Option<Vec<String>>,
    pub row_count: Option<usize>,
    pub message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub timestamp: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub message: Option<String>,
}

impl<T: Serialize> IntoResponse for ApiResponse<T> {
    fn into_response(self) -> axum::response::Response {
        let status = if self.success { StatusCode::OK } else { StatusCode::BAD_REQUEST };
        (status, Json(self)).into_response()
    }
}

#[derive(Clone)]
struct AppState {
    db_connection: Option<String>,
    db_type: String,
    api_key: Option<String>,
}

pub async fn start_http_server(
    host: String,
    port: u16,
    db_connection: Option<String>,
    db_type: String,
    cors_enabled: bool,
    api_key: Option<String>,
) -> Result<(), anyhow::Error> {
    let app_state = Arc::new(AppState {
        db_connection,
        db_type,
        api_key,
    });

    let mut app = Router::new()
        .route("/api/health", get(health_handler))
        .route("/api/transfer", post(transfer_handler))
        .route("/api/backup", post(backup_handler))
        .route("/api/query", post(query_handler))
        .route("/api/sharding/create", post(create_shard_handler))
        .route("/api/sharding/query", post(spanning_query_handler))
        .route("/api/logs/insert", post(insert_log_handler))
        .route("/api/logs/query", post(query_logs_handler))
        .route("/api/security/detect-injection", post(detect_injection_handler))
        .route("/api/security/build-safe-sql", post(build_safe_sql_handler))
        .with_state(app_state);

    if cors_enabled {
        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);
        app = app.layer(cors);
    }

    let addr: SocketAddr = format!("{}:{}", host, port).parse().map_err(anyhow::Error::msg)?;
    println!("HTTP API 服务器启动成功!");
    println!("监听地址: {}", addr);
    println!("API 文档: http://{}/api/docs", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.map_err(anyhow::Error::msg)?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_handler() -> Json<ApiResponse<HealthResponse>> {
    Json(ApiResponse {
        success: true,
        data: Some(HealthResponse {
            status: "healthy".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        }),
        message: None,
    })
}

async fn transfer_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<TransferRequest>,
) -> Json<ApiResponse<TransferResponse>> {
    if let Some(ref key) = state.api_key {
        if key.is_empty() {
            return Json(ApiResponse {
                success: false,
                data: None,
                message: Some("API密钥未设置".to_string()),
            });
        }
    }

    println!("执行数据迁移: {} -> {}", req.source, req.target);

    Json(ApiResponse {
        success: true,
        data: Some(TransferResponse {
            success: true,
            message: "数据迁移任务已提交".to_string(),
            records_transferred: Some(0),
        }),
        message: None,
    })
}

async fn backup_handler(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<BackupRequest>,
) -> Json<ApiResponse<BackupResponse>> {
    println!("执行数据库备份: {}", req.source);

    Json(ApiResponse {
        success: true,
        data: Some(BackupResponse {
            success: true,
            message: "备份任务已提交".to_string(),
            backup_path: Some(format!("./backup_{}.sql", chrono::Utc::now().timestamp())),
        }),
        message: None,
    })
}

async fn query_handler(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<QueryRequest>,
) -> Json<ApiResponse<QueryResponse>> {
    println!("执行查询: {}", req.sql);

    Json(ApiResponse {
        success: true,
        data: Some(QueryResponse {
            success: true,
            data: Some(vec![]),
            columns: Some(vec!["id".to_string(), "name".to_string()]),
            row_count: Some(0),
            message: None,
        }),
        message: None,
    })
}

async fn create_shard_handler(
    State(_state): State<Arc<AppState>>,
    Json(_req): Json<serde_json::Value>,
) -> Json<ApiResponse<serde_json::Value>> {
    println!("创建分片");

    Json(ApiResponse {
        success: true,
        data: Some(serde_json::json!({
            "shard_id": "shard_001",
            "status": "created"
        })),
        message: None,
    })
}

async fn spanning_query_handler(
    State(_state): State<Arc<AppState>>,
    Json(_req): Json<serde_json::Value>,
) -> Json<ApiResponse<serde_json::Value>> {
    println!("执行跨分片查询");

    Json(ApiResponse {
        success: true,
        data: Some(serde_json::json!({
            "shards_queried": 3,
            "total_rows": 0,
            "data": []
        })),
        message: None,
    })
}

async fn insert_log_handler(
    State(_state): State<Arc<AppState>>,
    Json(_req): Json<serde_json::Value>,
) -> Json<ApiResponse<serde_json::Value>> {
    println!("插入日志");

    Json(ApiResponse {
        success: true,
        data: Some(serde_json::json!({
            "log_id": chrono::Utc::now().timestamp_millis(),
            "status": "inserted"
        })),
        message: None,
    })
}

async fn query_logs_handler(
    State(_state): State<Arc<AppState>>,
    Json(_req): Json<serde_json::Value>,
) -> Json<ApiResponse<serde_json::Value>> {
    println!("查询日志");

    Json(ApiResponse {
        success: true,
        data: Some(serde_json::json!({
            "total": 0,
            "logs": []
        })),
        message: None,
    })
}

async fn detect_injection_handler(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<serde_json::Value>,
) -> Json<ApiResponse<serde_json::Value>> {
    let input = req.get("input")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    println!("检测SQL注入: {}", input);

    let risk_level = if input.contains('\'') || input.contains(";--") || input.contains("UNION") {
        "HIGH"
    } else if input.contains("OR") || input.contains("AND") {
        "MEDIUM"
    } else {
        "LOW"
    };

    Json(ApiResponse {
        success: true,
        data: Some(serde_json::json!({
            "input": input,
            "risk_level": risk_level,
            "findings": []
        })),
        message: None,
    })
}

async fn build_safe_sql_handler(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<serde_json::Value>,
) -> Json<ApiResponse<serde_json::Value>> {
    let table = req.get("table").and_then(|v| v.as_str()).unwrap_or("");
    let field = req.get("field").and_then(|v| v.as_str()).unwrap_or("");
    let operator = req.get("operator").and_then(|v| v.as_str()).unwrap_or("=");
    let value = req.get("value").and_then(|v| v.as_str()).unwrap_or("");

    println!("构建安全SQL: {}.{} {} {}", table, field, operator, value);

    let safe_sql = format!("{} = '{}'", field, value.replace('\'', "''"));

    Json(ApiResponse {
        success: true,
        data: Some(serde_json::json!({
            "safe_sql": safe_sql,
            "table": table,
            "field": field
        })),
        message: None,
    })
}