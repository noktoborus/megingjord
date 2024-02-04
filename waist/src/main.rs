use axum::{
    extract,
    extract::{DefaultBodyLimit, Json, State},
    handler::Handler,
    http::header,
    response::IntoResponse,
    routing::{get, options},
    Router,
};
use geojson::GeoJson;
use std::sync::{Arc, RwLock};
use tower_http::trace;
use tracing::Level;

use tower_http::{compression::CompressionLayer, limit::RequestBodyLimitLayer};

type SharedServerState = Arc<RwLock<ServerState>>;

struct ServerState {
    json: Option<GeoJson>,
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

async fn post_handler_new(State(state): State<SharedServerState>, Json(payload): Json<GeoJson>) -> impl IntoResponse {
    state.write().unwrap().json = Some(payload);

    ([(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")], "unique_id")
}

async fn handler_get(
    State(state): State<SharedServerState>,
    extract::Path(_id): extract::Path<String>,
) -> impl IntoResponse {
    let jsonstr = if let Some(geojson) = &state.read().unwrap().json {
        geojson.to_string()
    } else {
        "{}".to_string()
    };

    (
        [
            (header::CONTENT_TYPE, "application/geo+json"),
            (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"),
        ],
        jsonstr,
    )
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_target(false).compact().init();
    let shared_server_state = Arc::new(RwLock::new(ServerState { json: None }));

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
