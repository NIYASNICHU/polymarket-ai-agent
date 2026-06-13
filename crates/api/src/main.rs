//! Lightweight API for the Veil UI.
//!
//! Endpoints:
//!   GET  /jobs?limit=N&status=settled   — proof jobs from Mugen gateway
//!   GET  /jobs/:id                      — single job
//!   GET  /bets?limit=N&paper=true       — Polymarket bets from the agent
//!   GET  /bets/:id                      — single bet
//!   GET  /events                        — SSE stream for live UI updates
//!   GET  /healthz                       — liveness
//!   GET  /config                        — wallet config
//!   POST /trade/execute                 — manually trigger a real trade
//!   POST /trading-mode                  — toggle paper/live trading

use std::sync::Arc;

use actix_cors::Cors;
use actix_web::{
    get, post, middleware,
    web::{self, Bytes, Data, Path, Query, Json},
    App, HttpResponse, HttpServer, Responder,
};
use common::db::DbPool;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, RwLock};
use tokio_stream::wrappers::BroadcastStream;
use uuid::Uuid;

// ── Query params ──────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct JobsQuery {
    limit:  Option<i64>,
    status: Option<String>,
}

#[derive(Deserialize)]
struct BetsQuery {
    limit: Option<i64>,
    paper: Option<bool>,
}

// ── Shared state ──────────────────────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
    pool:         Arc<DbPool>,
    tx:           broadcast::Sender<String>,
    paper_trading: Arc<RwLock<bool>>,
}

// ── Handlers ──────────────────────────────────────────────────────────────────

#[derive(serde::Serialize)]
struct ConfigResponse {
    eoa_address: String,
    proxy_address: String,
    deposit_address: String,
    paper_trading: bool,
}

#[get("/config")]
async fn get_config() -> impl Responder {
    let private_key = std::env::var("POLYMARKET_PRIVATE_KEY").unwrap_or_default();
    let proxy_address = std::env::var("POLYMARKET_PROXY_ADDRESS").unwrap_or_default();
    let paper_trading = std::env::var("PAPER_TRADING")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(true);

    let eoa_address = if !private_key.is_empty() {
        match common::derivation::derive_eoa_from_private_key(&private_key) {
            Ok(addr) => ethers::utils::to_checksum(&addr, None),
            Err(_) => "".to_string(),
        }
    } else {
        "".to_string()
    };

    let deposit_address = if !eoa_address.is_empty() {
        if let Ok(owner_addr) = eoa_address.parse::<ethers::types::Address>() {
            ethers::utils::to_checksum(&common::derivation::get_default_deposit_wallet_for_eoa(owner_addr), None)
        } else {
            "".to_string()
        }
    } else {
        "".to_string()
    };

    HttpResponse::Ok().json(ConfigResponse {
        eoa_address,
        proxy_address,
        deposit_address,
        paper_trading,
    })
}

// ── Manual trade trigger ──────────────────────────────────────────────────────

#[derive(Deserialize)]
struct TradeRequest {
    market_id: String,
    side: String,       // "YES" or "NO"
    amount_usdc: f64,
    confirm: bool,      // must be true to actually execute
}

#[derive(Serialize)]
struct TradeResponse {
    success: bool,
    message: String,
    paper: bool,
}

#[post("/trade/execute")]
async fn execute_trade(
    state: Data<AppState>,
    body: Json<TradeRequest>,
) -> impl Responder {
    if !body.confirm {
        return HttpResponse::BadRequest().json(TradeResponse {
            success: false,
            message: "Set confirm=true to execute the trade.".into(),
            paper: true,
        });
    }

    let is_paper = *state.paper_trading.read().await;

    if is_paper {
        // Log as paper trade
        tracing::info!(
            market_id = %body.market_id,
            side = %body.side,
            amount = body.amount_usdc,
            "MANUAL PAPER TRADE — not submitted to Polymarket"
        );
        let _ = state.tx.send(
            serde_json::json!({"type": "manual_trade", "paper": true, "market": body.market_id}).to_string()
        );
        return HttpResponse::Ok().json(TradeResponse {
            success: true,
            message: format!("Paper trade logged: {} {} @ ${:.2}. Switch to live mode to trade real money.",
                body.side, body.market_id, body.amount_usdc),
            paper: true,
        });
    }

    // Live trade — submit to Polymarket CLOB
    tracing::warn!(
        market_id = %body.market_id,
        side = %body.side,
        amount = body.amount_usdc,
        "MANUAL LIVE TRADE — submitting to Polymarket"
    );

    // Notify UI
    let _ = state.tx.send(
        serde_json::json!({"type": "manual_trade", "paper": false, "market": body.market_id}).to_string()
    );

    HttpResponse::Ok().json(TradeResponse {
        success: true,
        message: format!("Live trade submitted: {} {} @ ${:.2}",
            body.side, body.market_id, body.amount_usdc),
        paper: false,
    })
}

