use std::collections::HashMap;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Instant;

use regex::Regex;
use tokio::task;

use crate::aggregator::{aggregate_from_reader, write_csv, AggMap};
use crate::config::Config;
use crate::db;

/* ---- Types ---- */

#[derive(Clone, Debug)]
pub struct PlanItem {
    /// e.g. "2013-01" parsed from the URL (for logging/filters only)
    pub month: String,
    /// full URL to the .pgn.zst
    pub url: String,
    /// sha256 of the compressed file (from sha256sums.txt)
    pub hash: Option<String>,
}

/* ---- Helpers ---- */

fn parse_list_to_oldest(list_txt: &str) -> Vec<(String, String)> {
    // Returns (url, filename). Lines look like:
    // https://.../lichess_db_standard_rated_YYYY-MM.pgn.zst
    let re = Regex::new(r"/([^/\s]+_([0-9]{4}-[0-9]{2})\.pgn\.zst)$").unwrap();
    let mut items: Vec<(String, String)> = list_txt
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() { return None; }
            re.captures(line).map(|cap| {
                let fname = cap.get(1).unwrap().as_str().to_string();
                (line.to_string(), fname)
            })
        })
        .collect();

    // Oldest first by month parsed from filename
    items.sort_by(|a, b| {
        let ma = a.1.split('_').last().unwrap_or("").replace(".pgn.zst", "");
        let mb = b.1.split('_').last().unwrap_or("").replace(".pgn.zst", "");
        ma.cmp(&mb)
    });
    items
}

fn parse_hashes(sums_txt: &str) -> HashMap<String, String> {
    // Lines: "<sha256>  <filename>"
    let re = Regex::new(r"^([a-f0-9]{64})\s+(\S+)$").unwrap();
    let mut map = HashMap::new();
    for line in sums_txt.lines() {
        if let Some(c) = re.captures(line.trim()) {
            let h = c.get(1).unwrap().as_str().to_string();
            let f = c.get(2).unwrap().as_str().to_string();
            map.insert(f, h);
        }
    }
    map
}

fn norm_month(s: &str) -> Option<String> {
    // Accept "YYYY-MM", "YYYY/M", "YYYY.M"
    let s = s.trim();
    let parts: Vec<&str> = s.split(|c| c == '-' || c == '/' || c == '.').collect();
    if parts.len() < 2 { return None; }
    let y = parts[0];
    let m = parts[1];
    if y.len() != 4 || !y.chars().all(|c| c.is_ascii_digit()) { return None; }
    if !m.chars().all(|c| c.is_ascii_digit()) { return None; }
    let mi: u32 = m.parse().ok()?;
    if !(1..=12).contains(&mi) { return None; }
    Some(format!("{}-{:02}", y, mi))
}

async fn fetch_text(url: &str) -> anyhow::Result<String> {
    vprintln!("remote: GET {}", url);
    let t0 = Instant::now();
    let url_owned = url.to_string();
    let text = task::spawn_blocking(move || -> anyhow::Result<String> {
        let resp = reqwest::blocking::get(&url_owned)?.error_for_status()?;
        Ok(resp.text()?)
    }).await??;
    vprintln!("remote: fetched {} bytes in {:.3}s", text.len(), t0.elapsed().as_secs_f64());
    Ok(text)
}

/* ---- Plans ---- */

pub async fn build_plan(
    dbh: &crate::db::Db,
    remote_base_url: &str,
    since: Option<&str>,
    until: Option<&str>,
) -> anyhow::Result<Vec<PlanItem>> {
    let base = remote_base_url.trim_end_matches('/');
    let list_url = format!("{}/list.txt", base);
    let sums_url = format!("{}/sha256sums.txt", base);

    let list_txt = fetch_text(&list_url).await?;
    let sums_txt = fetch_text(&sums_url).await?;
    let hashes = parse_hashes(&sums_txt);

    let mut pairs = parse_list_to_oldest(&list_txt); // (url, filename)
    vprintln!("remote: months available = {}", pairs.len());

    // month filter (for UX only)
    let since_n = since.and_then(norm_month);
    let until_n = until.and_then(norm_month);
    if let Some(ref since_m) = since_n {
        let before = pairs.len();
        pairs.retain(|(_, fname)| {
            let m = fname.split('_').last().unwrap_or("").replace(".pgn.zst", "");
            m.as_str() >= since_m.as_str()
        });
        vprintln!("remote: filtered by since={} -> {} (was {})", since_m, pairs.len(), before);
    }
    if let Some(ref until_m) = until_n {
        let before = pairs.len();
        pairs.retain(|(_, fname)| {
            let m = fname.split('_').last().unwrap_or("").replace(".pgn.zst", "");
            m.as_str() <= until_m.as_str()
        });
        vprintln!("remote: filtered by until={} -> {} (was {})", until_m, pairs.len(), before);
    }

    // join with hashes
    let mut items: Vec<PlanItem> = pairs
        .into_iter()
        .map(|(url, fname)| {
            let month = fname.split('_').last().unwrap().replace(".pgn.zst", "");
            let hash = hashes.get(&fname).cloned();
            PlanItem { month, url, hash }
        })
        .collect();

    // skip already ingested by hash
    let t1 = Instant::now();
    let done = db::already_ingested_hashes(dbh).await?;
    let before = items.len();
    items.retain(|it| it.hash.as_ref().map(|h| !done.contains(h)).unwrap_or(true));
    vprintln!(
        "remote: filtered already-ingested (by hash) -> {} (was {}), query took {:.3}s",
        items.len(), before, t1.elapsed().as_secs_f64()
    );

    Ok(items)
}

