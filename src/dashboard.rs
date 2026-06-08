use axum::{
    extract::{Query, State},
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use mysql::prelude::Queryable;
use mysql::{params, Pool};
use serde::Deserialize;
use serde_json::json;
use std::sync::atomic::Ordering;

use crate::level::LogLevel;
use crate::recent;
use crate::stats::GLOBAL_STATS;
use crate::{logger_levels, root_logger, set_logger_level};

#[derive(Clone)]
struct DashboardState {
    mysql_enabled: bool,
    mysql_url: String,
    mysql_table: String,
}

#[derive(Deserialize)]
struct RecentQuery {
    limit: Option<usize>,
    level: Option<String>,
    q: Option<String>,
}

#[derive(Deserialize)]
struct MysqlQuery {
    limit: Option<usize>,
    level: Option<String>,
    q: Option<String>,
}

#[derive(Deserialize)]
struct GenerateLogRequest {
    level: String,
    message: Option<String>,
}

#[derive(Deserialize)]
struct SetLevelRequest {
    logger: String,
    level: String,
}

#[derive(serde::Serialize)]
struct MysqlLogRow {
    id: u64,
    logger_name: String,
    level: String,
    message: String,
    file: String,
    line: u32,
    thread_id: u64,
    created_at: String,
}

async fn index() -> Html<&'static str> {
    Html(DASHBOARD_HTML)
}

async fn metrics() -> Json<serde_json::Value> {
    let debug = GLOBAL_STATS.debug.load(Ordering::Relaxed);
    let info = GLOBAL_STATS.info.load(Ordering::Relaxed);
    let warn = GLOBAL_STATS.warn.load(Ordering::Relaxed);
    let error = GLOBAL_STATS.error.load(Ordering::Relaxed);
    let fatal = GLOBAL_STATS.fatal.load(Ordering::Relaxed);
    let total = GLOBAL_STATS.total.load(Ordering::Relaxed);
    let runtime = crate::runtime::RUNTIME_STATS.snapshot();
    let loggers: Vec<_> = logger_levels()
        .into_iter()
        .map(|(name, level)| json!({ "name": name, "level": level.as_str() }))
        .collect();

    Json(json!({
        "total": total,
        "debug": debug,
        "info": info,
        "warn": warn,
        "error": error,
        "fatal": fatal,
        "qps": GLOBAL_STATS.qps(),
        "error_rate": GLOBAL_STATS.error_rate(),
        "levels": {
            "DEBUG": debug,
            "INFO": info,
            "WARN": warn,
            "ERROR": error,
            "FATAL": fatal
        },
        "runtime": runtime,
        "loggers": loggers
    }))
}

async fn recent_logs(Query(query): Query<RecentQuery>) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    let level = query.level.as_deref().and_then(LogLevel::from_str);
    let entries = recent::recent(limit, level, query.q.as_deref());
    Json(json!({ "ok": true, "logs": entries }))
}

async fn mysql_logs(
    State(state): State<DashboardState>,
    Query(query): Query<MysqlQuery>,
) -> impl IntoResponse {
    if !state.mysql_enabled {
        return Json(json!({
            "ok": false,
            "error": "MySQL is disabled in bitlog.toml",
            "logs": []
        }));
    }

    let Some(table) = mysql_table_identifier(&state.mysql_table) else {
        return Json(json!({
            "ok": false,
            "error": "invalid MySQL table name in bitlog.toml",
            "logs": []
        }));
    };

    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let level = query.level.unwrap_or_default();
    let keyword = query.q.unwrap_or_default();
    let level_filter = if level.is_empty() { None } else { Some(level) };
    let keyword_filter = if keyword.trim().is_empty() {
        None
    } else {
        Some(format!("%{}%", keyword.trim()))
    };

    let pool = match Pool::new(state.mysql_url.as_str()) {
        Ok(pool) => pool,
        Err(e) => {
            return Json(json!({
                "ok": false,
                "error": format!("MySQL pool error: {}", e),
                "logs": []
            }));
        }
    };

    let mut conn = match pool.get_conn() {
        Ok(conn) => conn,
        Err(e) => {
            return Json(json!({
                "ok": false,
                "error": format!("MySQL connection error: {}", e),
                "logs": []
            }));
        }
    };

    let sql = format!(
        "SELECT id, logger_name, level, message, file, line, thread_id, \
         DATE_FORMAT(created_at, '%Y-%m-%d %H:%i:%s') AS created_at \
         FROM {} \
         WHERE (:level IS NULL OR level = :level) \
           AND (:keyword IS NULL OR message LIKE :keyword OR logger_name LIKE :keyword OR file LIKE :keyword) \
         ORDER BY id DESC \
         LIMIT :limit",
        table
    );

    let rows = match conn.exec_map(
        sql,
        params! {
            "level" => level_filter,
            "keyword" => keyword_filter,
            "limit" => limit as u32,
        },
        |(id, logger_name, level, message, file, line, thread_id, created_at)| MysqlLogRow {
            id,
            logger_name,
            level,
            message,
            file,
            line,
            thread_id,
            created_at,
        },
    ) {
        Ok(rows) => rows,
        Err(e) => {
            return Json(json!({
                "ok": false,
                "error": format!("MySQL query error: {}", e),
                "logs": []
            }));
        }
    };

    Json(json!({ "ok": true, "logs": rows }))
}

