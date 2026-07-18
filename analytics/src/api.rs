/// Analytics HTTP API — mounts under `/analytics`.
///
/// Endpoints:
///   GET  /analytics/metrics          — aggregated metrics for a date range
///   GET  /analytics/metrics/daily    — daily breakdown
///   GET  /analytics/creators         — top creators
///   GET  /analytics/tippers          — top tippers
///   GET  /analytics/creator/:address — single creator stats
///   GET  /analytics/export/json      — JSON export for a date range
///   GET  /analytics/export/csv       — CSV export for a date range
use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use sqlx::PgPool;

use crate::{
    aggregator::MetricsAggregator,
    collector::MetricsCollector,
    exporter::MetricsExporter,
    models::DateRangeQuery,
};

#[derive(Clone)]
pub struct AnalyticsState {
    pub db: PgPool,
}

pub fn routes(state: AnalyticsState) -> Router {
    Router::new()
        .route("/metrics",          get(get_metrics))
        .route("/metrics/daily",    get(get_daily_metrics))
        .route("/creators",         get(get_top_creators))
        .route("/tippers",          get(get_top_tippers))
        .route("/creator/:address", get(get_creator_stats))
        .route("/export/json",      get(export_json))
        .route("/export/csv",       get(export_csv))
        .with_state(state)
}

// ── handlers ────────────────────────────────────────────────────────────────

async fn get_metrics(
    State(s): State<AnalyticsState>,
    Query(q): Query<DateRangeQuery>,
) -> impl IntoResponse {
    let start = q.start_date.as_deref().unwrap_or("1970-01-01");
    let end   = q.end_date.as_deref().unwrap_or("9999-12-31");
    let agg   = MetricsAggregator::new(s.db);
    match agg.get_period_metrics(start, end).await {
        Ok(m)  => Json(m).into_response(),
        Err(e) => internal(e).into_response(),
    }
}

async fn get_daily_metrics(
    State(s): State<AnalyticsState>,
    Query(q): Query<DateRangeQuery>,
) -> impl IntoResponse {
    // Reuse the collector's per-day query for today or a supplied date.
    let date = q.start_date.as_deref().unwrap_or("today");
    let col  = MetricsCollector::new(s.db);
    match col.get_daily_metrics(date).await {
        Ok(m)  => Json(m).into_response(),
        Err(e) => internal(e).into_response(),
    }
}

async fn get_top_creators(
    State(s): State<AnalyticsState>,
    Query(q): Query<DateRangeQuery>,
) -> impl IntoResponse {
    let limit = q.limit.unwrap_or(10).clamp(1, 100);
    let col   = MetricsCollector::new(s.db);
    match col.get_top_creators(limit).await {
        Ok(rows) => Json(rows).into_response(),
        Err(e)   => internal(e).into_response(),
    }
}

async fn get_top_tippers(
    State(s): State<AnalyticsState>,
    Query(q): Query<DateRangeQuery>,
) -> impl IntoResponse {
    let limit = q.limit.unwrap_or(10).clamp(1, 100);
    let col   = MetricsCollector::new(s.db);
    match col.get_top_tippers(limit).await {
        Ok(rows) => Json(rows).into_response(),
        Err(e)   => internal(e).into_response(),
    }
}

async fn get_creator_stats(
    State(s): State<AnalyticsState>,
    Path(address): Path<String>,
) -> impl IntoResponse {
    let agg = MetricsAggregator::new(s.db);
    match agg.get_creator_stats(&address).await {
        Ok(stats) => Json(stats).into_response(),
        Err(e)    => internal(e).into_response(),
    }
}

async fn export_json(
    State(s): State<AnalyticsState>,
    Query(q): Query<DateRangeQuery>,
) -> impl IntoResponse {
    let start = q.start_date.as_deref().unwrap_or("1970-01-01");
    let end   = q.end_date.as_deref().unwrap_or("9999-12-31");
    let exp   = MetricsExporter::new(s.db);
    match exp.export_json(start, end).await {
        Ok(json_str) => (
            [(header::CONTENT_TYPE, "application/json"),
             (header::CONTENT_DISPOSITION, "attachment; filename=\"metrics.json\"")],
            json_str,
        ).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn export_csv(
    State(s): State<AnalyticsState>,
    Query(q): Query<DateRangeQuery>,
) -> impl IntoResponse {
    let start = q.start_date.as_deref().unwrap_or("1970-01-01");
    let end   = q.end_date.as_deref().unwrap_or("9999-12-31");
    let exp   = MetricsExporter::new(s.db);
    let mut buf = Vec::new();
    match exp.export_csv(start, end, &mut buf).await {
        Ok(()) => (
            [(header::CONTENT_TYPE, "text/csv"),
             (header::CONTENT_DISPOSITION, "attachment; filename=\"metrics.csv\"")],
            buf,
        ).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

fn internal(e: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, format!("internal error: {e}"))
}
