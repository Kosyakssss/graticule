use std::collections::HashMap;
use std::sync::OnceLock;

/// Where words may break across lines, found by Frank Liang's method as TeX finds them: patterns of letters with numbers between them, odd numbers allowing a break and even ones forbidding it, the highest number at each place winning. A word hyphenated by hand in the exceptions is taken as written.
#[derive(Debug, Default)]
pub struct Hyphenation {
    patterns: HashMap<String, Vec<u8>>,
    exceptions: HashMap<String, Vec<usize>>,
    longest: usize,
    left: usize,
    right: usize,
}

impl Hyphenation {
    /// American English, from the patterns of the hyph-utf8 package, never leaving fewer than two letters before a break or three after it.
    pub fn english() -> &'static Hyphenation {
        static ENGLISH: OnceLock<Hyphenation> = OnceLock::new();
        ENGLISH.get_or_init(|| Hyphenation::new(include_str!("../data/hyph-en-us.txt"), 2, 3))
    }

    /// Reads patterns and exceptions, one to a line, in the hyph-utf8 package's plain text form: a pattern such as `.ach4`, an exception such as `as-so-ciate`; lines starting with `%` are notes. `left` and `right` are the fewest letters a break leaves on either side.
    pub fn new(text: &str, left: usize, right: usize) -> Self {
        let mut hyphenation = Hyphenation { left: left.max(1), right: right.max(1), ..Hyphenation::default() };
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('%')) {
            if line.contains('-') {
                let mut points = Vec::new();
                let mut word = String::new();
                for c in line.chars() {
                    match c {
                        '-' => points.push(word.chars().count()),
                        c => word.push(c),
                    }
                }
                hyphenation.exceptions.insert(word, points);
            } else {
                let mut letters = String::new();
                let mut values = vec![0];
                for c in line.chars() {
                    match c.to_digit(10) {
                        Some(d) => {
                            if let Some(last) = values.last_mut() {
                                *last = d as u8;
                            }
                        }
                        None => {
                            letters.push(c);
                            values.push(0);
                        }
                    }
                }
                hyphenation.longest = hyphenation.longest.max(letters.chars().count());
                hyphenation.patterns.insert(letters, values);
            }
        }
        hyphenation
    }

    /// Where `word` may break, as counts of characters before each break, in order. A word holding anything but letters is left whole.
    pub fn points(&self, word: &str) -> Vec<usize> {
        let lower: String = word.chars().flat_map(char::to_lowercase).collect();
        let count = lower.chars().count();
        if count != word.chars().count() || count < self.left + self.right || !lower.chars().all(char::is_alphabetic) {
            return Vec::new();
        }
        if let Some(points) = self.exceptions.get(&lower) {
            return points.clone();
        }
        let dotted: Vec<char> = std::iter::once('.').chain(lower.chars()).chain(std::iter::once('.')).collect();
        let mut values = vec![0u8; dotted.len() + 1];
        let mut key = String::new();
        for start in 0..dotted.len() {
            key.clear();
            for &c in dotted[start..].iter().take(self.longest) {
                key.push(c);
                if let Some(found) = self.patterns.get(&key) {
                    for (k, &v) in found.iter().enumerate() {
                        values[start + k] = values[start + k].max(v);
                    }
                }
            }
        }
        (self.left..=count - self.right).filter(|&k| values[k + 1] % 2 == 1).collect()
    }
}
