use crate::backend_state::WalletBackendState;
use crate::json_rpc_client::daemon_post;
use crate::solo_pool_sink::SoloPoolSink;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::net::UdpSocket;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::sync::{oneshot, Mutex};
use tokio::time::{interval, Duration, MissedTickBehavior};

/// Set `ARQMA_SOLO_POOL_LOG=1` (or `true`) to print `[solo-pool]` lines to stderr. If unset, logs only in **debug** builds.
fn solo_pool_log_enabled() -> bool {
    match std::env::var("ARQMA_SOLO_POOL_LOG") {
        Ok(ref s) if s == "1" || s.eq_ignore_ascii_case("true") => true,
        Ok(ref s) if s == "0" || s.eq_ignore_ascii_case("false") => false,
        Ok(_) => false,
        Err(_) => cfg!(debug_assertions),
    }
}

fn solo_pool_log(msg: impl AsRef<str>) {
    if !solo_pool_log_enabled() {
        return;
    }
    eprintln!("[solo-pool] {}", msg.as_ref());
}

/// File dumps for suspect XMRig `login` + quick disconnect. Off if `ARQMA_SOLO_POOL_DUMP=0`.
/// If unset, follows the same default as `solo_pool_log` (on in dev / `ARQMA_SOLO_POOL_LOG=1`).
fn solo_pool_file_dump_enabled() -> bool {
    match std::env::var("ARQMA_SOLO_POOL_DUMP") {
        Ok(ref s) if s == "0" || s.eq_ignore_ascii_case("false") => return false,
        Ok(ref s) if s == "1" || s.eq_ignore_ascii_case("true") => return true,
        Ok(_) => return true,
        Err(_) => {}
    }
    solo_pool_log_enabled()
}

/// Ms from pool `login` response to TCP drop — typical XMRig setBlob/err4 disconnect.
const SOLO_POOL_DUMP_EOF_MS: i64 = 8_000;

/// `xmrig::Job::m_blob` is 408 bytes; `setBlob` rejects `size >= sizeof(m_blob)`.
/// Stratum `job.blob` must be the **block hashing blob** (`blockhashing_blob` from daemon), not
/// the full `blocktemplate_blob` (grows with txs and can exceed 408B → login err 4 / "no active pools").
const XMRIG_MAX_STRATUM_BLOB_BYTES: usize = 408;

#[derive(Clone)]
struct LoginResponseSnapshot {
    at_ms: i64,
    peer: String,
    worker: String,
    session: String,
    job_id: String,
    height: u64,
    worker_diff: u64,
    blob_len: usize,
    blob_head200: String,
    blob_tail32: String,
    target: String,
    seed_hash: String,
    next_seed_stratum: String,
}

fn write_solo_pool_login_close_dump(
    config_dir: &str,
    snap: &LoginResponseSnapshot,
    elapsed_ms: i64,
    how: &str,
) -> Result<PathBuf, String> {
    let gui = Path::new(config_dir).join("gui");
    fs::create_dir_all(&gui).map_err(|e| e.to_string())?;
    let file_tag = if how == "EOF" { "eof" } else { "err" };
    let name = format!(
        "xmrig_suspect_login_{}_{}ms_{}.txt",
        snap.at_ms, elapsed_ms, file_tag
    );
    let path = gui.join(&name);
    let body = format!(
    "Arqma-Wallet solo-pool: miner closed TCP soon after a successful stratum `login` response.\n\
     Likely XMRig `login error code: 4` (job.setBlob) — use blob/seed fields below to compare with daemon / another miner.\n\
     \n\
     close_reason={}\n\
     peer={}\n\
     worker={}\n\
     session={}\n\
     stratum job_id={}\n\
     height={}\n\
     worker_diff={}\n\
     target_le_hex={}\n\
     blob_hex_len={}\n\
     blob_head200_hex={}\n\
     blob_tail32_hex={}\n\
     seed_hash={}\n\
     next_seed_stratum={}\n\
     elapsed_after_login_ms={}\n",
    how,
    snap.peer,
    snap.worker,
    snap.session,
    snap.job_id,
    snap.height,
    snap.worker_diff,
    snap.target,
    snap.blob_len,
    snap.blob_head200,
    snap.blob_tail32,
    snap.seed_hash,
    snap.next_seed_stratum,
    elapsed_ms
  );
    fs::write(&path, body).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Short hex fingerprint for logs (XMRig `login error 4` / setBlob is tied to blob + seeds).
fn hex_prefix_suffix(s: &str, head: usize, tail: usize) -> String {
    let b = s.as_bytes();
    if b.len() <= head + tail + 3 {
        return s.to_string();
    }
    format!(
        "{}...{}",
        String::from_utf8_lossy(&b[..head]),
        String::from_utf8_lossy(&b[b.len() - tail..])
    )
}

fn blocktemplate_blob_reject_reason(blob: &str) -> Option<&'static str> {
    let n = blob.len();
    if n < 152 {
        return Some("blocktemplate_blob hex too short (<152)");
    }
    if n > 512 * 1024 {
        return Some("blocktemplate_blob hex too long");
    }
    if n % 2 != 0 {
        return Some("blocktemplate_blob odd hex length");
    }
    if !blob.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Some("blocktemplate_blob non-hex");
    }
    if hex::decode(blob).is_err() {
        return Some("blocktemplate_blob hex decode failed");
    }
    None
}

fn is_persist_session(session_id: &str) -> bool {
    session_id.starts_with("persist-")
}

/// Public-pool-style block templates (“mimic uniform” / same reservation as typical pools).
/// **Always on** in this build: `get_block_template` uses this `reserve_size` only — not read from
/// `pool.mining.uniform` or any wallet UI toggle (legacy key is stripped from JSON on load/save).
pub(crate) const PUBLIC_POOL_TEMPLATE_RESERVE_SIZE: u64 = 8;

/// Drops obsolete `pool.mining.uniform` — template policy is fixed (always mimic public pool layout above).
pub(crate) fn strip_legacy_uniform_pool_option(pool: &mut Value) {
    if let Some(po) = pool.as_object_mut() {
        if let Some(mining) = po.get_mut("mining").and_then(|m| m.as_object_mut()) {
            mining.remove("uniform");
        }
    }
}

fn average_block_effort(blocks: &[Value]) -> f64 {
    let mut sum = 0f64;
    let mut n = 0u32;
    for b in blocks {
        let diff = b.get("diff").and_then(|v| v.as_f64()).unwrap_or(0.);
        let hashes = b.get("hashes").and_then(|v| v.as_f64()).unwrap_or(0.);
        if diff > 0. {
            sum += hashes / diff;
            n += 1;
        }
    }
    if n == 0 {
        0.
    } else {
        ((sum / n as f64) * 100.).round() / 100.
    }
}

#[derive(Clone, Default)]
struct JobState {
    id: String,
    /// Stratum / XMRig: daemon `blockhashing_blob` (or legacy small `blocktemplate_blob` when under 408B).
    blob: String,
    /// Full `blocktemplate_blob` (optional; not sent to miner — used if we later rebuild blocks for `submit_block`).
    template_blob: String,
    /// Byte offset into decoded `blocktemplate_blob` where the 4-byte Stratum nonce is written (`get_block_template`).
    reserved_offset: u64,
    /// `reserve_size` from `get_block_template` (informational; nonce patch uses 4 bytes at `reserved_offset`).
    reserve_size: u64,
    target: String,
    height: u64,
    difficulty: u64,
    seed_hash: String,
    next_seed_hash: String,
    created_ms: i64,
}

