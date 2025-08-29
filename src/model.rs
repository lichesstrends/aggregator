use std::hash::{Hash, Hasher};

#[derive(Clone, Debug, Eq)]
pub struct Key {
    pub month: String,     // "YYYY-MM"
    pub eco_group: String, // e.g., B20, C00, E60, U00
    pub w_bucket: u16,     // lower bound of bucket (e.g., 2200)
    pub b_bucket: u16,
}

impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.month == other.month
            && self.eco_group == other.eco_group
            && self.w_bucket == other.w_bucket
            && self.b_bucket == other.b_bucket
    }
}
impl Hash for Key {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.month.hash(state);
        self.eco_group.hash(state);
        self.w_bucket.hash(state);
        self.b_bucket.hash(state);
    }
}

#[derive(Clone, Debug, Default)]
pub struct Counter {
    pub games: u64,
    pub white_wins: u64,
    pub black_wins: u64,
    pub draws: u64,
}
impl Counter {
    pub fn add_result(&mut self, result: &str) {
        self.games += 1;
        match result {
            "1-0" => self.white_wins += 1,
            "0-1" => self.black_wins += 1,
            "1/2-1/2" => self.draws += 1,
            _ => {}
        }
    }
}
