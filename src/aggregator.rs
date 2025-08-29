use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, Write};
use std::path::Path;

use rayon::prelude::*;

use crate::config::Config;
use crate::model::{Counter, Key};
use crate::pgn::{
    elo_bucket_with_size, is_game_start, month_from_headers, eco_group_from_headers, parse_elo,
    parse_headers, result_from_headers,
};

pub type AggMap = HashMap<Key, Counter>;

/// Aggregate from any buffered reader of PGN text using config (batch size, bucket size).
pub fn aggregate_from_reader<R: BufRead>(mut reader: R, cfg: &Config) -> io::Result<(AggMap, usize)> {
    let mut global_map: AggMap = HashMap::new();
    let mut current_game: Vec<String> = Vec::with_capacity(512);
    let mut batch: Vec<Vec<String>> = Vec::with_capacity(cfg.batch_size);
    let mut total_games = 0usize;

    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line)?;
        if n == 0 { break; }
        if line.ends_with('\n') { line.pop(); if line.ends_with('\r') { line.pop(); } }

        if is_game_start(&line) && !current_game.is_empty() {
            batch.push(std::mem::take(&mut current_game));
            total_games += 1;
            if batch.len() >= cfg.batch_size {
                process_batch_parallel(&batch, &mut global_map, cfg);
                batch.clear();
            }
        }
        current_game.push(line.clone());
    }

    if !current_game.is_empty() {
        batch.push(current_game);
        total_games += 1;
    }
    if !batch.is_empty() {
        process_batch_parallel(&batch, &mut global_map, cfg);
    }

    Ok((global_map, total_games))
}

fn process_batch_parallel(batch: &[Vec<String>], global: &mut AggMap, cfg: &Config) {
    let batch_map: AggMap = batch
        .par_iter()
        .fold(
            || AggMap::new(),
            |mut acc, game_lines| { process_game_into_map(game_lines, &mut acc, cfg); acc },
        )
        .reduce(
            || AggMap::new(),
            |mut a, b| { merge_maps(&mut a, b); a },
        );
    merge_maps(global, batch_map);
}

fn process_game_into_map(game_lines: &[String], map: &mut AggMap, cfg: &Config) {
    if game_lines.is_empty() { return; }
    let h = parse_headers(game_lines);

    let month = month_from_headers(&h);
    let eco_group = eco_group_from_headers(&h);
    let result = result_from_headers(&h);

    let w_elo = parse_elo(h.get("WhiteElo"));
    let b_elo = parse_elo(h.get("BlackElo"));

    let key = Key {
        month,
        eco_group,
        w_bucket: elo_bucket_with_size(w_elo, cfg.bucket_size),
        b_bucket: elo_bucket_with_size(b_elo, cfg.bucket_size),
    };

    let counter = map.entry(key).or_default();
    counter.add_result(&result);
}

fn merge_maps(dst: &mut AggMap, src: AggMap) {
    for (k, c) in src {
        let e = dst.entry(k).or_default();
        e.games += c.games;
        e.white_wins += c.white_wins;
        e.black_wins += c.black_wins;
        e.draws += c.draws;
    }
}

pub fn write_csv(map: &AggMap, out_path: &Path) -> io::Result<()> {
    let mut entries: Vec<_> = map.iter().collect();
    entries.sort_by_key(|(_, c)| std::cmp::Reverse(c.games));

    let mut f = File::create(out_path)?;
    // counts only
    writeln!(
        f,
        "month,eco_group,white_bucket,black_bucket,games,white_wins,black_wins,draws"
    )?;
    for (k, c) in entries {
        writeln!(
            f,
            "{},{},{},{},{},{},{},{}",
            k.month,
            k.eco_group,
            k.w_bucket,
            k.b_bucket,
            c.games,
            c.white_wins,
            c.black_wins,
            c.draws
        )?;
    }
    Ok(())
}