#[derive(Clone, Default)]
struct WorkerState {
    session_id: String,
    miner: String,
    last_share_ms: i64,
    last_activity_ms: i64,
    last_retarget_ms: i64,
    difficulty: u64,
    last_job_id: String,
    shares: u64,
    rejects: u64,
    hashes_total: u64,
    share_times_ms: Vec<i64>,
    share_events: Vec<(i64, u64)>,
    hashrate_5min: u64,
    hashrate_1hr: u64,
    hashrate_6hr: u64,
    hashrate_24hr: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct PersistWorker {
    miner: String,
    last_share_ms: i64,
    difficulty: u64,
    shares: u64,
    rejects: u64,
    hashes_total: u64,
    share_times_ms: Vec<i64>,
    share_events: Vec<(i64, u64)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct PersistData {
    workers: Vec<PersistWorker>,
    blocks: Vec<Value>,
}

fn pool_stats_path(config_dir: &str) -> PathBuf {
    Path::new(config_dir)
        .join("gui")
        .join("solo_pool_stats.json")
}

fn pool_stats_db_path(config_dir: &str) -> PathBuf {
    Path::new(config_dir)
        .join("gui")
        .join("solo_pool_stats.sqlite")
}

fn sqlite_open(config_dir: &str) -> Option<Connection> {
    let p = pool_stats_db_path(config_dir);
    if let Some(parent) = p.parent() {
        let _ = fs::create_dir_all(parent);
    }
    Connection::open(p).ok()
}

fn sqlite_ensure_schema(conn: &Connection) {
    let _ = conn.execute_batch(
        "
    CREATE TABLE IF NOT EXISTS workers (
      miner TEXT PRIMARY KEY,
      last_share_ms INTEGER NOT NULL,
      difficulty INTEGER NOT NULL,
      shares INTEGER NOT NULL,
      rejects INTEGER NOT NULL,
      hashes_total INTEGER NOT NULL,
      share_times_json TEXT NOT NULL DEFAULT '[]'
    );
    CREATE TABLE IF NOT EXISTS blocks (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      status INTEGER NOT NULL,
      hash TEXT NOT NULL DEFAULT '',
      height INTEGER NOT NULL DEFAULT 0,
      time_found INTEGER NOT NULL DEFAULT 0,
      miner TEXT NOT NULL DEFAULT '',
      reward INTEGER NOT NULL DEFAULT -1,
      diff INTEGER NOT NULL DEFAULT 0,
      hashes INTEGER NOT NULL DEFAULT 0
    );
    CREATE TABLE IF NOT EXISTS hashrate_samples (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      miner TEXT NOT NULL,
      ts_ms INTEGER NOT NULL,
      hashes INTEGER NOT NULL
    );
    CREATE INDEX IF NOT EXISTS idx_hashrate_samples_miner_ts ON hashrate_samples(miner, ts_ms);
    ",
    );
}

fn load_persisted(config_dir: &str) -> PersistData {
    let Some(conn) = sqlite_open(config_dir) else {
        return PersistData::default();
    };
    sqlite_ensure_schema(&conn);

    let mut out = PersistData::default();
    let mut worker_samples: HashMap<String, Vec<(i64, u64)>> = HashMap::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT miner, ts_ms, hashes
     FROM hashrate_samples
     ORDER BY miner ASC, ts_ms ASC",
    ) {
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        });
        if let Ok(rows) = rows {
            for (miner, ts_ms, hashes_i64) in rows.flatten() {
                worker_samples
                    .entry(miner)
                    .or_default()
                    .push((ts_ms, hashes_i64.max(0) as u64));
            }
        }
    }

    if let Ok(mut stmt) = conn.prepare(
        "SELECT miner, last_share_ms, difficulty, shares, rejects, hashes_total, share_times_json
     FROM workers
     ORDER BY miner ASC",
    ) {
        let rows = stmt.query_map([], |row| {
            let miner: String = row.get(0)?;
            let share_times_json: String = row.get(6)?;
            let share_times_ms =
                serde_json::from_str::<Vec<i64>>(&share_times_json).unwrap_or_default();
            Ok(PersistWorker {
                miner: miner.clone(),
                last_share_ms: row.get::<_, i64>(1)?,
                difficulty: row.get::<_, i64>(2)?.max(0) as u64,
                shares: row.get::<_, i64>(3)?.max(0) as u64,
                rejects: row.get::<_, i64>(4)?.max(0) as u64,
                hashes_total: row.get::<_, i64>(5)?.max(0) as u64,
                share_times_ms,
                share_events: worker_samples.remove(&miner).unwrap_or_default(),
            })
        });
        if let Ok(rows) = rows {
            out.workers = rows.flatten().collect();
        }
    }

    if let Ok(mut stmt) = conn.prepare(
        "SELECT status, hash, height, time_found, miner, reward, diff, hashes
     FROM blocks
     ORDER BY id DESC
     LIMIT 100",
    ) {
        let rows = stmt.query_map([], |row| {
            Ok(json!({
              "status": row.get::<_, i64>(0)?,
              "hash": row.get::<_, String>(1)?,
              "height": row.get::<_, i64>(2)?,
              "timeFound": row.get::<_, i64>(3)?,
              "miner": row.get::<_, String>(4)?,
              "reward": row.get::<_, i64>(5)?,
              "diff": row.get::<_, i64>(6)?,
              "hashes": row.get::<_, i64>(7)?
            }))
        });
        if let Ok(rows) = rows {
            let mut blocks: Vec<Value> = rows.flatten().collect();
            blocks.reverse();
            out.blocks = blocks;
        }
    }

    out
}

fn save_persisted(config_dir: &str, data: &PersistData) {
    let Some(mut conn) = sqlite_open(config_dir) else {
        return;
    };
    sqlite_ensure_schema(&conn);
    let Ok(txn) = conn.transaction() else {
        return;
    };
    let _ = txn.execute("DELETE FROM workers", []);
    let _ = txn.execute("DELETE FROM blocks", []);
    let _ = txn.execute("DELETE FROM hashrate_samples", []);

    for w in &data.workers {
        let share_times_json =
            serde_json::to_string(&w.share_times_ms).unwrap_or_else(|_| "[]".to_string());
        let _ = txn.execute(
      "INSERT OR REPLACE INTO workers (miner, last_share_ms, difficulty, shares, rejects, hashes_total, share_times_json)
       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
      params![
        w.miner,
        w.last_share_ms,
        w.difficulty as i64,
        w.shares as i64,
        w.rejects as i64,
        w.hashes_total as i64,
        share_times_json
      ],
    );
        for (ts_ms, hashes) in &w.share_events {
            let _ = txn.execute(
                "INSERT INTO hashrate_samples (miner, ts_ms, hashes) VALUES (?1, ?2, ?3)",
                params![w.miner, *ts_ms, *hashes as i64],
            );
        }
    }

    for b in &data.blocks {
        let _ = txn.execute(
            "INSERT INTO blocks (status, hash, height, time_found, miner, reward, diff, hashes)
       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                b.get("status").and_then(|v| v.as_i64()).unwrap_or(0),
                b.get("hash").and_then(|v| v.as_str()).unwrap_or(""),
                b.get("height").and_then(|v| v.as_i64()).unwrap_or(0),
                b.get("timeFound").and_then(|v| v.as_i64()).unwrap_or(0),
                b.get("miner").and_then(|v| v.as_str()).unwrap_or(""),
                b.get("reward").and_then(|v| v.as_i64()).unwrap_or(-1),
                b.get("diff").and_then(|v| v.as_i64()).unwrap_or(0),
                b.get("hashes").and_then(|v| v.as_i64()).unwrap_or(0),
            ],
        );
    }
    let _ = txn.commit();
}

fn migrate_json_to_sqlite(config_dir: &str) {
    let Some(conn) = sqlite_open(config_dir) else {
        return;
    };
    sqlite_ensure_schema(&conn);
    let has_any = conn
        .query_row("SELECT EXISTS(SELECT 1 FROM workers LIMIT 1)", [], |r| {
            r.get::<_, i64>(0)
        })
        .unwrap_or(0)
        == 1
        || conn
            .query_row("SELECT EXISTS(SELECT 1 FROM blocks LIMIT 1)", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap_or(0)
            == 1
        || conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM hashrate_samples LIMIT 1)",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap_or(0)
            == 1;
    if has_any {
        return;
    }

    let json_path = pool_stats_path(config_dir);
    let Ok(raw) = fs::read_to_string(&json_path) else {
        return;
    };
    let Ok(data) = serde_json::from_str::<PersistData>(&raw) else {
        return;
    };
    save_persisted(config_dir, &data);
    let migrated_path = json_path.with_extension("json.migrated");
    let _ = fs::rename(&json_path, &migrated_path);
}