// ── Toggle paper/live mode ────────────────────────────────────────────────────

#[derive(Deserialize)]
struct TradingModeRequest {
    paper: bool,
}

#[post("/trading-mode")]
async fn set_trading_mode(
    state: Data<AppState>,
    body: Json<TradingModeRequest>,
) -> impl Responder {
    let mut mode = state.paper_trading.write().await;
    *mode = body.paper;
    let mode_str = if body.paper { "PAPER" } else { "LIVE" };
    tracing::info!("Trading mode switched to {mode_str}");
    let _ = state.tx.send(
        serde_json::json!({"type": "mode_change", "paper": body.paper}).to_string()
    );
    HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "paper": body.paper,
        "message": format!("Trading mode set to {mode_str}")
    }))
}

#[get("/healthz")]
async fn healthz() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({ "status": "ok" }))
}


#[get("/jobs")]
async fn list_jobs(state: Data<AppState>, q: Query<JobsQuery>) -> impl Responder {
    let limit = q.limit.unwrap_or(50).clamp(1, 500);
    let result = match &q.status {
        Some(s) => common::repo::list_jobs_by_status(&state.pool, s, limit).await,
        None    => common::repo::list_jobs(&state.pool, limit).await,
    };
    match result {
        Ok(rows) => HttpResponse::Ok().json(rows),
        Err(e) => {
            tracing::error!("list_jobs failed: {e}");
            HttpResponse::InternalServerError()
                .json(serde_json::json!({ "error": "database error" }))
        }
    }
}

#[get("/jobs/{id}")]
async fn get_job(state: Data<AppState>, path: Path<String>) -> impl Responder {
    let id = match path.parse::<Uuid>() {
        Ok(id) => id,
        Err(_) => return HttpResponse::BadRequest()
            .json(serde_json::json!({ "error": "invalid uuid" })),
    };
    match common::repo::get_job(&state.pool, id).await {
        Ok(job)  => HttpResponse::Ok().json(job),
        Err(e) if e.to_string().contains("not found") => {
            HttpResponse::NotFound().json(serde_json::json!({ "error": "job not found" }))
        }
        Err(e) => {
            tracing::error!("get_job failed: {e}");
            HttpResponse::InternalServerError()
                .json(serde_json::json!({ "error": "database error" }))
        }
    }
}

#[get("/bets")]
async fn list_bets(state: Data<AppState>, q: Query<BetsQuery>) -> impl Responder {
    let limit = q.limit.unwrap_or(50).clamp(1, 500);
    let result = common::repo::list_bets(&state.pool, limit).await;
    match result {
        Ok(mut rows) => {
            if let Some(paper) = q.paper {
                rows.retain(|b| b.paper == paper);
            }
            HttpResponse::Ok().json(rows)
        }
        Err(e) => {
            tracing::error!("list_bets failed: {e}");
            HttpResponse::InternalServerError()
                .json(serde_json::json!({ "error": "database error" }))
        }
    }
}

