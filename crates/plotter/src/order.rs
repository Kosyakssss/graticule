//! Stacking order as fractional keys: a key made between two others sorts between them, so putting a shape anywhere in the stack changes only that shape, and two devices doing it at once never renumber each other.

const DIGITS: &[u8; 62] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const BASE: u8 = 62;

/// A key that sorts after `before` and before `after`; `None` for either means the start or the end of the stack. `before` must sort before `after`.
pub fn between(before: Option<&str>, after: Option<&str>) -> String {
    let a: Vec<u8> = before.unwrap_or("").bytes().filter_map(value).collect();
    let b: Option<Vec<u8>> = after.map(|s| s.bytes().filter_map(value).collect());
    midpoint(&a, b.as_deref()).into_iter().map(|d| DIGITS[d as usize] as char).collect()
}

fn value(c: u8) -> Option<u8> {
    DIGITS.iter().position(|&d| d == c).map(|i| i as u8)
}

fn midpoint(a: &[u8], b: Option<&[u8]>) -> Vec<u8> {
    if let Some(b) = b {
        let shared = (0..b.len()).take_while(|&i| a.get(i).copied().unwrap_or(0) == b[i]).count();
        if shared > 0 {
            let mut key = b[..shared].to_vec();
            key.extend(midpoint(a.get(shared..).unwrap_or(&[]), Some(&b[shared..])));
            return key;
        }
    }
    let low = a.first().copied().unwrap_or(0);
    let high = b.map_or(BASE, |b| b.first().copied().unwrap_or(0));
    if high - low > 1 {
        return vec![(low + high) / 2];
    }
    if let Some(b) = b.filter(|b| b.len() > 1) {
        return vec![b[0]];
    }
    let mut key = vec![low];
    key.extend(midpoint(a.get(1..).unwrap_or(&[]), None));
    key
}
