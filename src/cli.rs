use std::path::PathBuf;

/// Minimal CLI: only --out <file.csv>. Unknown args are ignored.
pub struct Cli {
    pub out: Option<PathBuf>,
}

pub fn parse() -> Cli {
    let mut out: Option<PathBuf> = None;
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--out" => {
                if let Some(p) = it.next() {
                    out = Some(PathBuf::from(p));
                }
            }
            _ => {
                // ignore unknowns for now
            }
        }
    }
    Cli { out }
}
