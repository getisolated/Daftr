//! Hexadécimal minuscule des identifiants binaires (UUID, clés, espaces).

/// Les octets en hexadécimal minuscule.
pub(crate) fn encode(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";

    bytes
        .iter()
        .flat_map(|byte| {
            [
                DIGITS[usize::from(byte >> 4)],
                DIGITS[usize::from(byte & 0xF)],
            ]
        })
        .map(char::from)
        .collect()
}

/// Les `N` octets écrits en hexadécimal (minuscules ou majuscules), `None` pour toute autre
/// longueur ou un caractère invalide.
pub(crate) fn decode<const N: usize>(text: &str) -> Option<[u8; N]> {
    let digits = text.as_bytes();
    if digits.len() != N * 2 {
        return None;
    }

    let mut bytes = [0_u8; N];
    for (byte, pair) in bytes.iter_mut().zip(digits.chunks_exact(2)) {
        *byte = (nibble(pair[0])? << 4) | nibble(pair[1])?;
    }
    Some(bytes)
}

fn nibble(digit: u8) -> Option<u8> {
    char::from(digit)
        .to_digit(16)
        .and_then(|value| u8::try_from(value).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        assert_eq!(encode(&[0x00, 0xAB, 0x0F]), "00ab0f");
        assert_eq!(decode::<3>("00AB0f"), Some([0x00, 0xAB, 0x0F]));
    }

    #[test]
    fn refuses_bad_lengths_and_digits() {
        assert_eq!(decode::<2>("abc"), None);
        assert_eq!(decode::<2>("abcdef"), None);
        assert_eq!(decode::<2>("abcg"), None);
        assert_eq!(decode::<1>("é"), None);
    }
}
