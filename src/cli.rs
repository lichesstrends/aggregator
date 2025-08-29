use std::path::PathBuf;

pub struct Cli {
    pub out: Option<PathBuf>,
    pub ingest_remote: bool,
    pub until: Option<String>, // "YYYY-MM"
    pub list_url: String,      // list.txt endpoint
    pub verbose: bool,         // -v / --verbose
}

pub fn parse() -> Cli {
    let mut out: Option<PathBuf> = None;
    let mut ingest_remote = false;
    let mut until: Option<String> = None;
    let mut list_url = "https://database.lichess.org/standard/list.txt".to_string();
    let mut verbose = false;

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--out" => {
                if let Some(p) = it.next() {
                    out = Some(PathBuf::from(p));
                }
            }
            "--ingest-remote" | "--remote" => ingest_remote = true,
            "--until" => {
                if let Some(m) = it.next() {
                    until = Some(m);
                }
            }
            "--list-url" => {
                if let Some(u) = it.next() {
                    list_url = u;
                }
            }
            "--verbose" | "-v" => verbose = true,
            _ => {}
        }
    }

    Cli { out, ingest_remote, until, list_url, verbose }
}