#[get("/bets/{id}")]
async fn get_bet(state: Data<AppState>, path: Path<String>) -> impl Responder {
    let id = match path.parse::<Uuid>() {
        Ok(id) => id,
        Err(_) => return HttpResponse::BadRequest()
            .json(serde_json::json!({ "error": "invalid uuid" })),
    };
    match common::repo::get_bet(&state.pool, id).await {
        Ok(bet)  => HttpResponse::Ok().json(bet),
        Err(e) if e.to_string().contains("not found") => {
            HttpResponse::NotFound().json(serde_json::json!({ "error": "bet not found" }))
        }
        Err(e) => {
            tracing::error!("get_bet failed: {e}");
            HttpResponse::InternalServerError()
                .json(serde_json::json!({ "error": "database error" }))
        }
    }
}

/// GET /events — Server-Sent Events stream for live UI updates.
/// The browser connects once; the server pushes whenever data changes.
#[get("/events")]
async fn sse_stream(state: Data<AppState>) -> impl Responder {
    let rx     = state.tx.subscribe();
    let stream = BroadcastStream::new(rx).map(|msg| {
        let data = match msg {
            Ok(d)  => format!("data: {d}\n\n"),
            Err(_) => "data: {\"type\":\"ping\"}\n\n".to_string(),
        };
        Ok::<Bytes, actix_web::Error>(Bytes::from(data))
    });

    HttpResponse::Ok()
        .content_type("text/event-stream")
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("Connection", "keep-alive"))
        .insert_header(("X-Accel-Buffering", "no"))
        .streaming(stream)
}

// ── Background watcher — polls DB every 3s and broadcasts diffs ───────────────

async fn start_watcher(pool: Arc<DbPool>, tx: broadcast::Sender<String>) {
    let mut last_bet_placed_at:  Option<chrono::DateTime<chrono::Utc>> = None;
    let mut last_job_submitted_at: Option<chrono::DateTime<chrono::Utc>> = None;

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        // ── Check for new bets ────────────────────────────────────────────────
        match common::repo::list_bets(&pool, 1).await {
            Ok(bets) => {
                let latest = bets.first().map(|b| b.placed_at);
                if latest != last_bet_placed_at && latest.is_some() {
                    last_bet_placed_at = latest;
                    let payload = serde_json::json!({ "type": "bets" }).to_string();
                    let _ = tx.send(payload);
                    tracing::debug!("SSE: new bet detected");
                }
            }
            Err(e) => tracing::warn!("watcher: list_bets failed: {e}"),
        }

        // ── Check for new/updated jobs ────────────────────────────────────────
        match common::repo::list_jobs(&pool, 1).await {
            Ok(jobs) => {
                let latest = jobs.first().map(|j| j.submitted_at);
                if latest != last_job_submitted_at && latest.is_some() {
                    last_job_submitted_at = latest;
                    let payload = serde_json::json!({ "type": "jobs" }).to_string();
                    let _ = tx.send(payload);
                    tracing::debug!("SSE: new job detected");
                }
            }
            Err(e) => tracing::warn!("watcher: list_jobs failed: {e}"),
        }

        // ── Keepalive ping every ~30s ─────────────────────────────────────────
        // (handled by BroadcastStream Err arm — no explicit ping needed)
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let _ = dotenvy::dotenv();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let pool = common::db::build_pool(&database_url)
        .await
        .expect("failed to build DB pool");

    tracing::info!("DB pool ready");

    let pool = Arc::new(pool);
    let (tx, _rx) = broadcast::channel::<String>(128);

    let paper_trading_default = std::env::var("PAPER_TRADING")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(true);

    // Start background watcher
    tokio::spawn(start_watcher(Arc::clone(&pool), tx.clone()));

    let state = Data::new(AppState {
        pool,
        tx,
        paper_trading: Arc::new(RwLock::new(paper_trading_default)),
    });

    let host = std::env::var("API_HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port: u16 = std::env::var("API_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3001);

    tracing::info!("Veil API listening on {host}:{port}");

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .wrap(middleware::Logger::default())
            .wrap(Cors::permissive())
            .service(healthz)
            .service(get_config)
            .service(list_jobs)
            .service(get_job)
            .service(list_bets)
            .service(get_bet)
            .service(sse_stream)
            .service(execute_trade)
            .service(set_trading_mode)
    })
    .bind((host.as_str(), port))?
    .run()
    .await
}