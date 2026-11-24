//! Random strings.

/// Random bytes as hex, twice as many characters.
pub fn hex(bytes: usize) -> String {
    let mut buffer = vec![0u8; bytes];
    rand::fill(&mut buffer[..]);
    buffer.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Random letters and digits, for a name a person may read.
pub fn letters_and_digits(length: usize) -> String {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    (0..length)
        .map(|_| ALPHABET[rand::random_range(0..ALPHABET.len())] as char)
        .collect()
}

/// Random decimal digits, leading zeros included.
pub fn digits(count: u32) -> String {
    let below = 10u64.pow(count);
    format!(
        "{:0width$}",
        rand::random_range(0..below),
        width = count as usize
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_lengths() {
        assert_eq!(hex(16).len(), 32);
        assert_eq!(letters_and_digits(8).len(), 8);
        assert_eq!(digits(6).len(), 6);
        assert!(digits(6).chars().all(|c| c.is_ascii_digit()));
    }
}
