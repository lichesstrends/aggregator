use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, Write};
use std::path::Path;

use rayon::prelude::*;

use crate::model::{Counter, Key};
use crate::pgn::{
    elo_bucket, is_game_start, month_from_headers, opening_from_headers, parse_elo, parse_headers,
    result_from_headers,
};

pub type AggMap = HashMap<Key, Counter>;

/// Parse stdin PGN, aggregate in parallel (batched), return (map, total_games).
pub fn aggregate_stream_stdin() -> io::Result<(AggMap, usize)> {
    // Batch size (games per parallel batch). Tunable via env, default 1000.
    let batch_size: usize = std::env::var("AGG_BATCH_SIZE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1000);

    let stdin = io::stdin();
    let reader = io::BufReader::new(stdin.lock());

    let mut global_map: AggMap = HashMap::new();
    let mut current_game: Vec<String> = Vec::with_capacity(512);
    let mut batch: Vec<Vec<String>> = Vec::with_capacity(batch_size);
    let mut total_games = 0usize;

    // Producer: split stream into games and collect into batches
    for line_res in reader.lines() {
        let line = line_res?;

        if is_game_start(&line) && !current_game.is_empty() {
            // finalize previous game
            let finished = std::mem::take(&mut current_game);
            batch.push(finished);
            total_games += 1;

            if batch.len() >= batch_size {
                process_batch_parallel(&batch, &mut global_map);
                batch.clear();
            }
        }

        current_game.push(line);
    }

    // Flush last game + last batch
    if !current_game.is_empty() {
        batch.push(current_game);
        total_games += 1;
    }
    if !batch.is_empty() {
        process_batch_parallel(&batch, &mut global_map);
    }

    Ok((global_map, total_games))
}

/// Process one batch of games in parallel and merge into global map.
fn process_batch_parallel(batch: &[Vec<String>], global: &mut AggMap) {
    // Each worker builds a local map (no locks), then we reduce/merge.
    let batch_map: AggMap = batch
        .par_iter()
        .fold(
            || AggMap::new(),
            |mut acc, game_lines| {
                process_game_into_map(game_lines, &mut acc);
                acc
            },
        )
        .reduce(
            || AggMap::new(),
            |mut a, b| {
                merge_maps(&mut a, b);
                a
            },
        );

    merge_maps(global, batch_map);
}

/// Process one game's lines and update the provided (local) map.
fn process_game_into_map(game_lines: &[String], map: &mut AggMap) {
    if game_lines.is_empty() {
        return;
    }

    let h = parse_headers(game_lines);

    let month = month_from_headers(&h);
    let opening = opening_from_headers(&h);
    let result = result_from_headers(&h);

    let w_elo = parse_elo(h.get("WhiteElo"));
    let b_elo = parse_elo(h.get("BlackElo"));

    let key = Key {
        month,
        opening,
        w_bucket: elo_bucket(w_elo),
        b_bucket: elo_bucket(b_elo),
    };

    let counter = map.entry(key).or_default();
    counter.add_result(&result);
}

/// Merge `src` into `dst` by summing counters.
fn merge_maps(dst: &mut AggMap, src: AggMap) {
    for (k, c) in src {
        let e = dst.entry(k).or_default();
        e.games += c.games;
        e.white_wins += c.white_wins;
        e.black_wins += c.black_wins;
        e.draws += c.draws;
    }
}

/// Optional CSV writer (percentages included).
pub fn write_csv(map: &AggMap, out_path: &Path) -> io::Result<()> {
    let mut entries: Vec<_> = map.iter().collect();
    entries.sort_by_key(|(_, c)| std::cmp::Reverse(c.games));

    let mut f = File::create(out_path)?;
    writeln!(
        f,
        "month,opening,white_bucket,black_bucket,games,white_pct,black_pct,draw_pct"
    )?;
    for (k, c) in entries {
        let (w, b, d) = c.percentages();
        writeln!(
            f,
            "{},{},{},{},{},{:.3},{:.3},{:.3}",
            k.month,
            escape_csv(&k.opening),
            k.w_bucket,
            k.b_bucket,
            c.games,
            w,
            b,
            d
        )?;
    }
    Ok(())
}

fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}