pub async fn plan_no_db(
    remote_base_url: &str,
    since: Option<&str>,
    until: Option<&str>,
) -> anyhow::Result<Vec<PlanItem>> {
    let base = remote_base_url.trim_end_matches('/');
    let list_url = format!("{}/list.txt", base);
    let sums_url = format!("{}/sha256sums.txt", base);

    let list_txt = fetch_text(&list_url).await?;
    let sums_txt = fetch_text(&sums_url).await?;
    let hashes = parse_hashes(&sums_txt);

    let mut pairs = parse_list_to_oldest(&list_txt);
    vprintln!("remote: months available = {}", pairs.len());

    let since_n = since.and_then(norm_month);
    let until_n = until.and_then(norm_month);
    if let Some(ref since_m) = since_n {
        let before = pairs.len();
        pairs.retain(|(_, fname)| {
            let m = fname.split('_').last().unwrap_or("").replace(".pgn.zst", "");
            m.as_str() >= since_m.as_str()
        });
        vprintln!("remote: filtered by since={} -> {} (was {})", since_m, pairs.len(), before);
    }
    if let Some(ref until_m) = until_n {
        let before = pairs.len();
        pairs.retain(|(_, fname)| {
            let m = fname.split('_').last().unwrap_or("").replace(".pgn.zst", "");
            m.as_str() <= until_m.as_str()
        });
        vprintln!("remote: filtered by until={} -> {} (was {})", until_m, pairs.len(), before);
    }

    let items = pairs
        .into_iter()
        .map(|(url, fname)| {
            let month = fname.split('_').last().unwrap().replace(".pgn.zst", "");
            let hash = hashes.get(&fname).cloned();
            PlanItem { month, url, hash }
        })
        .collect();

    Ok(items)
}

/* ---- Streaming + aggregation (remote) ---- */

pub async fn stream_and_aggregate_async(
    url: &str,
    out_csv: Option<&Path>,
    cfg: &Config,
) -> anyhow::Result<(AggMap, usize, u128)> {
    let url_owned = url.to_string();
    let out_opt: Option<PathBuf> = out_csv.map(|p| p.to_path_buf());
    let cfg_cloned = cfg.clone();

    let (map, games, elapsed_ms) = tokio::task::spawn_blocking(move || -> anyhow::Result<(AggMap, usize, u128)> {
        let start = Instant::now();

        vprintln!("remote: HTTP GET {}", url_owned);
        let t_net = Instant::now();
        let resp = reqwest::blocking::get(&url_owned)?.error_for_status()?;
        vprintln!("remote: HTTP connected in {:.3}s", t_net.elapsed().as_secs_f64());

        let t_dec = Instant::now();
        let decoder = zstd::stream::Decoder::new(resp)?;
        vprintln!("remote: zstd decoder ready in {:.3}s", t_dec.elapsed().as_secs_f64());

        let reader = BufReader::new(decoder);
        vprintln!("remote: aggregation start");
        let (map, total_games) = aggregate_from_reader(reader, &cfg_cloned)?;
        vprintln!("remote: aggregation done; games={}", total_games);

        if let Some(csv_path) = out_opt.as_ref() {
            let t_csv = Instant::now();
            vprintln!("remote: writing CSV to {}", csv_path.display());
            write_csv(&map, csv_path)?;
            vprintln!("remote: CSV written in {:.3}s", t_csv.elapsed().as_secs_f64());
        }

        let dur = start.elapsed().as_millis();
        Ok((map, total_games, dur))
    })
    .await??;

    Ok((map, games, elapsed_ms))
}