fn mysql_table_identifier(table: &str) -> Option<String> {
    let parts: Vec<_> = table.split('.').collect();
    if parts.is_empty() || parts.len() > 2 {
        return None;
    }

    let mut escaped = Vec::with_capacity(parts.len());
    for part in parts {
        if part.is_empty()
            || !part
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        {
            return None;
        }
        escaped.push(format!("`{}`", part));
    }

    Some(escaped.join("."))
}

async fn generate_log(Json(req): Json<GenerateLogRequest>) -> impl IntoResponse {
    let logger = root_logger();
    let message = req
        .message
        .unwrap_or_else(|| format!("manual {} log from dashboard", req.level.to_uppercase()));

    match req.level.to_uppercase().as_str() {
        "DEBUG" => crate::LOG_DEBUG!(logger, "{}", message),
        "INFO" => crate::LOG_INFO!(logger, "{}", message),
        "WARN" => crate::LOG_WARN!(logger, "{}", message),
        "ERROR" => crate::LOG_ERROR!(logger, "{}", message),
        "FATAL" => crate::LOG_FATAL!(logger, "{}", message),
        _ => crate::LOG_INFO!(logger, "{}", message),
    }

    Json(json!({ "ok": true }))
}

async fn set_level(Json(req): Json<SetLevelRequest>) -> impl IntoResponse {
    let Some(level) = LogLevel::from_str(&req.level) else {
        return Json(json!({ "ok": false, "error": "invalid level" }));
    };

    if set_logger_level(&req.logger, level) {
        Json(json!({ "ok": true }))
    } else {
        Json(json!({ "ok": false, "error": "logger not found" }))
    }
}

pub async fn start_dashboard() {
    let config = crate::config::BitLogConfig::load_or_default("bitlog.toml");
    start_dashboard_with_config(config).await;
}

pub async fn start_dashboard_with_config(config: crate::config::BitLogConfig) {
    let state = DashboardState {
        mysql_enabled: config.mysql.enabled,
        mysql_url: config.mysql.url.clone(),
        mysql_table: config.mysql.table.clone(),
    };
    let bind_addr = format!("{}:{}", config.dashboard.host, config.dashboard.port);

    let app = Router::new()
        .route("/", get(index))
        .route("/metrics", get(metrics))
        .route("/logs/recent", get(recent_logs))
        .route("/logs/mysql", get(mysql_logs))
        .route("/logs/generate", post(generate_log))
        .route("/loggers/level", post(set_level))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&bind_addr).await.unwrap();

    println!("Dashboard running on http://{}", bind_addr);

    axum::serve(listener, app).await.unwrap();
}

