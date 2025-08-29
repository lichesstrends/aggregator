use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub bucket_size: u16,          // ELO bucket size (default 200)
    pub list_url: String,          // lichess list.txt
    pub batch_size: usize,         // games per parallel batch
    pub rayon_threads: Option<usize>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bucket_size: 200,
            list_url: "https://database.lichess.org/standard/list.txt".to_string(),
            batch_size: 1000,
            rayon_threads: None,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        match std::fs::read_to_string("config.toml") {
            Ok(s) => toml::from_str(&s).unwrap_or_else(|_| Self::default()),
            Err(_) => Self::default(),
        }
    }
}
