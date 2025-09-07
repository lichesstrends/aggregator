use std::fs::File;
use std::io::{Read, BufReader};
use std::path::{Path};
use std::time::Instant;

use sha2::{Digest, Sha256};

use crate::aggregator::{aggregate_from_reader, write_csv, AggMap};
use crate::config::Config;

/// Reader that updates a SHA256 as it reads.
struct HashedReader<R: Read> {
    inner: R,
    hasher: Sha256,
}

impl<R: Read> HashedReader<R> {
    fn new(inner: R) -> Self {
        Self { inner, hasher: Sha256::new() }
    }
    fn finalize_hex(self) -> String {
        let bytes = self.hasher.finalize();
        hex::encode(bytes)
    }
}

impl<R: Read> Read for HashedReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        if n > 0 {
            self.hasher.update(&buf[..n]);
        }
        Ok(n)
    }
}

/// Stream a local .zst file, compute SHA256 of the compressed bytes, decode and aggregate.
/// Returns (sha256_hex, map, total_games, elapsed_ms).
pub fn process_local_file(
    path: &Path,
    out_csv: Option<&Path>,
    cfg: &Config,
) -> anyhow::Result<(String, AggMap, usize, u128)> {
    let t0 = Instant::now();
    let f = File::open(path)?;
    let mut hashed_reader = HashedReader::new(f);

    let decoder = zstd::stream::Decoder::new(&mut hashed_reader)?;
    let reader = BufReader::new(decoder);

    let (map, total_games) = aggregate_from_reader(reader, cfg)?;
    let dur = t0.elapsed().as_millis();

    if let Some(csv_path) = out_csv {
        write_csv(&map, csv_path)?;
    }

    // finalize hash AFTER we consumed the reader (single pass)
    let hash_hex = hashed_reader.finalize_hex();
    Ok((hash_hex, map, total_games, dur))
}
