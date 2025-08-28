use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, Write};
use std::path::Path;

use crate::model::{Counter, Key};
use crate::pgn::{
    elo_bucket, is_game_start, month_from_headers, opening_from_headers, parse_elo, parse_headers,
    result_from_headers,
};

pub type AggMap = HashMap<Key, Counter>;

/// Parse stdin PGN, aggregate, return (map, total_games).
pub fn aggregate_stream_stdin() -> io::Result<(AggMap, usize)> {
    let stdin = io::stdin();
    let reader = io::BufReader::new(stdin.lock());

    let mut map: AggMap = HashMap::new();
    let mut current_game: Vec<String> = Vec::with_capacity(512);
    let mut total_games = 0usize;

    for line_res in reader.lines() {
        let line = line_res?;
        if is_game_start(&line) && !current_game.is_empty() {
            process_game(&current_game, &mut map);
            current_game.clear();
            total_games += 1;
        }
        current_game.push(line);
    }
    if !current_game.is_empty() {
        process_game(&current_game, &mut map);
        total_games += 1;
    }

    Ok((map, total_games))
}

/// Process one game's lines and update aggregates.
fn process_game(game_lines: &[String], map: &mut AggMap) {
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
