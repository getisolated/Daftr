//! SHA-1 (RFC 3174), seulement pour dériver des UUID version 5.
//!
//! SHA-1 n'est plus sûr comme fonction de hachage cryptographique ; il n'est utilisé ici que
//! parce que les UUID version 5 l'imposent (RFC 9562), pour obtenir un identifiant stable.

const INITIAL_STATE: [u32; 5] = [
    0x6745_2301,
    0xEFCD_AB89,
    0x98BA_DCFE,
    0x1032_5476,
    0xC3D2_E1F0,
];
const BLOCK_LENGTH: usize = 64;

/// Empreinte SHA-1 (20 octets) des morceaux mis bout à bout.
pub(crate) fn digest(parts: &[&[u8]]) -> [u8; 20] {
    let message: Vec<u8> = parts.concat();
    let mut state = INITIAL_STATE;

    for block in padded(&message).chunks_exact(BLOCK_LENGTH) {
        compress(&mut state, block);
    }

    let mut output = [0_u8; 20];
    for (chunk, word) in output.chunks_exact_mut(4).zip(state) {
        chunk.copy_from_slice(&word.to_be_bytes());
    }
    output
}

/// Le message suivi de `0x80`, de zéros et de sa longueur en bits (64 bits gros-boutiste),
/// pour atteindre un multiple de 64 octets.
fn padded(message: &[u8]) -> Vec<u8> {
    let bit_length = (message.len() as u64).wrapping_mul(8);
    let mut padded = message.to_vec();

    padded.push(0x80);
    while padded.len() % BLOCK_LENGTH != BLOCK_LENGTH - 8 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_length.to_be_bytes());
    padded
}

/// Traite un bloc de 64 octets.
fn compress(state: &mut [u32; 5], block: &[u8]) {
    let schedule = message_schedule(block);
    let [mut a, mut b, mut c, mut d, mut e] = *state;

    for (round, word) in schedule.iter().enumerate() {
        let (mixed, constant) = round_function(round, b, c, d);
        let next = a
            .rotate_left(5)
            .wrapping_add(mixed)
            .wrapping_add(e)
            .wrapping_add(constant)
            .wrapping_add(*word);

        e = d;
        d = c;
        c = b.rotate_left(30);
        b = a;
        a = next;
    }

    for (current, added) in state.iter_mut().zip([a, b, c, d, e]) {
        *current = current.wrapping_add(added);
    }
}

/// Les 80 mots du bloc : les 16 mots du bloc, puis leurs combinaisons.
fn message_schedule(block: &[u8]) -> [u32; 80] {
    let mut words = [0_u32; 80];

    for (word, bytes) in words.iter_mut().zip(block.chunks_exact(4)) {
        *word = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    }
    for index in 16..80 {
        let mixed = words[index - 3] ^ words[index - 8] ^ words[index - 14] ^ words[index - 16];
        words[index] = mixed.rotate_left(1);
    }
    words
}

/// Fonction et constante de la ronde.
fn round_function(round: usize, b: u32, c: u32, d: u32) -> (u32, u32) {
    match round {
        0..=19 => ((b & c) | (!b & d), 0x5A82_7999),
        20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
        40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
        _ => (b ^ c ^ d, 0xCA62_C1D6),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[test]
    fn matches_the_reference_vectors() {
        assert_eq!(
            hex(&digest(&[b""])),
            "da39a3ee5e6b4b0d3255bfef95601890afd80709"
        );
        assert_eq!(
            hex(&digest(&[b"abc"])),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        let two_blocks = b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";
        assert_eq!(
            hex(&digest(&[two_blocks])),
            "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
        );
    }

    #[test]
    fn hashes_the_parts_as_one_message() {
        assert_eq!(digest(&[b"ab", b"c"]), digest(&[b"abc"]));
    }

    #[test]
    fn handles_a_million_characters() {
        let message = vec![b'a'; 1_000_000];
        assert_eq!(
            hex(&digest(&[&message])),
            "34aa973cd4c4daa4f61eeb2bdbad27316534016f"
        );
    }
}