const DASHBOARD_HTML: &str = r#"<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>BitLog Dashboard</title>
  <style>
    :root {
      --bg: #f3f0ff;
      --panel: #ffffff;
      --panel-2: #f8fafc;
      --text: #111827;
      --muted: #667085;
      --line: #d7deea;
      --accent: #2563eb;
      --debug: #3b82f6;
      --info: #16a34a;
      --warn: #d97706;
      --error: #dc2626;
      --fatal: #7f1d1d;
    }

    * { box-sizing: border-box; }
    body {
      margin: 0;
      font-family: Inter, "Segoe UI", Arial, sans-serif;
      color: var(--text);
      background:
        radial-gradient(circle at 12% 8%, rgba(37, 99, 235, .18), transparent 24%),
        radial-gradient(circle at 88% 4%, rgba(236, 72, 153, .18), transparent 24%),
        radial-gradient(circle at 72% 48%, rgba(22, 163, 74, .13), transparent 22%),
        radial-gradient(circle at 18% 80%, rgba(245, 158, 11, .15), transparent 24%),
        var(--bg);
    }

    header {
      background: rgba(255, 255, 255, .92);
      border-bottom: 1px solid var(--line);
      backdrop-filter: blur(10px);
      position: sticky;
      top: 0;
      z-index: 10;
    }

    .header-inner {
      max-width: 1440px;
      margin: 0 auto;
      padding: 16px 24px;
      display: flex;
      justify-content: space-between;
      gap: 16px;
      align-items: center;
    }

    .brand {
      display: flex;
      align-items: center;
      gap: 12px;
    }

    .mark {
      width: 38px;
      height: 38px;
      border-radius: 8px;
      display: grid;
      place-items: center;
      color: white;
      font-weight: 800;
      background: linear-gradient(135deg, #2563eb, #ec4899 48%, #16a34a);
      box-shadow: 0 10px 24px rgba(37, 99, 235, .25);
    }

    h1 {
      margin: 0;
      font-size: 22px;
      letter-spacing: 0;
    }

    .subtitle {
      color: var(--muted);
      font-size: 13px;
      margin-top: 3px;
    }

    .top-actions {
      display: flex;
      gap: 10px;
      align-items: center;
      flex-wrap: wrap;
      justify-content: flex-end;
    }

    .pill {
      height: 34px;
      display: inline-flex;
      align-items: center;
      gap: 8px;
      border: 1px solid var(--line);
      background: white;
      color: var(--muted);
      border-radius: 999px;
      padding: 0 12px;
      font-size: 13px;
    }

    .dot {
      width: 9px;
      height: 9px;
      border-radius: 50%;
      background: var(--info);
      box-shadow: 0 0 0 4px rgba(22, 163, 74, .14);
    }

    button {
      height: 34px;
      border: 1px solid var(--line);
      background: white;
      border-radius: 7px;
      padding: 0 12px;
      color: var(--text);
      cursor: pointer;
      font-size: 13px;
    }

    button:hover { border-color: var(--accent); }

    main {
      max-width: 1440px;
      margin: 0 auto;
      padding: 24px;
    }

    .cards {
      display: grid;
      grid-template-columns: repeat(5, minmax(0, 1fr));
      gap: 14px;
      margin-bottom: 18px;
    }

    .card, .panel {
      background: rgba(255, 255, 255, .94);
      border: 1px solid var(--line);
      border-radius: 8px;
      box-shadow: 0 10px 28px rgba(16, 24, 40, .06);
    }

    .card {
      min-height: 122px;
      padding: 17px;
      position: relative;
      overflow: hidden;
      border: 0;
      color: white;
    }

    .card::before {
      content: "";
      position: absolute;
      inset: 0;
      background: linear-gradient(135deg, #2563eb, #7c3aed);
      z-index: 0;
    }

    .card::after {
      content: "";
      position: absolute;
      right: -28px;
      top: -28px;
      width: 98px;
      height: 98px;
      border-radius: 50%;
      background: rgba(255, 255, 255, .22);
      z-index: 0;
    }

    .card.total::before { background: linear-gradient(135deg, #2563eb, #9333ea); }
    .card.info::before { background: linear-gradient(135deg, #0891b2, #16a34a); }
    .card.warn::before { background: linear-gradient(135deg, #f59e0b, #ef4444); }
    .card.error::before { background: linear-gradient(135deg, #dc2626, #be123c); }
    .card.fatal::before { background: linear-gradient(135deg, #7f1d1d, #4c1d95); }
    .label {
      position: relative;
      z-index: 1;
      color: rgba(255, 255, 255, .82);
      font-size: 13px;
      margin-bottom: 12px;
    }

    .value {
      position: relative;
      z-index: 1;
      font-size: 32px;
      font-weight: 780;
      line-height: 1;
      color: white;
      text-shadow: 0 2px 12px rgba(0, 0, 0, .16);
    }

    .hint {
      position: relative;
      z-index: 1;
      margin-top: 10px;
      color: rgba(255, 255, 255, .78);
      font-size: 12px;
    }

    .layout {
      display: grid;
      grid-template-columns: 420px minmax(0, 1fr);
      gap: 18px;
      align-items: start;
      margin-bottom: 18px;
    }

    .panel {
      padding: 18px;
    }

    .summary-panel {
      min-height: 246px;
    }

    .query-panel {
      font-size: 16px;
    }

    .query-panel .tab,
    .query-panel button {
      font-size: 15px;
    }

    .query-panel .meta {
      font-size: 14px;
    }

    .query-panel .msg {
      font-size: 16px;
    }

    .panel-title {
      display: flex;
      justify-content: space-between;
      align-items: center;
      gap: 12px;
      margin-bottom: 14px;
    }

    .panel h2 {
      margin: 0;
      font-size: 20px;
      line-height: 1.2;
      font-weight: 800;
      letter-spacing: 0;
      display: inline-flex;
      align-items: center;
      gap: 8px;
    }

    .panel h2::before {
      content: "";
      width: 10px;
      height: 10px;
      border-radius: 3px;
      background: linear-gradient(135deg, #2563eb, #ec4899);
    }

    .small {
      color: var(--muted);
      font-size: 13px;
    }

    .health {
      display: grid;
      gap: 12px;
    }

    .health-line {
      display: flex;
      justify-content: space-between;
      padding: 12px;
      border: 1px solid var(--line);
      border-radius: 7px;
      background: var(--panel-2);
    }

    .bars {
      display: grid;
      gap: 16px;
      margin-top: 14px;
    }

    .bar-row {
      display: grid;
      grid-template-columns: 78px 1fr 68px;
      align-items: center;
      gap: 10px;
      font-size: 16px;
      font-weight: 750;
    }

    .track {
      height: 16px;
      border-radius: 999px;
      background: #edf1f7;
      overflow: hidden;
    }

    .bar {
      height: 100%;
      width: 0%;
      border-radius: inherit;
      transition: width .25s ease;
    }

    .tabs {
      display: flex;
      gap: 8px;
      margin-bottom: 12px;
      flex-wrap: wrap;
    }

    .tab {
      height: 32px;
      border: 1px solid var(--line);
      background: white;
      border-radius: 999px;
      padding: 0 12px;
      font-size: 13px;
      cursor: pointer;
    }

    .tab.active {
      background: linear-gradient(135deg, #2563eb, #7c3aed);
      color: white;
      border-color: var(--accent);
      box-shadow: 0 8px 18px rgba(37, 99, 235, .22);
    }

    .toolbar {
      display: grid;
      grid-template-columns: 150px 1fr 116px;
      gap: 10px;
      margin-bottom: 12px;
      font-size: 15px;
    }

    .export-actions {
      display: flex;
      justify-content: flex-end;
      gap: 10px;
      margin-bottom: 12px;
    }

    select, input {
      width: 100%;
      height: 42px;
      border: 1px solid rgba(14, 165, 233, .28);
      border-radius: 6px;
      background: linear-gradient(180deg, #ffffff, #f0f9ff);
      color: var(--text);
      padding: 0 12px;
      font-size: 15px;
      outline: none;
      box-shadow: 0 4px 12px rgba(14, 165, 233, .08);
    }

    select:focus, input:focus {
      border-color: #38bdf8;
      box-shadow: 0 0 0 3px rgba(56, 189, 248, .16);
    }

    .logs {
      display: grid;
      gap: 8px;
      max-height: 570px;
      overflow: auto;
      padding-right: 4px;
    }

    .log {
      display: grid;
      grid-template-columns: 52px 74px 164px 1fr;
      gap: 10px;
      align-items: start;
      border: 1px solid var(--line);
      border-radius: 7px;
      padding: 10px;
      background: #fff;
      font-size: 15px;
      position: relative;
      overflow: hidden;
      box-shadow: 0 5px 14px rgba(16, 24, 40, .04);
    }

    .row-no {
      display: inline-flex;
      justify-content: center;
      align-items: center;
      width: 34px;
      height: 28px;
      border-radius: 999px;
      background: #eef2ff;
      color: #3730a3;
      font-weight: 800;
      font-size: 13px;
    }

    .log::before {
      content: "";
      position: absolute;
      left: 0;
      top: 0;
      bottom: 0;
      width: 5px;
      background: #94a3b8;
    }

    .log-debug {
      border-color: rgba(59, 130, 246, .32);
      background: linear-gradient(90deg, rgba(59, 130, 246, .14), #fff 38%);
    }
    .log-debug::before { background: var(--debug); }

    .log-info {
      border-color: rgba(22, 163, 74, .32);
      background: linear-gradient(90deg, rgba(22, 163, 74, .14), #fff 38%);
    }
    .log-info::before { background: var(--info); }

    .log-warn {
      border-color: rgba(217, 119, 6, .38);
      background: linear-gradient(90deg, rgba(245, 158, 11, .18), #fff 38%);
    }
    .log-warn::before { background: var(--warn); }

    .log-error {
      border-color: rgba(220, 38, 38, .40);
      background: linear-gradient(90deg, rgba(220, 38, 38, .18), #fff 38%);
    }
    .log-error::before { background: var(--error); }

    .log-fatal {
      border-color: rgba(127, 29, 29, .45);
      background: linear-gradient(90deg, rgba(127, 29, 29, .20), #fff 38%);
    }
    .log-fatal::before { background: var(--fatal); }

    .badge {
      display: inline-flex;
      justify-content: center;
      min-width: 64px;
      padding: 4px 8px;
      border-radius: 999px;
      color: white;
      font-size: 13px;
      font-weight: 700;
      box-shadow: 0 6px 14px rgba(16, 24, 40, .16);
    }

    .meta {
      color: var(--muted);
      line-height: 1.5;
      overflow-wrap: anywhere;
    }

    .msg {
      line-height: 1.5;
      overflow-wrap: anywhere;
    }

    .empty {
      color: var(--muted);
      border: 1px dashed var(--line);
      border-radius: 7px;
      padding: 18px;
      text-align: center;
      background: var(--panel-2);
    }

    .alert {
      border-color: rgba(220, 38, 38, .35);
      background: #fff5f5;
      color: #991b1b;
    }

    .action-btn {
      border: 0;
      color: #075985;
      font-weight: 750;
      background: linear-gradient(135deg, #e0f2fe, #bae6fd);
      box-shadow: 0 8px 18px rgba(14, 165, 233, .14);
      transition: transform .18s ease, box-shadow .18s ease, filter .18s ease;
    }

    .action-btn:hover {
      transform: translateY(-1px);
      background: linear-gradient(135deg, #bae6fd, #a7f3d0);
      filter: saturate(1.04);
      box-shadow: 0 12px 24px rgba(14, 165, 233, .22);
    }

    .export-btn {
      min-width: 108px;
      border: 1px solid rgba(59, 130, 246, .24);
      color: #1d4ed8;
      font-weight: 600;
      background: linear-gradient(135deg, #eff6ff, #dbeafe);
      box-shadow: 0 8px 18px rgba(59, 130, 246, .12);
      transition: transform .18s ease, box-shadow .18s ease, background .18s ease;
    }

    .export-btn:hover {
      transform: translateY(-1px);
      background: linear-gradient(135deg, #dbeafe, #bfdbfe);
      box-shadow: 0 12px 24px rgba(59, 130, 246, .18);
    }

    @media (max-width: 1100px) {
      .cards { grid-template-columns: repeat(2, minmax(0, 1fr)); }
      .layout { grid-template-columns: 1fr; }
    }

    @media (max-width: 680px) {
      .header-inner { align-items: flex-start; flex-direction: column; }
      main { padding: 16px; }
      .cards { grid-template-columns: 1fr; }
      .toolbar { grid-template-columns: 1fr; }
      .log { grid-template-columns: 1fr; }
    }
  </style>
</head>
<body>
  <header>
    <div class="header-inner">
      <div class="brand">
        <div class="mark">B</div>
        <div>
          <h1>BitLog Dashboard</h1>
          <div class="subtitle">实时日志统计、内存日志流和 MySQL 历史查询</div>
        </div>
      </div>
      <div class="top-actions">
        <div class="pill"><span class="dot"></span><span id="statusText">连接中</span></div>
        <div class="pill" id="updated">等待数据</div>
        <button id="pauseBtn">暂停刷新</button>
      </div>
    </div>
  </header>

  <main>
    <section class="cards">
      <div class="card total"><div class="label">总日志数</div><div class="value" id="total">0</div><div class="hint">通过级别过滤后的日志</div></div>
      <div class="card info"><div class="label">QPS</div><div class="value" id="qps">0.00</div><div class="hint">平均每秒日志量</div></div>
      <div class="card warn"><div class="label">WARN</div><div class="value" id="warnCount">0</div><div class="hint">需要关注的异常状态</div></div>
      <div class="card error"><div class="label">ERROR</div><div class="value" id="errorCount">0</div><div class="hint">已发生错误</div></div>
      <div class="card fatal"><div class="label">错误率</div><div class="value" id="errorRate">0.00%</div><div class="hint">ERROR + FATAL 占比</div></div>
    </section>

    <section class="layout">
      <div class="panel summary-panel">
        <div class="panel-title">
          <h2>运行状态</h2>
          <span class="small">每秒刷新</span>
        </div>
        <div class="health">
          <div class="health-line"><span>Dashboard</span><strong id="dashState">正常</strong></div>
          <div class="health-line"><span>错误告警</span><strong id="alertState">正常</strong></div>
          <div class="health-line"><span>异步批次</span><strong id="batchCount">0</strong></div>
        </div>
      </div>

      <div class="panel summary-panel">
        <div class="panel-title">
          <h2>日志级别分布</h2>
          <span class="small" id="levelSummary">0 条</span>
        </div>
        <div class="bars" id="bars"></div>
      </div>
    </section>

    <section class="panel query-panel">
      <div class="panel-title">
        <h2>日志查询</h2>
        <span class="small">内存实时日志 + MySQL 历史日志</span>
      </div>

      <div class="tabs">
        <button class="tab active" data-source="recent">实时日志 <span id="recentTabCount">0</span></button>
        <button class="tab" data-source="mysql">MySQL 历史 <span id="mysqlTabCount">0</span></button>
      </div>

      <div class="toolbar controls">
        <select id="generateLevel">
          <option>INFO</option>
          <option>DEBUG</option>
          <option>WARN</option>
          <option>ERROR</option>
          <option>FATAL</option>
        </select>
        <input id="generateMessage" placeholder="手动生成日志内容">
        <button id="generateBtn" class="action-btn">生成日志</button>
      </div>

      <div class="toolbar controls">
        <select id="loggerSelect"></select>
        <select id="loggerLevel">
          <option>DEBUG</option>
          <option>INFO</option>
          <option>WARN</option>
          <option>ERROR</option>
          <option>FATAL</option>
          <option>OFF</option>
        </select>
        <button id="setLevelBtn" class="action-btn">修改级别</button>
      </div>

      <div class="toolbar">
        <select id="level">
          <option value="">全部级别</option>
          <option>DEBUG</option>
          <option>INFO</option>
          <option>WARN</option>
          <option>ERROR</option>
          <option>FATAL</option>
        </select>
        <input id="query" placeholder="搜索日志内容、logger 名称、文件名">
        <select id="limit">
          <option value="50">50 条</option>
          <option value="100" selected>100 条</option>
          <option value="200">200 条</option>
        </select>
      </div>

      <div class="export-actions">
        <button id="exportJsonBtn" class="export-btn">导出 JSON</button>
        <button id="exportTxtBtn" class="export-btn">导出 TXT</button>
      </div>

      <div class="logs" id="logs"><div class="empty">暂无日志</div></div>
    </section>
  </main>

  <script>
    const colors = {
      DEBUG: 'linear-gradient(135deg, #60a5fa, #2563eb)',
      INFO: 'linear-gradient(135deg, #34d399, #16a34a)',
      WARN: 'linear-gradient(135deg, #fbbf24, #d97706)',
      ERROR: 'linear-gradient(135deg, #fb7185, #dc2626)',
      FATAL: 'linear-gradient(135deg, #991b1b, #4c1d95)'
    };

    const solidColors = {
      DEBUG: '#3b82f6',
      INFO: '#16a34a',
      WARN: '#d97706',
      ERROR: '#dc2626',
      FATAL: '#7f1d1d'
    };

    const labels = ['DEBUG', 'INFO', 'WARN', 'ERROR', 'FATAL'];
    const bars = document.getElementById('bars');
    let paused = false;
    let source = 'recent';
    let currentLogs = [];

    bars.innerHTML = labels.map(level => `
      <div class="bar-row">
        <strong>${level}</strong>
        <div class="track"><div class="bar" id="bar-${level}" style="background:${solidColors[level]}"></div></div>
        <span id="count-${level}">0</span>
      </div>
    `).join('');

    function escapeHtml(value) {
      return String(value ?? '')
        .replaceAll('&', '&amp;')
        .replaceAll('<', '&lt;')
        .replaceAll('>', '&gt;')
        .replaceAll('"', '&quot;')
        .replaceAll("'", '&#039;');
    }

    function formatTime(epochSeconds) {
      return new Date(epochSeconds * 1000).toLocaleString();
    }

    async function loadMetrics() {
      const res = await fetch('/metrics');
      const data = await res.json();
      const errorTotal = Number(data.error || 0) + Number(data.fatal || 0);
      const errorRate = Number(data.error_rate || 0);

      document.getElementById('total').textContent = data.total;
      document.getElementById('qps').textContent = Number(data.qps).toFixed(2);
      document.getElementById('warnCount').textContent = data.warn;
      document.getElementById('errorCount').textContent = errorTotal;
      document.getElementById('errorRate').textContent = (errorRate * 100).toFixed(2) + '%';
      document.getElementById('batchCount').textContent = data.runtime.async_batches;
      document.getElementById('levelSummary').textContent = `${data.total} 条`;
      const alerting = errorTotal >= 10 || errorRate >= 0.10;
      document.getElementById('alertState').textContent = alerting ? '超过阈值' : '正常';
      document.getElementById('alertState').parentElement.classList.toggle('alert', alerting);

      const loggerSelect = document.getElementById('loggerSelect');
      const selectedLogger = loggerSelect.value;
      loggerSelect.innerHTML = data.loggers.map(item => `<option value="${escapeHtml(item.name)}">${escapeHtml(item.name)} (${item.level})</option>`).join('');
      if (selectedLogger) loggerSelect.value = selectedLogger;

      const max = Math.max(1, ...labels.map(level => data.levels[level] || 0));
      for (const level of labels) {
        const count = data.levels[level] || 0;
        document.getElementById(`count-${level}`).textContent = count;
        document.getElementById(`bar-${level}`).style.width = `${Math.max(3, count / max * 100)}%`;
      }
    }

    async function loadLogs() {
      const level = document.getElementById('level').value;
      const q = document.getElementById('query').value.trim();
      const limit = document.getElementById('limit').value;
      const params = new URLSearchParams({ limit });
      if (level) params.set('level', level);
      if (q) params.set('q', q);

      const endpoint = source === 'mysql' ? '/logs/mysql?' : '/logs/recent?';
      const res = await fetch(endpoint + params.toString());
      const data = await res.json();
      const logs = document.getElementById('logs');

      if (!data.ok) {
        logs.innerHTML = `<div class="empty alert">${escapeHtml(data.error || '查询失败')}</div>`;
        currentLogs = [];
        updateSourceCount(0);
        return;
      }

      if (!data.logs.length) {
        logs.innerHTML = '<div class="empty">没有匹配的日志</div>';
        currentLogs = [];
        updateSourceCount(0);
        return;
      }

      currentLogs = data.logs;
      updateSourceCount(data.logs.length);
      logs.innerHTML = data.logs.map((entry, index) => {
        const level = String(entry.level || '').toUpperCase();
        const levelClass = `log-${level.toLowerCase()}`;
        const time = source === 'mysql' ? entry.created_at : formatTime(entry.timestamp);
        const line = entry.line ?? 0;
        const id = source === 'mysql' ? `#${entry.id}` : `#${entry.id}`;
        return `
          <div class="log ${levelClass}">
            <div><span class="row-no">${index + 1}</span></div>
            <div><span class="badge" style="background:${colors[level] || '#475467'}">${level}</span></div>
            <div class="meta">${id}<br>${escapeHtml(time)}<br>${escapeHtml(entry.logger_name)}</div>
            <div class="msg">
              <strong>${escapeHtml(entry.message)}</strong>
              <div class="meta">${escapeHtml(entry.file)}:${line} · thread ${escapeHtml(entry.thread_id)}</div>
            </div>
          </div>
        `;
      }).join('');
    }

    function updateSourceCount(count) {
      if (source === 'mysql') {
        document.getElementById('mysqlTabCount').textContent = count;
      } else {
        document.getElementById('recentTabCount').textContent = count;
      }
    }

    let logTimer;
    function refreshLogsSoon() {
      clearTimeout(logTimer);
      logTimer = setTimeout(loadLogs, 180);
    }

    async function refresh() {
      if (paused) return;
      try {
        await Promise.all([loadMetrics(), loadLogs()]);
        document.getElementById('statusText').textContent = '实时连接';
        document.getElementById('dashState').textContent = '正常';
        document.getElementById('updated').textContent = '最后刷新 ' + new Date().toLocaleTimeString();
      } catch (err) {
        document.getElementById('statusText').textContent = '连接失败';
        document.getElementById('dashState').textContent = '异常';
      }
    }

    document.querySelectorAll('.tab').forEach(tab => {
      tab.addEventListener('click', () => {
        document.querySelectorAll('.tab').forEach(item => item.classList.remove('active'));
        tab.classList.add('active');
        source = tab.dataset.source;
        loadLogs();
      });
    });

    document.getElementById('level').addEventListener('change', loadLogs);
    document.getElementById('limit').addEventListener('change', loadLogs);
    document.getElementById('query').addEventListener('input', refreshLogsSoon);
    document.getElementById('generateBtn').addEventListener('click', async () => {
      const level = document.getElementById('generateLevel').value;
      const message = document.getElementById('generateMessage').value || `manual ${level} log`;
      await fetch('/logs/generate', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ level, message })
      });
      document.getElementById('generateMessage').value = '';
      await refresh();
    });
    document.getElementById('setLevelBtn').addEventListener('click', async () => {
      const logger = document.getElementById('loggerSelect').value;
      const level = document.getElementById('loggerLevel').value;
      await fetch('/loggers/level', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ logger, level })
      });
      await refresh();
    });
    document.getElementById('exportJsonBtn').addEventListener('click', () => {
      download(`bitlog-${source}.json`, JSON.stringify(currentLogs, null, 2), 'application/json');
    });
    document.getElementById('exportTxtBtn').addEventListener('click', () => {
      const text = currentLogs.map(entry => {
        const level = String(entry.level || '').toUpperCase();
        const time = source === 'mysql' ? entry.created_at : formatTime(entry.timestamp);
        return `[${time}][${level}][${entry.logger_name}] ${entry.message} (${entry.file}:${entry.line})`;
      }).join('\n');
      download(`bitlog-${source}.txt`, text, 'text/plain');
    });
    document.getElementById('pauseBtn').addEventListener('click', () => {
      paused = !paused;
      document.getElementById('pauseBtn').textContent = paused ? '继续刷新' : '暂停刷新';
    });

    function download(filename, content, type) {
      const blob = new Blob([content], { type });
      const url = URL.createObjectURL(blob);
      const link = document.createElement('a');
      link.href = url;
      link.download = filename;
      document.body.appendChild(link);
      link.click();
      link.remove();
      URL.revokeObjectURL(url);
    }

    refresh();
    setInterval(refresh, 1000);
  </script>
</body>
</html>
"#;
