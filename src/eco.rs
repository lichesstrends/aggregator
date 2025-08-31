// src/eco.rs
// Map ECO codes (e.g., "B45") to natural family ranges like "B20-B99".
// Unknown / missing ECO maps to "U00".

pub struct EcoRange {
    pub start: u16,              // A00..E99 => 0..499 (A=0*100, B=1*100, ...)
    pub end: u16,                // inclusive
    pub label: &'static str,     // e.g., "B20-B99" or "A47"
}

const fn idx(letter: char) -> u16 { (letter as u8 - b'A') as u16 }
const fn code(letter: char, n: u8) -> u16 { idx(letter) * 100 + (n as u16) }

// The canonical groupings (names kept in comments for clarity).
// Label is exactly what we store into DB (e.g., "B20-B99", "A47").
pub static ECO_RANGES: &[EcoRange] = &[
    // A00–A99
    EcoRange { start: code('A', 00), end: code('A', 00), label: "A00" }, // Polish (Sokolsky)
    EcoRange { start: code('A', 01), end: code('A', 01), label: "A01" }, // Nimzovich-Larsen
    EcoRange { start: code('A', 02), end: code('A', 03), label: "A02-A03" }, // Bird's
    EcoRange { start: code('A', 04), end: code('A', 09), label: "A04-A09" }, // Reti
    EcoRange { start: code('A', 10), end: code('A', 39), label: "A10-A39" }, // English
    EcoRange { start: code('A', 40), end: code('A', 41), label: "A40-A41" }, // Queen's pawn
    EcoRange { start: code('A', 42), end: code('A', 42), label: "A42" },     // Modern (Averbakh)
    EcoRange { start: code('A', 43), end: code('A', 44), label: "A43-A44" }, // Old Benoni
    EcoRange { start: code('A', 45), end: code('A', 46), label: "A45-A46" }, // Queen's pawn game
    EcoRange { start: code('A', 47), end: code('A', 47), label: "A47" },     // Queen's Indian
    EcoRange { start: code('A', 48), end: code('A', 49), label: "A48-A49" }, // East Indian / KID dev.
    EcoRange { start: code('A', 50), end: code('A', 50), label: "A50" },     // QP game (1.d4 Nf6 2.c4)
    EcoRange { start: code('A', 51), end: code('A', 52), label: "A51-A52" }, // Budapest
    EcoRange { start: code('A', 53), end: code('A', 55), label: "A53-A55" }, // Old Indian
    EcoRange { start: code('A', 56), end: code('A', 56), label: "A56" },     // Benoni
    EcoRange { start: code('A', 57), end: code('A', 59), label: "A57-A59" }, // Benko
    EcoRange { start: code('A', 60), end: code('A', 79), label: "A60-A79" }, // Benoni
    EcoRange { start: code('A', 80), end: code('A', 99), label: "A80-A99" }, // Dutch

    // B00–B99
    EcoRange { start: code('B', 00), end: code('B', 00), label: "B00" },     // King's pawn opening
    EcoRange { start: code('B', 01), end: code('B', 01), label: "B01" },     // Scandinavian
    EcoRange { start: code('B', 02), end: code('B', 05), label: "B02-B05" }, // Alekhine
    EcoRange { start: code('B', 06), end: code('B', 06), label: "B06" },     // Modern (Robatsch)
    EcoRange { start: code('B', 07), end: code('B', 09), label: "B07-B09" }, // Pirc
    EcoRange { start: code('B', 10), end: code('B', 19), label: "B10-B19" }, // Caro-Kann
    EcoRange { start: code('B', 20), end: code('B', 99), label: "B20-B99" }, // Sicilian

    // C00–C99
    EcoRange { start: code('C', 00), end: code('C', 19), label: "C00-C19" }, // French
    EcoRange { start: code('C', 20), end: code('C', 20), label: "C20" },     // K's pawn game
    EcoRange { start: code('C', 21), end: code('C', 22), label: "C21-C22" }, // Centre game
    EcoRange { start: code('C', 23), end: code('C', 24), label: "C23-C24" }, // Bishop's opening
    EcoRange { start: code('C', 25), end: code('C', 29), label: "C25-C29" }, // Vienna
    EcoRange { start: code('C', 30), end: code('C', 39), label: "C30-C39" }, // King's Gambit
    EcoRange { start: code('C', 40), end: code('C', 40), label: "C40" },     // King's knight opening
    EcoRange { start: code('C', 41), end: code('C', 41), label: "C41" },     // Philidor
    EcoRange { start: code('C', 42), end: code('C', 43), label: "C42-C43" }, // Petrov
    EcoRange { start: code('C', 44), end: code('C', 44), label: "C44" },     // K's pawn game (misc)
    EcoRange { start: code('C', 45), end: code('C', 45), label: "C45" },     // Scotch
    EcoRange { start: code('C', 46), end: code('C', 46), label: "C46" },     // Three knights
    EcoRange { start: code('C', 47), end: code('C', 49), label: "C47-C49" }, // Four knights / Scotch var.
    EcoRange { start: code('C', 50), end: code('C', 50), label: "C50" },     // Italian Game (umbrella)
    EcoRange { start: code('C', 51), end: code('C', 52), label: "C51-C52" }, // Evans gambit
    EcoRange { start: code('C', 53), end: code('C', 54), label: "C53-C54" }, // Giuoco Piano
    EcoRange { start: code('C', 55), end: code('C', 59), label: "C55-C59" }, // Two knights
    EcoRange { start: code('C', 60), end: code('C', 99), label: "C60-C99" }, // Ruy Lopez

    // D00–D99
    EcoRange { start: code('D', 00), end: code('D', 00), label: "D00" },     // Queen's pawn game
    EcoRange { start: code('D', 01), end: code('D', 01), label: "D01" },     // Richter–Veresov
    EcoRange { start: code('D', 02), end: code('D', 02), label: "D02" },     // QP game
    EcoRange { start: code('D', 03), end: code('D', 03), label: "D03" },     // Torre
    EcoRange { start: code('D', 04), end: code('D', 05), label: "D04-D05" }, // QP game (e3)
    EcoRange { start: code('D', 06), end: code('D', 06), label: "D06" },     // Queen's Gambit
    EcoRange { start: code('D', 07), end: code('D', 09), label: "D07-D09" }, // QGD, Chigorin
    EcoRange { start: code('D', 10), end: code('D', 15), label: "D10-D15" }, // Slav
    EcoRange { start: code('D', 16), end: code('D', 16), label: "D16" },     // Slav accepted (Alapin)
    EcoRange { start: code('D', 17), end: code('D', 19), label: "D17-D19" }, // Slav, Czech
    EcoRange { start: code('D', 20), end: code('D', 29), label: "D20-D29" }, // QGA
    EcoRange { start: code('D', 30), end: code('D', 42), label: "D30-D42" }, // QGD
    EcoRange { start: code('D', 43), end: code('D', 49), label: "D43-D49" }, // QGD Semi-Slav
    EcoRange { start: code('D', 50), end: code('D', 69), label: "D50-D69" }, // QGD 4.Bg5
    EcoRange { start: code('D', 70), end: code('D', 79), label: "D70-D79" }, // Neo-Grünfeld
    EcoRange { start: code('D', 80), end: code('D', 99), label: "D80-D99" }, // Grünfeld

    // E00–E99
    EcoRange { start: code('E', 00), end: code('E', 00), label: "E00" },     // QP game
    EcoRange { start: code('E', 01), end: code('E', 09), label: "E01-E09" }, // Catalan (closed)
    EcoRange { start: code('E', 10), end: code('E', 10), label: "E10" },     // QP game
    EcoRange { start: code('E', 11), end: code('E', 11), label: "E11" },     // Bogo-Indian
    EcoRange { start: code('E', 12), end: code('E', 19), label: "E12-E19" }, // Queen's Indian
    EcoRange { start: code('E', 20), end: code('E', 59), label: "E20-E59" }, // Nimzo-Indian
    EcoRange { start: code('E', 60), end: code('E', 99), label: "E60-E99" }, // King's Indian
];

fn parse_eco_code(s: &str) -> Option<u16> {
    let s = s.trim().to_ascii_uppercase();
    if s.len() != 3 { return None; }
    let mut it = s.chars();
    let letter = it.next()?;
    let d1 = it.next()?.to_digit(10)? as u8;
    let d2 = it.next()?.to_digit(10)? as u8;
    if !('A'..='E').contains(&letter) { return None; }
    Some(code(letter, d1 * 10 + d2))
}

pub fn label_for_code(eco: &str) -> &'static str {
    if let Some(num) = parse_eco_code(eco) {
        for r in ECO_RANGES {
            if num >= r.start && num <= r.end {
                return r.label;
            }
        }
    }
    "U00" // unknown/missing
}