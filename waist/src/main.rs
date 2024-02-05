use axum::{
    extract,
    extract::DefaultBodyLimit,
    handler::Handler,
    http::header,
    response::IntoResponse,
    routing::{get, options},
    Router,
};
pub use axum_macros::debug_handler;
use geojson::GeoJson;
use sqlx::migrate::MigrateDatabase;
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::trace;
use tower_http::{compression::CompressionLayer, limit::RequestBodyLimitLayer};
use tracing::Level;

type SharedServerState = Arc<RwLock<ServerState>>;

struct ServerState {
    json: Option<GeoJson>,
    sqlite: SqlitePool,
}

impl ServerState {
    async fn create_db(db_url: &String) -> SqlitePool {
        if !sqlx::Sqlite::database_exists(&db_url).await.unwrap_or(false) {
            match sqlx::Sqlite::create_database(&db_url).await {
                Ok(_) => tracing::info!("Database created sucessfully"),
                Err(e) => panic!("{}", e),
            }
        }
        Self::build_db_schema(db_url).await
    }

    async fn build_db_schema(db_url: &String) -> SqlitePool {
        let instance = SqlitePool::connect(db_url).await.unwrap();
        let qry = "CREATE TABLE IF NOT EXISTS lines (timestamp DATETIME, json TEXT);";
        let result = sqlx::query(&qry).execute(&instance).await;

        match result {
            Ok(_) => {
                tracing::info!("DB schema created successfully");
            }
            Err(e) => panic!("{}", e),
        }
        instance
    }

    async fn new() -> Self {
        let db_url = String::from("sqlite://sqlite.db");
        let sqlite = Self::create_db(&db_url).await;

        Self { json: None, sqlite }
    }
}

impl Drop for ServerState {
    fn drop(&mut self) {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        runtime.block_on(async { self.sqlite.close().await });
    }
}

async fn options_handler_new() -> impl IntoResponse {
    (
        [
            (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"),
            (header::ACCESS_CONTROL_ALLOW_METHODS, "POST, OPTIONS"),
            (
                header::ACCESS_CONTROL_ALLOW_HEADERS,
                "Origin, X-Requested-With, Content-Type",
            ),
        ],
        "",
    )
}

#[debug_handler]
async fn post_handler_new(
    extract::State(state): extract::State<SharedServerState>,
    extract::Json(payload): extract::Json<GeoJson>,
) -> impl IntoResponse {
    state.write().await.json = Some(payload.clone());

    let pool = &state.write().await.sqlite;

    match &payload {
        GeoJson::Geometry(_) => {}
        GeoJson::Feature(_) => {}
        GeoJson::FeatureCollection(fc) => {
            for feature in &fc.features {
                let result = sqlx::query("INSERT INTO lines (timestamp, json) VALUES (datetime('now'), $1)")
                    .bind(feature.to_string())
                    .execute(pool)
                    .await;
                match result {
                    Ok(_) => {}
                    Err(e) => tracing::error!("DB insert fail: {:?}", e),
                }
            }
        }
    }

    ([(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")], "world")
}

#[derive(sqlx::FromRow)]
struct QueryResult {
    json: sqlx::types::JsonValue,
}

async fn handler_get(
    extract::State(state): extract::State<SharedServerState>,
    extract::Path(_id): extract::Path<String>,
) -> impl IntoResponse {
    let pool = &state.write().await.sqlite;

    let result: Vec<QueryResult> =
        sqlx::query_as("SELECT json FROM lines WHERE timestamp > datetime('now', '-7 day');")
            .fetch_all(pool)
            .await
            .unwrap();

    (
        [
            (header::CONTENT_TYPE, "application/geo+json"),
            (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"),
        ],
        geojson::FeatureCollection {
            bbox: None,
            features: result
                .iter()
                .map(|feature_result| geojson::Feature::from_json_value(feature_result.json.clone()).unwrap())
                .collect(),
            foreign_members: None,
        }
        .to_string(),
    )
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_target(false).compact().init();
    let shared_server_state = Arc::new(RwLock::new(ServerState::new().await));

    let app = Router::new()
        .route("/", get(|| async { "What are you doing here?" }))
        .route(
            "/new",
            options(options_handler_new).post_service(
                post_handler_new
                    .layer((
                        DefaultBodyLimit::disable(),
                        RequestBodyLimitLayer::new(1024 * 1_000 /* ~1mb */),
                    ))
                    .with_state(Arc::clone(&shared_server_state)),
            ),
        )
        .route("/get/:id", get(handler_get).layer(CompressionLayer::new()))
        .layer(
            trace::TraceLayer::new_for_http()
                .make_span_with(trace::DefaultMakeSpan::new().level(Level::INFO))
                .on_response(trace::DefaultOnResponse::new().level(Level::INFO)),
        )
        .with_state(Arc::clone(&shared_server_state));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await.unwrap();
    tracing::info!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
