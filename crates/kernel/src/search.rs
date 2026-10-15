//! Finding by typing: a fuzzy match of what was typed against a line of words, scored so the best match comes first, and where its letters are.

/// How well `query` matches a line: a higher score is a better match, and `at` holds the places of the matched characters, counted in characters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fuzzy {
    pub score: i32,
    pub at: Vec<usize>,
}

const MATCHED: i32 = 16;
const WORD_START: i32 = 8;
const NEXT_TO: i32 = 6;

/// Whether the letters of `query` appear in `text` in order, and how well. Spaces in the query are ignored. Case is ignored unless the query has a capital letter, as Helix and Sublime do. A match scores more where its letters start words and sit next to one another, and less for the letters it skips.
pub fn fuzzy(query: &str, text: &str) -> Option<Fuzzy> {
    let exact = query.chars().any(char::is_uppercase);
    let fold = |c: char| if exact { c } else { c.to_lowercase().next().unwrap_or(c) };
    let q: Vec<char> = query.chars().filter(|c| !c.is_whitespace()).map(fold).collect();
    if q.is_empty() {
        return Some(Fuzzy { score: 0, at: Vec::new() });
    }
    let raw: Vec<char> = text.chars().collect();
    let t: Vec<char> = raw.iter().map(|&c| fold(c)).collect();
    let n = t.len();
    if q.len() > n {
        return None;
    }
    let starts = |j: usize| j == 0 || !raw[j - 1].is_alphanumeric() && raw[j].is_alphanumeric() || raw[j - 1].is_lowercase() && raw[j].is_uppercase();
    const NO: i32 = i32::MIN / 2;
    let mut score = vec![NO; q.len() * n];
    let mut from = vec![usize::MAX; q.len() * n];
    for (i, &want) in q.iter().enumerate() {
        let (mut best, mut best_at) = (NO, usize::MAX);
        for j in 0..n {
            if i > 0 && j > 0 {
                let prev = score[(i - 1) * n + j - 1];
                if best > NO {
                    best -= 1;
                }
                if prev > best {
                    (best, best_at) = (prev, j - 1);
                }
            }
            if t[j] != want {
                continue;
            }
            let here = MATCHED + if starts(j) { WORD_START } else { 0 };
            if i == 0 {
                score[j] = here - (j as i32).min(4);
                continue;
            }
            let next_to = if j > 0 && score[(i - 1) * n + j - 1] > NO { score[(i - 1) * n + j - 1] + NEXT_TO } else { NO };
            let (s, f) = if next_to >= best { (next_to, j.wrapping_sub(1)) } else { (best, best_at) };
            if s > NO {
                score[i * n + j] = s + here;
                from[i * n + j] = f;
            }
        }
    }
    let last = q.len() - 1;
    let (mut j, best) = (0..n).map(|j| (j, score[last * n + j])).filter(|&(_, s)| s > NO).max_by_key(|&(j, s)| (s, std::cmp::Reverse(j)))?;
    let mut at = vec![0; q.len()];
    for i in (0..q.len()).rev() {
        at[i] = j;
        j = from[i * n + j];
    }
    Some(Fuzzy { score: best, at })
}

/// What a picker keeps its highlight under, so a new query starts from the top.
pub(crate) fn hashed(query: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    query.trim().hash(&mut h);
    h.finish()
}

/// The bytes of `text` that `query` matched, as ranges of neighbouring characters, for marking them.
pub(crate) fn marked(query: &str, text: &str) -> Vec<(u32, u32)> {
    let Some(found) = fuzzy(query, text) else { return Vec::new() };
    let mut ranges: Vec<(u32, u32)> = Vec::new();
    let mut at = found.at.into_iter().peekable();
    for (k, (byte, c)) in text.char_indices().enumerate() {
        if at.peek() != Some(&k) {
            continue;
        }
        at.next();
        let (start, end) = (byte as u32, (byte + c.len_utf8()) as u32);
        match ranges.last_mut() {
            Some(last) if last.1 == start => last.1 = end,
            _ => ranges.push((start, end)),
        }
    }
    ranges
}
