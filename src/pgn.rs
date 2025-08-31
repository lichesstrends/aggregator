use std::collections::HashMap;

/// Parse PGN headers (Tag -> Value).
pub fn parse_headers(game_lines: &[String]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in game_lines {
        let line = line.trim();
        if !(line.starts_with('[') && line.ends_with(']')) { continue; }
        if let Some(space_idx) = line.find(' ') {
            let tag = &line[1..space_idx];
            if let (Some(fq_rel), Some(lq)) = (line[space_idx..].find('"'), line.rfind('"')) {
                let fq = space_idx + fq_rel;
                if lq > fq {
                    let val = &line[(fq + 1)..lq];
                    map.insert(tag.to_string(), val.to_string());
                }
            }
        }
    }
    map
}

pub fn is_game_start(line: &str) -> bool {
    line.starts_with("[Event ")
}

/// Extract YYYY-MM from UTCDate or Date ("YYYY.MM.DD"); else "unknown".
pub fn month_from_headers(h: &HashMap<String, String>) -> String {
    let date = h.get("UTCDate").or_else(|| h.get("Date"));
    if let Some(d) = date {
        if d.len() >= 7 && d.as_bytes().get(4) == Some(&b'.') && d.as_bytes().get(7) == Some(&b'.') {
            let y = &d[0..4];
            let m = &d[5..7];
            if y.chars().all(|c| c.is_ascii_digit()) && m.chars().all(|c| c.is_ascii_digit()) {
                return format!("{}-{}", y, m);
            }
        }
    }
    "unknown".to_string()
}

pub fn eco_group_from_headers(h: &std::collections::HashMap<String, String>) -> String {
    if let Some(eco) = h.get("ECO") {
        // Map specific ECO (e.g., "B45") to a natural group label (e.g., "B20-B99")
        return crate::eco::label_for_code(eco).to_string();
    }
    "U00".to_string()
}

pub fn result_from_headers(h: &HashMap<String, String>) -> String {
    h.get("Result").cloned().unwrap_or_else(|| "*".to_string())
}

pub fn parse_elo(s: Option<&String>) -> Option<u16> {
    s.and_then(|x| x.parse::<u16>().ok())
}

/// Bucket ELO with an arbitrary bucket size (e.g., 200).
pub fn elo_bucket_with_size(elo: Option<u16>, size: u16) -> u16 {
    match elo {
        Some(e) if size > 0 => (e / size) * size,
        _ => 0,
    }
}