fn is_hex_8(s: &str) -> bool {
    s.len() == 8 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn is_hex_64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn is_hex_len(s: &str, n: usize) -> bool {
    s.len() == n && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Some JSON-RPC stacks return integers as floats or strings; `as_u64()` alone may read as 0.
fn json_u64(v: &Value) -> Option<u64> {
    if let Some(n) = v.as_u64() {
        return Some(n);
    }
    if let Some(n) = v.as_i64() {
        if n >= 0 {
            return Some(n as u64);
        }
    }
    if let Some(f) = v.as_f64() {
        if f.is_finite() && f >= 0. {
            return Some(f as u64);
        }
    }
    v.as_str().and_then(|s| s.trim().parse::<u64>().ok())
}

/// XMRig can reject jobs when `next_seed_hash` is an empty string around seed-epoch edges
/// (expects 32-byte hex like `seed_hash`). Mirror common pool behavior: duplicate `seed_hash`.
fn stratum_next_seed_hash(seed_hash: &str, next_seed_hash: &str) -> String {
    if next_seed_hash.is_empty() {
        seed_hash.to_string()
    } else {
        next_seed_hash.to_string()
    }
}

/// Short blob for stratum (typically daemon `blockhashing_blob`); must fit XMRig `Job` buffer.
fn stratum_mining_blob_ok(blob: &str) -> bool {
    let n = blob.len();
    if n < 152 || n > 2 * (XMRIG_MAX_STRATUM_BLOB_BYTES - 1) {
        return false;
    }
    if n % 2 != 0 {
        return false;
    }
    if !blob.bytes().all(|b| b.is_ascii_hexdigit()) {
        return false;
    }
    let b = match hex::decode(blob) {
        Ok(b) => b,
        Err(_) => return false,
    };
    b.len() < XMRIG_MAX_STRATUM_BLOB_BYTES
}

/// XMRig `Client::parseJob` code **4** is `!job.setBlob` — not auth / diff.
/// Rejecting invalid daemon blobs avoids sending a login `job` that XMRig drops with "login error code: 4"
/// and then reconnect loops ("no active pools").
fn blocktemplate_blob_ok(blob: &str) -> bool {
    let n = blob.len();
    // Keep lower bound strict: very short blobs can pass hex checks but still fail
    // XMRig parseJob/setBlob (login error code 4, then "no active pools").
    // 76 bytes (152 hex chars) covers block header + nonce area for RandomX templates.
    // Keep upper bound conservative too: extremely large templates can still be valid
    // JSON-wise, but be rejected by miner-side blob parser.
    if n < 152 || n > 512 * 1024 {
        return false;
    }
    if n % 2 != 0 {
        return false;
    }
    if !blob.bytes().all(|b| b.is_ascii_hexdigit()) {
        return false;
    }
    // Ensure the daemon value is truly decodable hex, not only [0-9a-f] shaped text.
    hex::decode(blob).is_ok()
}

/// Patch the 4-byte Stratum `nonce` (8 hex chars, same byte order as in `blockhashing_blob`) into the full
/// `blocktemplate_blob` at `reserved_offset`, then re-encode hex for `submit_block`.
/// XMRig does not send `blob` on `submit` — this path relays solo-found blocks to `arqmad`.
fn assemble_block_blob_for_submit(
    template_hex: &str,
    reserved_offset: u64,
    nonce_hex: &str,
) -> Result<String, String> {
    if template_hex.is_empty() {
        return Err("empty blocktemplate_blob".into());
    }
    if !is_hex_8(nonce_hex) {
        return Err("nonce must be 8 hex chars".into());
    }
    let mut bytes = hex::decode(template_hex).map_err(|_| "blocktemplate_blob hex decode failed".to_string())?;
    let off = reserved_offset as usize;
    let nonce_bytes = hex::decode(nonce_hex).map_err(|_| "nonce hex decode failed".to_string())?;
    if nonce_bytes.len() != 4 {
        return Err("nonce must decode to 4 bytes".into());
    }
    if off + 4 > bytes.len() {
        return Err(format!(
            "reserved_offset+4 out of range: off={off} len={}",
            bytes.len()
        ));
    }
    bytes[off..off + 4].copy_from_slice(&nonce_bytes);
    Ok(hex::encode(bytes))
}

fn blocktemplate_reserved_offset_ok(
    blob_hex_len: usize,
    reserved_offset: u64,
    reserve_size: u64,
) -> bool {
    // Offsets are byte-based in daemon API. Pool/miner need at least 4 bytes for nonce.
    let blob_bytes = blob_hex_len / 2;
    let nonce_end = reserved_offset.saturating_add(4);
    let reserve_end = reserved_offset.saturating_add(reserve_size);
    nonce_end <= blob_bytes as u64 && reserve_end <= blob_bytes as u64
}

/// Approximate compact stratum target (4-byte LE hex) from difficulty.
/// Electron uses full 256-bit math; here we keep a deterministic monotonic approximation.
fn difficulty_to_target_hex(difficulty: u64) -> String {
    let d = difficulty.max(1);
    // Stratum compact target is 32-bit; using u64::MAX here collapses most
    // practical difficulties to 0xffffffff (effective diff ~1).
    let t = ((u32::MAX as u64) / d).max(1) as u32;
    let le = t.to_le_bytes();
    hex::encode(le)
}

fn passes_compact_target(result_hash: &str, compact_target_le_hex: &str) -> bool {
    if !is_hex_64(result_hash) || !is_hex_8(compact_target_le_hex) {
        return false;
    }
    let target_le = u32::from_le_bytes({
        let mut a = [0u8; 4];
        if let Ok(v) = hex::decode(compact_target_le_hex) {
            if v.len() == 4 {
                a.copy_from_slice(&v);
            }
        }
        a
    });
    let Ok(hash_bytes) = hex::decode(result_hash) else {
        return false;
    };
    if hash_bytes.len() != 32 {
        return false;
    }
    // Different miners/pool stacks disagree on which 32-bit slice/endian should
    // be used for compact target checks. Accept any canonical variant to avoid
    // false "low difficulty share" rejects caused purely by byte-order mismatch.
    let first = [hash_bytes[0], hash_bytes[1], hash_bytes[2], hash_bytes[3]];
    let last = [
        hash_bytes[28],
        hash_bytes[29],
        hash_bytes[30],
        hash_bytes[31],
    ];
    let samples = [
        u32::from_le_bytes(first),
        u32::from_be_bytes(first),
        u32::from_le_bytes(last),
        u32::from_be_bytes(last),
    ];
    samples.into_iter().any(|sample| sample <= target_le)
}

fn pool_enabled(st: &WalletBackendState) -> bool {
    st.config_data
        .get("pool")
        .and_then(|p| p.get("server"))
        .and_then(|s| s.get("enabled"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

pub fn preferred_bind_ip() -> String {
    let Ok(sock) = UdpSocket::bind("0.0.0.0:0") else {
        return "127.0.0.1".to_string();
    };
    let _ = sock.connect("8.8.8.8:80");
    sock.local_addr()
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string())
}

fn pool_mining_address(st: &WalletBackendState) -> String {
    st.config_data
        .get("pool")
        .and_then(|p| p.get("mining"))
        .and_then(|m| m.get("address"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn pool_bind_addr(st: &WalletBackendState) -> Option<String> {
    let raw_ip = st
        .config_data
        .get("pool")
        .and_then(|p| p.get("server"))
        .and_then(|s| s.get("bindIP"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let ip = if raw_ip.is_empty() || raw_ip == "0.0.0.0" || raw_ip == "127.0.0.1" {
        preferred_bind_ip()
    } else {
        raw_ip.to_string()
    };
    let port = st
        .config_data
        .get("pool")
        .and_then(|p| p.get("server"))
        .and_then(|s| s.get("bindPort"))
        .and_then(|v| v.as_u64())
        .unwrap_or(3333);
    Some(format!("{ip}:{port}"))
}

pub(crate) fn daemon_host_port(st: &WalletBackendState) -> Option<(String, u16)> {
    let net = st
        .config_data
        .get("app")
        .and_then(|a| a.get("net_type"))
        .and_then(|v| v.as_str())
        .unwrap_or("mainnet");
    let cfg = st.config_data.get("daemons")?.get(net)?;
    let typ = cfg.get("type").and_then(|v| v.as_str()).unwrap_or("remote");
    if typ == "remote" {
        let host = cfg.get("remote_host").and_then(|v| v.as_str())?.to_string();
        let port = cfg.get("remote_port").and_then(|v| v.as_u64())? as u16;
        Some((host, port))
    } else {
        let host = cfg
            .get("rpc_bind_ip")
            .and_then(|v| v.as_str())
            .unwrap_or("127.0.0.1")
            .to_string();
        let port = cfg
            .get("rpc_bind_port")
            .and_then(|v| v.as_u64())
            .unwrap_or(19994) as u16;
        Some((host, port))
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn sanitize_worker_name(raw: &str) -> String {
    let mut out = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else if ch.is_ascii_whitespace() {
            out.push('-');
        }
    }
    if out.is_empty() {
        "Unnamed_Worker".to_string()
    } else {
        out
    }
}

fn worker_job_id(base_job_id: &str, difficulty: u64) -> String {
    format!("{base_job_id}|{difficulty}")
}

fn canonical_job_id(job_id: &str) -> &str {
    job_id.split('|').next().unwrap_or(job_id)
}

fn calc_hashrate(hashes: u64, ms: i64) -> u64 {
    if hashes == 0 || ms <= 0 {
        return 0;
    }
    ((hashes as f64) / ((ms as f64) / 1000.0)) as u64
}

fn prune_share_events(ws: &mut WorkerState, now: i64) {
    ws.share_events
        .retain(|(ts, _)| now.saturating_sub(*ts) <= 24 * 60 * 60 * 1000);
}

fn window_hashrate(ws: &WorkerState, now: i64, window_ms: i64) -> u64 {
    if window_ms <= 0 {
        return 0;
    }
    let hashes: u64 = ws
        .share_events
        .iter()
        .filter(|(ts, _)| now.saturating_sub(*ts) <= window_ms)
        .map(|(_, h)| *h)
        .sum();
    calc_hashrate(hashes, window_ms)
}

fn build_hashrate_graph(ws: &WorkerState, now: i64, bucket_ms: i64, buckets: i64) -> Value {
    let mut out = serde_json::Map::new();
    for i in 0..buckets {
        let bucket_end = now - (i * bucket_ms);
        let bucket_start = bucket_end - bucket_ms;
        let hashes: u64 = ws
            .share_events
            .iter()
            .filter(|(ts, _)| *ts > bucket_start && *ts <= bucket_end)
            .map(|(_, h)| *h)
            .sum();
        let rate = calc_hashrate(hashes, bucket_ms);
        out.insert(bucket_start.to_string(), json!(rate));
    }
    Value::Object(out)
}

fn abs_diff_u64(a: u64, b: u64) -> u64 {
    if a >= b {
        a - b
    } else {
        b - a
    }
}

/// VarDiff: interval-based retarget (share spacing) blended with a **MoneroOcean-style** estimate
/// `sum(share_difficulty) * targetTime / windowSeconds` over recent `share_events`, so low-hashrate
/// miners still move toward a difficulty where a block (network target) remains possible given RandomARQ latency.
fn maybe_retarget(
    ws: &mut WorkerState,
    now: i64,
    var_enabled: bool,
    retarget_time_s: u64,
    target_time_s: u64,
    variance_percent: u64,
    min_diff: u64,
    max_diff: u64,
    max_jump_percent: u64,
) -> bool {
    // Need at least two inter-share samples (nodejs-pool often uses longer history; we stay lighter).
    if !var_enabled || ws.share_times_ms.len() < 2 {
        return false;
    }
    let retarget_ms = (retarget_time_s as i64).saturating_mul(1000);
    if retarget_ms > 0 && now.saturating_sub(ws.last_retarget_ms) < retarget_ms {
        return false;
    }
    // Recent spacing between accepted shares (ms).
    let n_time = ws.share_times_ms.len().min(12);
    let slice_time = &ws.share_times_ms[ws.share_times_ms.len() - n_time..];
    let mut avg_ms = slice_time.iter().copied().sum::<i64>() / (n_time as i64);
    if avg_ms <= 0 {
        return false;
    }
    // Avoid explosive upward jumps when a single very fast share arrives after a long silence.
    avg_ms = avg_ms.max(300);
    let target_ms = (target_time_s as i64) * 1000;
    let variance_ms = (target_ms * variance_percent as i64) / 100;
    let min_target = target_ms - variance_ms;
    let max_target = target_ms + variance_ms;
    let in_variance_band = avg_ms >= min_target && avg_ms <= max_target;

    // Rate-based implied difficulty (similar to `miner.hashes * targetTime / period` in MoneroOcean).
    const RATE_WINDOW_MS: i64 = 120_000;
    let mut sum_d: u64 = 0;
    let mut t_min = i64::MAX;
    let mut t_max = i64::MIN;
    for (ts, d) in &ws.share_events {
        if now.saturating_sub(*ts) <= RATE_WINDOW_MS {
            sum_d = sum_d.saturating_add(*d);
            t_min = t_min.min(*ts);
            t_max = t_max.max(*ts);
        }
    }
    let span_ms = (t_max.saturating_sub(t_min)).max(1000);
    let diff_from_rate = if sum_d > 0 && ws.share_events.len() >= 2 {
        ((sum_d as f64) * (target_time_s as f64) / (span_ms as f64 / 1000.0)) as u64
    } else {
        0
    };

    let raw_interval = ((ws.difficulty as f64) * (target_ms as f64) / (avg_ms as f64)) as u64;
    let raw = if diff_from_rate > 0 {
        let a = diff_from_rate.max(1) as f64;
        let b = raw_interval.max(1) as f64;
        // Geometric mean: agree with both spacing and aggregate work rate (stable for rx/arq).
        ((a * b).sqrt()) as u64
    } else {
        raw_interval
    };

    if in_variance_band {
        // In the OK time band: only retarget if the rate-based view disagrees materially (stale difficulty).
        if diff_from_rate == 0 || ws.difficulty == 0 {
            ws.last_retarget_ms = now;
            return false;
        }
        let drift_permille = (abs_diff_u64(diff_from_rate, ws.difficulty) as u128)
            .saturating_mul(1000)
            / ws.difficulty.max(1) as u128;
        // ~18% minimum change (same order as MoneroOcean `setNewDiff` 0.2 ratio) to limit thrashing.
        if drift_permille < 180 {
            ws.last_retarget_ms = now;
            return false;
        }
    }

    let prev = ws.difficulty;
    // `100 - maxJump` on u64 underflows for jump>100, which breaks downward retargets.
    let jump = max_jump_percent.max(1).min(1_000_000);
    let d = ws.difficulty;
    let min_step = if jump < 100 {
        d.saturating_mul(100 - jump) / 100
    } else {
        0u64
    };
    let max_mul = 100u128.saturating_add(jump as u128);
    let max_step = (d as u128).saturating_mul(max_mul).saturating_div(100) as u64;
    let lo = min_step.max(min_diff);
    let hi = max_step.min(max_diff);
    let mut next = if lo <= hi {
        raw.clamp(lo, hi)
    } else {
        raw.clamp(min_diff, max_diff)
    };

    // Allow faster drops when shares are *very* slow (solo UX); still respect min_diff / jump floor.
    if avg_ms > max_target.saturating_mul(3) {
        let aggressive = ((d as f64) * (target_ms as f64) / (avg_ms as f64)) as u64;
        next = next.min(aggressive).max(min_diff);
        if lo <= hi {
            next = next.clamp(lo, hi);
        } else {
            next = next.clamp(min_diff, max_diff);
        }
    }

    // Final deadband outside the variance band: skip microscopic nudges.
    if !in_variance_band && prev > 0 {
        let delta_pm = (abs_diff_u64(next, prev) as u128).saturating_mul(1000) / prev as u128;
        if delta_pm < 120 {
            ws.last_retarget_ms = now;
            return false;
        }
    }

    let changed = next != prev;
    if changed {
        ws.difficulty = next;
        ws.last_retarget_ms = now;
        ws.share_times_ms.clear();
    } else {
        ws.last_retarget_ms = now;
    }
    changed
}

async fn refresh_job(
    http: &reqwest::Client,
    daemon: &(String, u16),
    wallet_address: &str,
    job: &Arc<Mutex<JobState>>,
    job_ring: &Arc<Mutex<Vec<JobState>>>,
    seq: &AtomicU64,
    reserve_size_requested: u64,
) {
    if wallet_address.is_empty() {
        return;
    }
    let rr = reserve_size_requested.clamp(1, 255);
    let params = json!({
      "wallet_address": wallet_address,
      "reserve_size": rr
    });
    let Ok(v) = daemon_post(http, &daemon.0, daemon.1, "get_block_template", 0, &params).await
    else {
        solo_pool_log("get_block_template: RPC request failed (daemon unreachable or HTTP error)");
        return;
    };
    if let Some(err) = v.get("error") {
        solo_pool_log(&format!("get_block_template: JSON-RPC error field: {err}"));
        return;
    }
    let Some(r) = v.get("result") else {
        solo_pool_log("get_block_template: missing result");
        return;
    };
    let template_full = r
        .get("blocktemplate_blob")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    if let Some(reason) = blocktemplate_blob_reject_reason(&template_full) {
        solo_pool_log(&format!(
            "reject template: {reason} (hex_len={})",
            template_full.len()
        ));
        return;
    }
    if !blocktemplate_blob_ok(&template_full) {
        solo_pool_log(
            "reject template: blocktemplate_blob_ok false after reject_reason (internal)",
        );
        return;
    }
    let reserved_offset = r.get("reserved_offset").and_then(json_u64).unwrap_or(0);
    let reserve_size = r.get("reserve_size").and_then(json_u64).unwrap_or(1);
    if !blocktemplate_reserved_offset_ok(template_full.len(), reserved_offset, reserve_size) {
        solo_pool_log(&format!(
      "reject template: reserved area out of range hex_len={} reserved_offset={} reserve_size={}",
      template_full.len(),
      reserved_offset,
      reserve_size
    ));
        return;
    }
    let hashing = r
        .get("blockhashing_blob")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let stratum_blob = if !hashing.is_empty() && stratum_mining_blob_ok(&hashing) {
        hashing
    } else if !hashing.is_empty() {
        solo_pool_log(&format!(
      "reject template: blockhashing_blob invalid for XMRig (hex_len={}); see XMRIG_MAX_STRATUM_BLOB_BYTES",
      hashing.len()
    ));
        return;
    } else if stratum_mining_blob_ok(&template_full) {
        solo_pool_log(&format!(
      "template: no blockhashing_blob; using small blocktemplate_blob for stratum (hex_len={})",
      template_full.len()
    ));
        template_full.clone()
    } else {
        solo_pool_log(&format!(
      "reject template: need blockhashing_blob for stratum — blocktemplate_blob hex_len={} (>= {}B decoded) exceeds XMRig Job buffer; miningcore/Arqma RPC always provides blockhashing_blob",
      template_full.len(),
      XMRIG_MAX_STRATUM_BLOB_BYTES
    ));
        return;
    };
    let blob = stratum_blob;
    let height = r.get("height").and_then(json_u64).unwrap_or(0);
    let difficulty = r.get("difficulty").and_then(json_u64).unwrap_or(0);
    let seed_hash = r
        .get("seed_hash")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let next_seed_hash = r
        .get("next_seed_hash")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    if !is_hex_64(&seed_hash) {
        solo_pool_log(&format!(
            "reject template: seed_hash must be 64 hex chars, got len={}",
            seed_hash.len()
        ));
        return;
    }
    // Some daemons may briefly omit next_seed_hash around updates; keep it optional.
    if !next_seed_hash.is_empty() && !is_hex_len(&next_seed_hash, 64) {
        solo_pool_log(&format!(
            "reject template: next_seed_hash invalid (len={})",
            next_seed_hash.len()
        ));
        return;
    }
    let id = format!("{:x}", seq.fetch_add(1, Ordering::SeqCst));
    let seed_prefix: String = seed_hash.chars().take(12).collect();
    let next_e = next_seed_hash.is_empty();
    solo_pool_log(&format!(
    "template OK job_id={} height={} network_diff={} stratum_blob_hex_len={} template_hex_len={} blob_fp={} reserved_off={} daemon_res_sz={} reserve_req={} next_seed_empty={} seed_prefix={}",
    id,
    height,
    difficulty,
    blob.len(),
    template_full.len(),
    hex_prefix_suffix(&blob, 16, 16),
    reserved_offset,
    reserve_size,
    rr,
    next_e,
    seed_prefix
  ));
    let mut j = job.lock().await;
    j.id = id;
    j.blob = blob;
    j.template_blob = template_full;
    j.reserved_offset = reserved_offset;
    j.reserve_size = reserve_size;
    j.target = difficulty_to_target_hex(difficulty);
    j.height = height;
    j.difficulty = difficulty;
    j.seed_hash = seed_hash;
    j.next_seed_hash = next_seed_hash;
    j.created_ms = now_ms();
    let mut ring = job_ring.lock().await;
    ring.push(j.clone());
    if ring.len() > 8 {
        let keep_from = ring.len().saturating_sub(8);
        ring.drain(0..keep_from);
    }
}

async fn send_line(
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
    payload: &Value,
) -> Result<(), String> {
    let s = format!("{payload}\n");
    writer
        .write_all(s.as_bytes())
        .await
        .map_err(|e| e.to_string())
}

pub fn start<S: SoloPoolSink + Clone + Send + 'static>(sink: S, st: &mut WalletBackendState) {
    stop(st);
    if !pool_enabled(st) {
        return;
    }
    let mining_address = pool_mining_address(st);
    if mining_address.is_empty() {
        sink.emit_receive("set_pool_data", json!({ "status": -1 }));
        return;
    }
    let Some(addr) = pool_bind_addr(st) else {
        return;
    };
    let Some(daemon) = daemon_host_port(st) else {
        return;
    };
    let fixed_diff_separator = st
        .config_data
        .get("pool")
        .and_then(|p| p.get("varDiff"))
        .and_then(|v| v.get("fixedDiffSeparator"))
        .and_then(|v| v.as_str())
        .unwrap_or(".")
        .to_string();
    let config_dir = st.paths.config_dir.clone();
    migrate_json_to_sqlite(&config_dir);
    let persisted = load_persisted(&config_dir);
    // VarDiff is always on (no UI toggle); config `enabled` is ignored.
    let var_enabled = true;
    let var_start_diff = st
        .config_data
        .get("pool")
        .and_then(|p| p.get("varDiff"))
        .and_then(|v| v.get("startDiff"))
        .and_then(|v| v.as_u64())
        .unwrap_or(50_000)
        .clamp(1000, 100_000_000);
    let var_retarget_time_s = st
        .config_data
        .get("pool")
        .and_then(|p| p.get("varDiff"))
        .and_then(|v| v.get("retargetTime"))
        .and_then(|v| v.as_u64())
        .unwrap_or(45);
    let var_target_time_s = st
        .config_data
        .get("pool")
        .and_then(|p| p.get("varDiff"))
        .and_then(|v| v.get("targetTime"))
        .and_then(|v| v.as_u64())
        .unwrap_or(45);
    let var_variance_percent = st
        .config_data
        .get("pool")
        .and_then(|p| p.get("varDiff"))
        .and_then(|v| v.get("variancePercent"))
        .and_then(|v| v.as_u64())
        .unwrap_or(30);
    let var_min_diff = st
        .config_data
        .get("pool")
        .and_then(|p| p.get("varDiff"))
        .and_then(|v| v.get("minDiff"))
        .and_then(|v| v.as_u64())
        .unwrap_or(25_000);
    let var_max_diff = st
        .config_data
        .get("pool")
        .and_then(|p| p.get("varDiff"))
        .and_then(|v| v.get("maxDiff"))
        .and_then(|v| v.as_u64())
        .unwrap_or(10_000_000);
    let var_max_jump_percent = st
        .config_data
        .get("pool")
        .and_then(|p| p.get("varDiff"))
        .and_then(|v| v.get("maxJump"))
        .and_then(|v| v.as_u64())
        .unwrap_or(150);
    let miner_timeout_ms = st
        .config_data
        .get("pool")
        .and_then(|p| p.get("mining"))
        .and_then(|m| m.get("minerTimeout"))
        .and_then(|v| v.as_u64())
        .unwrap_or(900)
        .saturating_mul(1000)
        .max(60_000) as i64;
    let (tx, mut rx) = oneshot::channel::<()>();
    st.solo_pool_shutdown = Some(tx);
    let sink = sink.clone();
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    st.solo_pool_task = Some(tokio::spawn(async move {
        // Windows may keep socket in a transient state right after stop/start toggle.
        // Retry binding briefly before failing with error 10048.
        let mut last_bind_err = String::new();
        let mut listener_opt = None;
        for _ in 0..20 {
            match TcpListener::bind(&addr).await {
                Ok(l) => {
                    listener_opt = Some(l);
                    break;
                }
                Err(e) => {
                    last_bind_err = e.to_string();
                    tokio::time::sleep(Duration::from_millis(150)).await;
                }
            }
        }
        let Some(listener) = listener_opt else {
            eprintln!("[solo-pool] TcpListener::bind({addr}) failed: {last_bind_err}");
            sink.emit_receive(
                "set_pool_data",
                json!({
                  "status": -1,
                  "stats": { "activeWorkers": 0 },
                  "workers": []
                }),
            );
            // OS error strings (e.g. invalid address) are locale-dependent; keep UI copy in English.
            sink.emit_receive(
        "show_notification",
        json!({
          "type": "negative",
          "message": "Solo pool could not start: the bind IP or port is invalid, not available, or already in use. Check Solo Pool bind IP and port in settings.",
          "timeout": 5000
        }),
      );
            return;
        };

        solo_pool_log(&format!(
            "listening on {addr} (stderr: [solo-pool]; release build: set ARQMA_SOLO_POOL_LOG=1)"
        ));

        let workers: Arc<Mutex<HashMap<String, WorkerState>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let senders: Arc<Mutex<HashMap<String, mpsc::UnboundedSender<Value>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let current_job: Arc<Mutex<JobState>> = Arc::new(Mutex::new(JobState::default()));
        let job_ring: Arc<Mutex<Vec<JobState>>> = Arc::new(Mutex::new(Vec::new()));
        let blocks: Arc<Mutex<Vec<Value>>> = Arc::new(Mutex::new(Vec::new()));
        {
            let mut w = workers.lock().await;
            for (idx, pw) in persisted.workers.iter().enumerate() {
                let sid = format!("persist-{idx}-{}", pw.miner);
                w.insert(
                    sid.clone(),
                    WorkerState {
                        session_id: sid,
                        miner: pw.miner.clone(),
                        last_share_ms: pw.last_share_ms,
                        last_activity_ms: pw.last_share_ms,
                        last_retarget_ms: 0,
                        difficulty: pw.difficulty.clamp(var_min_diff, var_max_diff),
                        last_job_id: String::new(),
                        shares: pw.shares,
                        rejects: pw.rejects,
                        hashes_total: pw.hashes_total,
                        share_times_ms: pw.share_times_ms.clone(),
                        share_events: pw.share_events.clone(),
                        hashrate_5min: 0,
                        hashrate_1hr: 0,
                        hashrate_6hr: 0,
                        hashrate_24hr: 0,
                    },
                );
            }
        }
        {
            let mut b = blocks.lock().await;
            *b = persisted.blocks;
        }
        let session_seq = Arc::new(AtomicU64::new(1));
        let job_seq = Arc::new(AtomicU64::new(1));
        let round_hashes = Arc::new(AtomicU64::new(0));
        // Miners that timed out: last WorkerState for the table until the same name reconnects.
        let last_disconnected: Arc<Mutex<HashMap<String, WorkerState>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let mut prev_job_id = String::new();
        refresh_job(
            &http,
            &daemon,
            &mining_address,
            &current_job,
            &job_ring,
            job_seq.as_ref(),
            PUBLIC_POOL_TEMPLATE_RESERVE_SIZE,
        )
        .await;
        let mut beat = interval(Duration::from_secs(5));
        beat.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut persist_tick = interval(Duration::from_secs(30));
        persist_tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            tokio::select! {
              _ = &mut rx => {
                let workers_snapshot = {
                  let w = workers.lock().await;
                  w.values().map(|ws| PersistWorker {
                    miner: ws.miner.clone(),
                    last_share_ms: ws.last_share_ms,
                    difficulty: ws.difficulty,
                    shares: ws.shares,
                    rejects: ws.rejects,
                    hashes_total: ws.hashes_total,
                    share_times_ms: ws.share_times_ms.clone(),
                    share_events: ws.share_events.clone()
                  }).collect::<Vec<_>>()
                };
                let blocks_snapshot = { blocks.lock().await.clone() };
                save_persisted(&config_dir, &PersistData { workers: workers_snapshot, blocks: blocks_snapshot });
                break;
              }
              _ = persist_tick.tick() => {
                let workers_snapshot = {
                  let w = workers.lock().await;
                  w.values().map(|ws| PersistWorker {
                    miner: ws.miner.clone(),
                    last_share_ms: ws.last_share_ms,
                    difficulty: ws.difficulty,
                    shares: ws.shares,
                    rejects: ws.rejects,
                    hashes_total: ws.hashes_total,
                    share_times_ms: ws.share_times_ms.clone(),
                    share_events: ws.share_events.clone()
                  }).collect::<Vec<_>>()
                };
                let blocks_snapshot = { blocks.lock().await.clone() };
                save_persisted(&config_dir, &PersistData { workers: workers_snapshot, blocks: blocks_snapshot });
              }
              _ = beat.tick() => {
                refresh_job(
                  &http,
                  &daemon,
                  &mining_address,
                  &current_job,
                  &job_ring,
                  job_seq.as_ref(),
                  PUBLIC_POOL_TEMPLATE_RESERVE_SIZE,
                )
                .await;
                let now = now_ms();
                let current = current_job.lock().await.clone();
                let job_changed = current.id != prev_job_id;
                prev_job_id = current.id.clone();
                let mut changed_sessions: Vec<String> = Vec::new();
                {
                  let mut w = workers.lock().await;
                  for ws in w.values_mut() {
                    prune_share_events(ws, now);
                    let old_diff = ws.difficulty;
                    maybe_retarget(
                      ws,
                      now,
                      var_enabled,
                      var_retarget_time_s,
                      var_target_time_s,
                      var_variance_percent,
                      var_min_diff,
                      var_max_diff,
                      var_max_jump_percent,
                    );
                    if job_changed || ws.difficulty != old_diff || ws.last_job_id != current.id {
                      ws.last_job_id = current.id.clone();
                      changed_sessions.push(ws.session_id.clone());
                    }
                    ws.hashrate_5min = window_hashrate(ws, now, (5 * 60 * 1000) as i64);
                    ws.hashrate_1hr = window_hashrate(ws, now, (60 * 60 * 1000) as i64);
                    ws.hashrate_6hr = window_hashrate(ws, now, (6 * 60 * 60 * 1000) as i64);
                    ws.hashrate_24hr = window_hashrate(ws, now, (24 * 60 * 60 * 1000) as i64);
                  }
                }
                {
                  let to_evict: Vec<(String, WorkerState)> = {
                    let w = workers.lock().await;
                    w.iter()
                      .filter(|(k, ws)| {
                        !is_persist_session(k)
                          && now.saturating_sub(ws.last_activity_ms) >= miner_timeout_ms
                      })
                      .map(|(k, ws)| (k.clone(), ws.clone()))
                      .collect()
                  };
                  if !to_evict.is_empty() {
                    let mut w = workers.lock().await;
                    let mut s = senders.lock().await;
                    let mut d = last_disconnected.lock().await;
                    for (sid, ws) in to_evict {
                      w.remove(&sid);
                      s.remove(&sid);
                      d.insert(ws.miner.clone(), ws);
                      while d.len() > 32 {
                        if let Some(k) = d.keys().next().cloned() {
                          d.remove(&k);
                        } else {
                          break;
                        }
                      }
                    }
                  }
                }
                if !changed_sessions.is_empty() && !current.blob.is_empty() && blocktemplate_blob_ok(&current.blob) {
                  solo_pool_log(&format!(
                    "stratum: broadcast method=job to {} session(s) canonical_job_id={} height={} blob_hex_len={} blob_fp={} (varDiff/retarget)",
                    changed_sessions.len(),
                    current.id,
                    current.height,
                    current.blob.len(),
                    hex_prefix_suffix(&current.blob, 16, 16)
                  ));
                  let senders_map = senders.lock().await;
                  let workers_map = workers.lock().await;
                  for sid in changed_sessions {
                    if let (Some(tx), Some(ws)) = (senders_map.get(&sid), workers_map.get(&sid)) {
                      let worker_target = difficulty_to_target_hex(ws.difficulty);
                      let worker_job_id = worker_job_id(&current.id, ws.difficulty);
                      let _ = tx.send(json!({
                        "jsonrpc":"2.0",
                        "method":"job",
                        "params":{
                          "blob": current.blob,
                          "job_id": worker_job_id,
                          "target": worker_target,
                          "height": current.height,
                          "algo": "rx/arq",
                          "seed_hash": current.seed_hash,
                          "next_seed_hash": stratum_next_seed_hash(&current.seed_hash, &current.next_seed_hash)
                        }
                      }));
                    }
                  }
                }
                let w = workers.lock().await;
                let active_count = w
                  .values()
                  .filter(|ws| !is_persist_session(&ws.session_id))
                  .count();
                let known_miners: HashSet<String> = w.values().map(|ws| ws.miner.clone()).collect();
                let mut list: Vec<Value> = w
                  .values()
                  .map(|ws| {
                    let graph = build_hashrate_graph(ws, now, 60_000, 60);
                    json!({
                      "miner": ws.miner,
                      "active": !is_persist_session(&ws.session_id),
                      "lastShare": ws.last_share_ms,
                      "difficulty": ws.difficulty,
                      "hashes": ws.hashes_total,
                      "rejects": ws.rejects,
                      "hashrate_5min": ws.hashrate_5min,
                      "hashrate_1hr": ws.hashrate_1hr,
                      "hashrate_6hr": ws.hashrate_6hr,
                      "hashrate_24hr": ws.hashrate_24hr,
                      "hashrate_graph": graph
                    })
                  })
                  .collect();
                drop(w);
                {
                  let d = last_disconnected.lock().await;
                  for ows in d.values() {
                    if known_miners.contains(&ows.miner) {
                      continue;
                    }
                    let graph = build_hashrate_graph(ows, now, 60_000, 60);
                    list.push(json!({
                      "miner": ows.miner,
                      "active": false,
                      "lastShare": ows.last_share_ms,
                      "difficulty": ows.difficulty,
                      "hashes": ows.hashes_total,
                      "rejects": ows.rejects,
                      "hashrate_5min": ows.hashrate_5min,
                      "hashrate_1hr": ows.hashrate_1hr,
                      "hashrate_6hr": ows.hashrate_6hr,
                      "hashrate_24hr": ows.hashrate_24hr,
                      "hashrate_graph": graph
                    }));
                  }
                }
                let w = workers.lock().await;
                // Daemon height + network fields (one RPC); also unlock pending block rows.
                let mut network_hashrate: u64 = 0;
                let mut network_diff: u64 = 0;
                let mut network_height: u64 = 0;
                if let Ok(info) = daemon_post(&http, &daemon.0, daemon.1, "get_info", 0, &Value::Null).await {
                  if let Some(r) = info.get("result") {
                    if let Some(h) = r.get("height").and_then(|v| v.as_u64()) {
                      network_height = h;
                      let mut bl = blocks.lock().await;
                      for b in bl.iter_mut() {
                        let bh = b.get("height").and_then(|v| v.as_u64()).unwrap_or(0);
                        let st = b.get("status").and_then(|v| v.as_i64()).unwrap_or(0);
                        if st == 0 && h >= bh.saturating_add(19) {
                          if let Some(o) = b.as_object_mut() {
                            o.insert("status".into(), json!(2));
                          }
                        }
                      }
                    }
                    network_diff = r.get("difficulty").and_then(|v| v.as_u64()).unwrap_or(0);
                    let target = r.get("target").and_then(|v| v.as_u64()).unwrap_or(120);
                    if target > 0 {
                      network_hashrate = network_diff / target;
                    }
                  }
                }
                let pool_h5 = w
                  .values()
                  .filter(|ws| !is_persist_session(&ws.session_id))
                  .map(|x| x.hashrate_5min)
                  .sum::<u64>();
                let rh = round_hashes.load(Ordering::Relaxed);
                let current_effort = if network_diff == 0 {
                  0f64
                } else {
                  ((100.0 * (rh as f64) / (network_diff as f64)).round()) / 100.0
                };
                let block_time_ms: u64 = if pool_h5 == 0 {
                  0
                } else {
                  1000u64.saturating_mul(120).saturating_mul(network_hashrate) / pool_h5
                };
                let blocks_snapshot = { blocks.lock().await.clone() };
                let blocks_found = blocks_snapshot.len() as u64;
                let avg_eff = average_block_effort(&blocks_snapshot);
                sink.emit_receive("set_pool_data", json!({
                  "workers": list,
                  "stats": {
                    "activeWorkers": active_count,
                    "roundHashes": rh,
                    "currentEffort": current_effort,
                    "blockTime": block_time_ms,
                    "blocksFound": blocks_found,
                    "averageEffort": avg_eff,
                    "networkHashrate": network_hashrate,
                    "diff": network_diff,
                    "height": network_height,
                    "h": {
                      "hashrate_5min": pool_h5,
                      "hashrate_1hr": w.values().filter(|ws| !is_persist_session(&ws.session_id)).map(|x| x.hashrate_1hr).sum::<u64>(),
                      "hashrate_6hr": w.values().filter(|ws| !is_persist_session(&ws.session_id)).map(|x| x.hashrate_6hr).sum::<u64>(),
                      "hashrate_24hr": w.values().filter(|ws| !is_persist_session(&ws.session_id)).map(|x| x.hashrate_24hr).sum::<u64>()
                    }
                  },
                  "blocks": blocks_snapshot,
                  "status": 2
                }));
              }
              accepted = listener.accept() => {
                let Ok((socket, peer)) = accepted else { continue };
                let peer_l = peer.to_string();
                let workers2 = workers.clone();
                let senders2 = senders.clone();
                let current_job2 = current_job.clone();
                let job_ring2 = job_ring.clone();
                let blocks2 = blocks.clone();
                let session_seq2 = session_seq.clone();
                let daemon2 = daemon.clone();
                let http2 = http.clone();
                let fixed_diff_separator2 = fixed_diff_separator.clone();
                let mining_addr2 = mining_address.clone();
                let job_seq2 = job_seq.clone();
                let round_hashes2 = round_hashes.clone();
                let last_disconnected2 = last_disconnected.clone();
                let config_dir_dump = config_dir.clone();
                tokio::spawn(async move {
                  solo_pool_log(&format!("stratum: accepted connection from {peer_l}"));
                  let (reader_half, mut writer_half) = socket.into_split();
                  let (tx_out, mut rx_out) = mpsc::unbounded_channel::<Value>();
                  tokio::spawn(async move {
                    while let Some(payload) = rx_out.recv().await {
                      let _ = send_line(&mut writer_half, &payload).await;
                    }
                  });
                  let mut reader = BufReader::new(reader_half);
                  let mut line = String::new();
                  let mut my_session_id = String::new();
                  let mut last_login_dump: Option<LoginResponseSnapshot> = None;
                  let mut seen_submits: HashMap<String, HashSet<String>> = HashMap::new();
                  loop {
                    line.clear();
                    let n = match reader.read_line(&mut line).await {
                      Ok(0) => {
                        if let Some(snap) = last_login_dump.take() {
                          let elapsed = now_ms() - snap.at_ms;
                          if elapsed >= 0
                            && elapsed < SOLO_POOL_DUMP_EOF_MS
                            && my_session_id == snap.session
                            && solo_pool_file_dump_enabled()
                          {
                            match write_solo_pool_login_close_dump(&config_dir_dump, &snap, elapsed, "EOF") {
                              Ok(p) => solo_pool_log(&format!(
                                "wrote fast-disconnect diagnostic file {} ({} ms after login; compare blob/seeds with XMRig if err 4)"
                                ,
                                p.display(),
                                elapsed
                              )),
                              Err(e) => {
                                solo_pool_log(&format!("diagnostic file write failed: {e}"));
                              }
                            }
                          }
                        }
                        solo_pool_log(&format!(
                          "stratum: peer {peer_l} closed connection (read EOF) session={my_session_id}"
                        ));
                        break;
                      }
                      Ok(n) => n,
                      Err(e) => {
                        if let Some(snap) = last_login_dump.take() {
                          let elapsed = now_ms() - snap.at_ms;
                          if elapsed >= 0
                            && elapsed < SOLO_POOL_DUMP_EOF_MS
                            && my_session_id == snap.session
                            && solo_pool_file_dump_enabled()
                          {
                            let reason = format!("read line error: {e}");
                            match write_solo_pool_login_close_dump(
                              &config_dir_dump,
                              &snap,
                              elapsed,
                              &reason,
                            ) {
                              Ok(p) => solo_pool_log(&format!(
                                "wrote fast-disconnect diagnostic file {} ({} ms after login)"
                                ,
                                p.display(),
                                elapsed
                              )),
                              Err(we) => {
                                solo_pool_log(&format!("diagnostic file write failed: {we}"));
                              }
                            }
                          }
                        }
                        solo_pool_log(&format!(
                          "stratum: peer {peer_l} read_line error: {e} session={my_session_id}"
                        ));
                        break;
                      }
                    };
                    if n == 0 || line.trim().is_empty() {
                      continue;
                    }
                    let parsed: Value = match serde_json::from_str(line.trim()) {
                      Ok(v) => v,
                      Err(e) => {
                        let preview: String = line.chars().take(220).collect();
                        solo_pool_log(&format!(
                          "stratum: peer {peer_l} JSON parse error: {e} (line_len={}) preview={}",
                          line.len(),
                          preview
                        ));
                        let _ = tx_out.send(json!({
                          "id": null,
                          "jsonrpc":"2.0",
                          "error":{"code":-1,"message":"Malformed stratum call"},
                          "result": null
                        }));
                        continue;
                      }
                    };
                    let id = parsed.get("id").cloned().unwrap_or(Value::Null);
                    let method = parsed.get("method").and_then(|v| v.as_str()).unwrap_or("");
                    let params = parsed.get("params").cloned().unwrap_or_else(|| json!({}));

                    if method == "login" {
                      let login = params.get("login").and_then(|v| v.as_str()).unwrap_or("");
                      let pass = params.get("pass").and_then(|v| v.as_str()).unwrap_or("");
                      let rigid = params
                        .get("rigid")
                        .or_else(|| params.get("rig-id"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                      let raw_name = if !rigid.is_empty() { rigid } else if !pass.is_empty() && pass.to_lowercase() != "x" { pass } else { login };
                      let worker_name = sanitize_worker_name(raw_name);
                      let worker_name_for_log = worker_name.clone();
                      let mut worker_diff = 5000u64;
                      if var_enabled {
                        worker_diff = var_start_diff;
                      }
                      let login_parts: Vec<&str> = login.split(&fixed_diff_separator2).collect();
                      if login_parts.len() > 1 {
                        if let Some(last) = login_parts.last() {
                          if let Ok(d) = last.parse::<u64>() {
                            worker_diff = d.clamp(var_min_diff, var_max_diff);
                          }
                        }
                      }
                      let mut j = current_job2.lock().await.clone();
                      if j.id.is_empty() || j.blob.is_empty() || !blocktemplate_blob_ok(&j.blob) {
                        for _ in 0u32..20 {
                          tokio::time::sleep(Duration::from_millis(150)).await;
                          refresh_job(
                            &http2,
                            &daemon2,
                            &mining_addr2,
                            &current_job2,
                            &job_ring2,
                            job_seq2.as_ref(),
                            PUBLIC_POOL_TEMPLATE_RESERVE_SIZE,
                          )
                          .await;
                          j = current_job2.lock().await.clone();
                          if !j.id.is_empty() && !j.blob.is_empty() && blocktemplate_blob_ok(&j.blob) {
                            break;
                          }
                        }
                      }
                      if j.id.is_empty() || j.blob.is_empty() || !blocktemplate_blob_ok(&j.blob) {
                        solo_pool_log(&format!(
                          "stratum: peer {peer_l} login -> Waiting for daemon (no valid job yet job_id={} blob_len={})",
                          j.id,
                          j.blob.len()
                        ));
                        let _ = tx_out.send(json!({
                          "id": id,
                          "jsonrpc":"2.0",
                          "error":{"code":-1,"message":"Waiting for daemon"},
                          "result": null
                        }));
                        continue;
                      }
                      {
                        let mut d = last_disconnected2.lock().await;
                        d.remove(&worker_name);
                      }
                      my_session_id = format!("{:x}", session_seq2.fetch_add(1, Ordering::SeqCst));
                      let t = now_ms();
                      {
                        let mut w = workers2.lock().await;
                        w.insert(my_session_id.clone(), WorkerState {
                          session_id: my_session_id.clone(),
                          miner: worker_name,
                          last_share_ms: t,
                          last_activity_ms: t,
                          last_retarget_ms: t,
                          difficulty: worker_diff,
                          last_job_id: String::new(),
                          shares: 0,
                          rejects: 0,
                          hashes_total: 0,
                          share_times_ms: Vec::new(),
                          share_events: Vec::new(),
                          hashrate_5min: 0,
                          hashrate_1hr: 0,
                          hashrate_6hr: 0,
                          hashrate_24hr: 0
                        });
                      }
                      {
                        let mut s = senders2.lock().await;
                        s.insert(my_session_id.clone(), tx_out.clone());
                      }
                      let worker_target = difficulty_to_target_hex(worker_diff);
                      let worker_job_id = worker_job_id(&j.id, worker_diff);
                      let next_s = stratum_next_seed_hash(&j.seed_hash, &j.next_seed_hash);
                      solo_pool_log(&format!(
                        "stratum: peer {peer_l} login OK worker={} session={} job_id={} height={} worker_diff={} blob_hex_len={} target={} blob_fp={} seed_prefix={} next_stratum_len={} (XMRig err 4 = setBlob)",
                        worker_name_for_log,
                        my_session_id,
                        worker_job_id,
                        j.height,
                        worker_diff,
                        j.blob.len(),
                        worker_target,
                        hex_prefix_suffix(&j.blob, 16, 16),
                        j.seed_hash.chars().take(10).collect::<String>(),
                        next_s.len()
                      ));
                      {
                        let at_login = now_ms();
                        let b = j.blob.as_str();
                        let blob_len = b.len();
                        let blob_head200: String = if b.len() <= 200 {
                          b.to_string()
                        } else {
                          b[..200].to_string()
                        };
                        let blob_tail32: String = if b.len() > 32 {
                          b[b.len() - 32..].to_string()
                        } else {
                          b.to_string()
                        };
                        last_login_dump = Some(LoginResponseSnapshot {
                          at_ms: at_login,
                          peer: peer_l.clone(),
                          worker: worker_name_for_log.clone(),
                          session: my_session_id.clone(),
                          job_id: worker_job_id.clone(),
                          height: j.height,
                          worker_diff,
                          blob_len,
                          blob_head200,
                          blob_tail32,
                          target: worker_target.clone(),
                          seed_hash: j.seed_hash.clone(),
                          next_seed_stratum: next_s.clone()
                        });
                      }
                      let _ = tx_out.send(json!({
                        "id": id,
                        "jsonrpc":"2.0",
                        "error": null,
                        "result": {
                          "id": my_session_id,
                          "job": {
                            "blob": j.blob,
                            "job_id": worker_job_id,
                            "target": worker_target,
                            "height": j.height,
                            "algo": "rx/arq",
                            "seed_hash": j.seed_hash,
                            "next_seed_hash": next_s
                          },
                          "extensions": ["algo", "keepalive"],
                          "keepalive": false,
                          "status":"OK"
                        }
                      }));
                      continue;
                    }

                    if method == "keepalived" || method == "keepalive" {
                      if my_session_id.is_empty() {
                        let _ = tx_out.send(json!({"id": id, "jsonrpc":"2.0", "error":{"code":-1,"message":"Unauthenticated"}, "result": null}));
                        continue;
                      }
                      {
                        let mut w = workers2.lock().await;
                        if let Some(ws) = w.get_mut(&my_session_id) {
                          ws.last_activity_ms = now_ms();
                        }
                      }
                      let _ = tx_out.send(json!({"id": id, "jsonrpc":"2.0", "error": null, "result": {"status":"KEEPALIVED"}}));
                      continue;
                    }

                    if method == "getjob" {
                      if my_session_id.is_empty() {
                        let _ = tx_out.send(json!({"id": id, "jsonrpc":"2.0", "error":{"code":-1,"message":"Unauthenticated"}, "result": null}));
                        continue;
                      }
                      {
                        let mut w = workers2.lock().await;
                        if let Some(ws) = w.get_mut(&my_session_id) {
                          ws.last_activity_ms = now_ms();
                        }
                      }
                      let j = current_job2.lock().await.clone();
                      if j.blob.is_empty() || !blocktemplate_blob_ok(&j.blob) {
                        let _ = tx_out.send(json!({
                          "id": id,
                          "jsonrpc":"2.0",
                          "error":{"code":-1,"message":"Waiting for daemon"},
                          "result": null
                        }));
                        continue;
                      }
                      let worker_target = {
                        let w = workers2.lock().await;
                        let d = w.get(&my_session_id).map(|ws| ws.difficulty).unwrap_or(5000);
                        difficulty_to_target_hex(d)
                      };
                      let worker_diff = {
                        let w = workers2.lock().await;
                        w.get(&my_session_id).map(|ws| ws.difficulty).unwrap_or(5000)
                      };
                      let worker_job_id = worker_job_id(&j.id, worker_diff);
                      let _ = tx_out.send(json!({
                        "id": id,
                        "jsonrpc":"2.0",
                        "error": null,
                        "result": {
                          "blob": j.blob,
                          "job_id": worker_job_id,
                          "target": worker_target,
                          "height": j.height,
                          "algo": "rx/arq",
                          "seed_hash": j.seed_hash,
                          "next_seed_hash": stratum_next_seed_hash(&j.seed_hash, &j.next_seed_hash)
                        }
                      }));
                      continue;
                    }

                    if method == "submit" {
                      if my_session_id.is_empty() {
                        let _ = tx_out.send(json!({"id": id, "jsonrpc":"2.0", "error":{"code":-1,"message":"Unauthenticated"}, "result": null}));
                        continue;
                      }
                      {
                        let mut w = workers2.lock().await;
                        if let Some(ws) = w.get_mut(&my_session_id) {
                          ws.last_activity_ms = now_ms();
                        }
                      }
                      let job_id = params.get("job_id").and_then(|v| v.as_str()).unwrap_or("");
                      let job_id_base = canonical_job_id(job_id);
                      let nonce = params.get("nonce").and_then(|v| v.as_str()).unwrap_or("");
                      let result_hash = params.get("result").and_then(|v| v.as_str()).unwrap_or("");
                      let valid = !job_id.is_empty() && is_hex_8(nonce) && !result_hash.is_empty();
                      let mut current = {
                        let ring = job_ring2.lock().await;
                        ring.iter().rev().find(|j| j.id == job_id_base).cloned()
                      };
                      if current.is_none() {
                        current = Some(current_job2.lock().await.clone());
                      }
                      let current = current.unwrap_or_default();
                      let submit_key = format!("{job_id}:{nonce}");
                      let set = seen_submits.entry(job_id.to_string()).or_default();
                      if set.contains(&submit_key) {
                        let mut w = workers2.lock().await;
                        if let Some(ws) = w.get_mut(&my_session_id) {
                          ws.rejects = ws.rejects.saturating_add(1);
                        }
                        let _ = tx_out.send(json!({"id": id, "jsonrpc":"2.0", "error":{"code":-1,"message":"Duplicate share"}, "result": null}));
                        continue;
                      }
                      if !valid || current.id.is_empty() || !is_hex_64(result_hash) || job_id_base != current.id {
                        let mut w = workers2.lock().await;
                        if let Some(ws) = w.get_mut(&my_session_id) {
                          ws.rejects = ws.rejects.saturating_add(1);
                        }
                        let _ = tx_out.send(json!({"id": id, "jsonrpc":"2.0", "error":{"code":-1,"message":"Invalid work"}, "result": null}));
                        continue;
                      }
                      let worker_target = {
                        let w = workers2.lock().await;
                        let d = w.get(&my_session_id).map(|ws| ws.difficulty).unwrap_or(5000);
                        difficulty_to_target_hex(d)
                      };
                      if !passes_compact_target(result_hash, &worker_target) {
                        let mut w = workers2.lock().await;
                        if let Some(ws) = w.get_mut(&my_session_id) {
                          ws.rejects = ws.rejects.saturating_add(1);
                        }
                        let _ = tx_out.send(json!({"id": id, "jsonrpc":"2.0", "error":{"code":-1,"message":"Rejected low difficulty share"}, "result": null}));
                        continue;
                      }
                      set.insert(submit_key);
                      {
                        let mut w = workers2.lock().await;
                        if let Some(ws) = w.get_mut(&my_session_id) {
                          let share_d = ws.difficulty;
                          let dt = now_ms().saturating_sub(ws.last_share_ms);
                          if dt > 0 {
                            ws.share_times_ms.push(dt);
                            if ws.share_times_ms.len() > 16 {
                              let keep_from = ws.share_times_ms.len().saturating_sub(16);
                              ws.share_times_ms.drain(0..keep_from);
                            }
                          }
                          ws.last_share_ms = now_ms();
                          ws.shares = ws.shares.saturating_add(1);
                          ws.hashes_total = ws.hashes_total.saturating_add(share_d);
                          ws.share_events.push((ws.last_share_ms, share_d));
                          round_hashes2.fetch_add(share_d, Ordering::Relaxed);
                          let old_diff = ws.difficulty;
                          maybe_retarget(
                            ws,
                            ws.last_share_ms,
                            var_enabled,
                            var_retarget_time_s,
                            var_target_time_s,
                            var_variance_percent,
                            var_min_diff,
                            var_max_diff,
                            var_max_jump_percent,
                          );
                          if ws.difficulty != old_diff {
                            let j = current_job2.lock().await.clone();
                            ws.last_job_id = j.id.clone();
                            let worker_target = difficulty_to_target_hex(ws.difficulty);
                            let _ = tx_out.send(json!({
                              "jsonrpc":"2.0",
                              "method":"job",
                              "params":{
                                "blob": j.blob,
                                "job_id": worker_job_id(&j.id, ws.difficulty),
                                "target": worker_target,
                                "height": j.height,
                                "algo": "rx/arq",
                                "seed_hash": j.seed_hash,
                                "next_seed_hash": stratum_next_seed_hash(&j.seed_hash, &j.next_seed_hash)
                              }
                            }));
                          }
                        }
                      }
                      // When PoW meets **network** difficulty, relay block to `arqmad`.
                      // XMRig `submit` carries `nonce` + `result` only — assemble full `blocktemplate_blob` here.
                      // If the miner sends `blob` (non-standard), use it when it also meets network target.
                      let maybe_blob_param = params.get("blob").and_then(|v| v.as_str()).unwrap_or("");
                      let hits_network =
                        current.difficulty > 0 && passes_compact_target(result_hash, &current.target);
                      if hits_network {
                        let block_hex: Option<String> = if !maybe_blob_param.is_empty() {
                          Some(maybe_blob_param.to_string())
                        } else if !current.template_blob.is_empty() {
                          match assemble_block_blob_for_submit(
                            &current.template_blob,
                            current.reserved_offset,
                            nonce,
                          ) {
                            Ok(h) => Some(h),
                            Err(e) => {
                              solo_pool_log(&format!(
                                "submit_block: assemble from template failed: {e} height={} job={}",
                                current.height, current.id
                              ));
                              None
                            }
                          }
                        } else {
                          solo_pool_log(&format!(
                            "submit_block: skip — no miner `blob` and empty template_blob height={}",
                            current.height
                          ));
                          None
                        };
                        if let Some(hx) = block_hex {
                          if let Ok(sb) = daemon_post(
                            &http2,
                            &daemon2.0,
                            daemon2.1,
                            "submit_block",
                            0,
                            &json!([hx]),
                          )
                          .await
                          {
                            if sb.get("error").is_none() {
                              let worker_name = {
                                let w = workers2.lock().await;
                                w.get(&my_session_id)
                                  .map(|ws| ws.miner.clone())
                                  .unwrap_or_else(|| "worker".to_string())
                              };
                              let total_round = round_hashes2.swap(0, Ordering::Relaxed);
                              let mut bl = blocks2.lock().await;
                              bl.push(json!({
                                "status": 0,
                                "hash": result_hash,
                                "height": current.height,
                                "timeFound": now_ms(),
                                "miner": worker_name,
                                "reward": -1,
                                "diff": current.difficulty,
                                "hashes": total_round
                              }));
                              if bl.len() > 100 {
                                let keep_from = bl.len().saturating_sub(100);
                                bl.drain(0..keep_from);
                              }
                              solo_pool_log(&format!(
                                "submit_block: accepted height={} miner={worker_name}",
                                current.height
                              ));
                            } else {
                              solo_pool_log(&format!(
                                "submit_block: daemon RPC error height={} err={}",
                                current.height,
                                sb.get("error").unwrap_or(&Value::Null)
                              ));
                            }
                          }
                        }
                      }
                      let _ = tx_out.send(json!({"id": id, "jsonrpc":"2.0", "error": null, "result": {"status":"OK"}}));
                      continue;
                    }

                    let _ = tx_out.send(json!({
                      "id": id,
                      "jsonrpc":"2.0",
                      "error":{"code":-1,"message":"Invalid method"},
                      "result": null
                    }));
                  }
                  if !my_session_id.is_empty() {
                    let mut w = workers2.lock().await;
                    w.remove(&my_session_id);
                    drop(w);
                    let mut s = senders2.lock().await;
                    s.remove(&my_session_id);
                  } else {
                    let mut w = workers2.lock().await;
                    w.remove(&peer.to_string());
                  }
                });
              }
            }
        }
    }));
}

pub fn stop(st: &mut WalletBackendState) {
    if let Some(tx) = st.solo_pool_shutdown.take() {
        let _ = tx.send(());
    }
    if let Some(h) = st.solo_pool_task.take() {
        h.abort();
    }
}
