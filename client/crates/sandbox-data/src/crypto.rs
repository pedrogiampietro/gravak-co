//! ChaCha20-Poly1305 (RFC 8439), the authenticated cipher of the Gravak
//! pack (`pack.rs`). Written out here so the client needs no extra crate;
//! the tests check it against the RFC's own vectors.

const SIGMA: [u32; 4] = [0x6170_7865, 0x3320_646e, 0x7962_2d32, 0x6b20_6574];

fn le32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

#[inline(always)]
fn quarter(state: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
    state[a] = state[a].wrapping_add(state[b]);
    state[d] = (state[d] ^ state[a]).rotate_left(16);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_left(12);
    state[a] = state[a].wrapping_add(state[b]);
    state[d] = (state[d] ^ state[a]).rotate_left(8);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_left(7);
}

/// One 64-byte ChaCha20 keystream block.
pub(crate) fn chacha20_block(key: &[u8; 32], counter: u32, nonce: &[u8; 12]) -> [u8; 64] {
    let mut input = [0_u32; 16];
    input[..4].copy_from_slice(&SIGMA);
    for i in 0..8 {
        input[4 + i] = le32(&key[i * 4..]);
    }
    input[12] = counter;
    for i in 0..3 {
        input[13 + i] = le32(&nonce[i * 4..]);
    }
    let mut state = input;
    for _ in 0..10 {
        quarter(&mut state, 0, 4, 8, 12);
        quarter(&mut state, 1, 5, 9, 13);
        quarter(&mut state, 2, 6, 10, 14);
        quarter(&mut state, 3, 7, 11, 15);
        quarter(&mut state, 0, 5, 10, 15);
        quarter(&mut state, 1, 6, 11, 12);
        quarter(&mut state, 2, 7, 8, 13);
        quarter(&mut state, 3, 4, 9, 14);
    }
    let mut out = [0_u8; 64];
    for i in 0..16 {
        out[i * 4..i * 4 + 4].copy_from_slice(&state[i].wrapping_add(input[i]).to_le_bytes());
    }
    out
}

/// XORs `data` with the keystream starting at block `counter`.
pub(crate) fn chacha20_xor(key: &[u8; 32], counter: u32, nonce: &[u8; 12], data: &mut [u8]) {
    for (index, chunk) in data.chunks_mut(64).enumerate() {
        let block = chacha20_block(key, counter.wrapping_add(index as u32), nonce);
        for (byte, stream) in chunk.iter_mut().zip(block) {
            *byte ^= stream;
        }
    }
}

/// Poly1305 one-time authenticator (26-bit limbs).
struct Poly1305 {
    r: [u32; 5],
    s: [u32; 4],
    h: [u32; 5],
    pad: [u32; 4],
    buffer: [u8; 16],
    buffered: usize,
}

impl Poly1305 {
    fn new(key: &[u8; 32]) -> Self {
        let r = [
            le32(&key[0..]) & 0x03ff_ffff,
            (le32(&key[3..]) >> 2) & 0x03ff_ff03,
            (le32(&key[6..]) >> 4) & 0x03ff_c0ff,
            (le32(&key[9..]) >> 6) & 0x03f0_3fff,
            (le32(&key[12..]) >> 8) & 0x000f_ffff,
        ];
        Self {
            r,
            s: [r[1] * 5, r[2] * 5, r[3] * 5, r[4] * 5],
            h: [0; 5],
            pad: [
                le32(&key[16..]),
                le32(&key[20..]),
                le32(&key[24..]),
                le32(&key[28..]),
            ],
            buffer: [0; 16],
            buffered: 0,
        }
    }

    fn block(&mut self, block: &[u8; 16], high_bit: u32) {
        let [r0, r1, r2, r3, r4] = self.r.map(u64::from);
        let [s1, s2, s3, s4] = self.s.map(u64::from);
        let h = &mut self.h;
        h[0] += le32(&block[0..]) & 0x03ff_ffff;
        h[1] += (le32(&block[3..]) >> 2) & 0x03ff_ffff;
        h[2] += (le32(&block[6..]) >> 4) & 0x03ff_ffff;
        h[3] += (le32(&block[9..]) >> 6) & 0x03ff_ffff;
        h[4] += (le32(&block[12..]) >> 8) | high_bit;
        let [h0, h1, h2, h3, h4] = h.map(u64::from);

        let d0 = h0 * r0 + h1 * s4 + h2 * s3 + h3 * s2 + h4 * s1;
        let mut d1 = h0 * r1 + h1 * r0 + h2 * s4 + h3 * s3 + h4 * s2;
        let mut d2 = h0 * r2 + h1 * r1 + h2 * r0 + h3 * s4 + h4 * s3;
        let mut d3 = h0 * r3 + h1 * r2 + h2 * r1 + h3 * r0 + h4 * s4;
        let mut d4 = h0 * r4 + h1 * r3 + h2 * r2 + h3 * r1 + h4 * r0;

        let mut carry = d0 >> 26;
        h[0] = (d0 & 0x03ff_ffff) as u32;
        d1 += carry;
        carry = d1 >> 26;
        h[1] = (d1 & 0x03ff_ffff) as u32;
        d2 += carry;
        carry = d2 >> 26;
        h[2] = (d2 & 0x03ff_ffff) as u32;
        d3 += carry;
        carry = d3 >> 26;
        h[3] = (d3 & 0x03ff_ffff) as u32;
        d4 += carry;
        carry = d4 >> 26;
        h[4] = (d4 & 0x03ff_ffff) as u32;
        h[0] += (carry * 5) as u32;
        let carry = h[0] >> 26;
        h[0] &= 0x03ff_ffff;
        h[1] += carry;
    }

    fn update(&mut self, mut data: &[u8]) {
        if self.buffered > 0 {
            let take = (16 - self.buffered).min(data.len());
            self.buffer[self.buffered..self.buffered + take].copy_from_slice(&data[..take]);
            self.buffered += take;
            data = &data[take..];
            if self.buffered < 16 {
                return;
            }
            let block = self.buffer;
            self.block(&block, 1 << 24);
            self.buffered = 0;
        }
        let mut chunks = data.chunks_exact(16);
        for chunk in &mut chunks {
            self.block(chunk.try_into().expect("16-byte chunk"), 1 << 24);
        }
        let rest = chunks.remainder();
        self.buffer[..rest.len()].copy_from_slice(rest);
        self.buffered = rest.len();
    }

    /// Zero padding to the next 16-byte boundary (the AEAD's pad16).
    fn pad16(&mut self) {
        if self.buffered > 0 {
            let zeros = [0_u8; 16];
            self.update(&zeros[..16 - self.buffered]);
        }
    }

    fn finish(mut self) -> [u8; 16] {
        if self.buffered > 0 {
            let mut block = [0_u8; 16];
            block[..self.buffered].copy_from_slice(&self.buffer[..self.buffered]);
            block[self.buffered] = 1;
            self.block(&block, 0);
        }
        let mut h = self.h;
        let mut carry = h[1] >> 26;
        h[1] &= 0x03ff_ffff;
        h[2] += carry;
        carry = h[2] >> 26;
        h[2] &= 0x03ff_ffff;
        h[3] += carry;
        carry = h[3] >> 26;
        h[3] &= 0x03ff_ffff;
        h[4] += carry;
        carry = h[4] >> 26;
        h[4] &= 0x03ff_ffff;
        h[0] += carry * 5;
        carry = h[0] >> 26;
        h[0] &= 0x03ff_ffff;
        h[1] += carry;

        // h + -p: kept when h >= p.
        let mut g = [0_u32; 5];
        g[0] = h[0].wrapping_add(5);
        carry = g[0] >> 26;
        g[0] &= 0x03ff_ffff;
        g[1] = h[1].wrapping_add(carry);
        carry = g[1] >> 26;
        g[1] &= 0x03ff_ffff;
        g[2] = h[2].wrapping_add(carry);
        carry = g[2] >> 26;
        g[2] &= 0x03ff_ffff;
        g[3] = h[3].wrapping_add(carry);
        carry = g[3] >> 26;
        g[3] &= 0x03ff_ffff;
        g[4] = h[4].wrapping_add(carry).wrapping_sub(1 << 26);

        let mask = (g[4] >> 31).wrapping_sub(1);
        for i in 0..5 {
            h[i] = (h[i] & !mask) | (g[i] & mask);
        }

        let words = [
            h[0] | (h[1] << 26),
            (h[1] >> 6) | (h[2] << 20),
            (h[2] >> 12) | (h[3] << 14),
            (h[3] >> 18) | (h[4] << 8),
        ];
        let mut tag = [0_u8; 16];
        let mut f = 0_u64;
        for i in 0..4 {
            f = u64::from(words[i]) + u64::from(self.pad[i]) + (f >> 32);
            tag[i * 4..i * 4 + 4].copy_from_slice(&(f as u32).to_le_bytes());
        }
        tag
    }
}

fn aead_tag(key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], ciphertext: &[u8]) -> [u8; 16] {
    let block = chacha20_block(key, 0, nonce);
    let one_time: [u8; 32] = block[..32].try_into().expect("32 bytes");
    let mut mac = Poly1305::new(&one_time);
    mac.update(aad);
    mac.pad16();
    mac.update(ciphertext);
    mac.pad16();
    mac.update(&(aad.len() as u64).to_le_bytes());
    mac.update(&(ciphertext.len() as u64).to_le_bytes());
    mac.finish()
}

pub(crate) const TAG_LENGTH: usize = 16;

/// Encrypts `data` in place and appends the 16-byte tag.
pub(crate) fn seal(key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], data: &mut Vec<u8>) {
    chacha20_xor(key, 1, nonce, data);
    let tag = aead_tag(key, nonce, aad, data);
    data.extend_from_slice(&tag);
}

/// Checks the tag and decrypts in place; `false` (and `data` untouched
/// beyond the tag) when anything was altered.
pub(crate) fn open(key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], data: &mut Vec<u8>) -> bool {
    if data.len() < TAG_LENGTH {
        return false;
    }
    let split = data.len() - TAG_LENGTH;
    let expected = aead_tag(key, nonce, aad, &data[..split]);
    let difference = expected
        .iter()
        .zip(&data[split..])
        .fold(0_u8, |acc, (a, b)| acc | (a ^ b));
    if difference != 0 {
        return false;
    }
    data.truncate(split);
    chacha20_xor(key, 1, nonce, data);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(text: &str) -> Vec<u8> {
        let text: String = text.chars().filter(char::is_ascii_hexdigit).collect();
        (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn chacha20_block_matches_rfc8439_2_3_2() {
        let key: [u8; 32] = hex("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f")
            .try_into()
            .unwrap();
        let nonce: [u8; 12] = hex("000000090000004a00000000").try_into().unwrap();
        let block = chacha20_block(&key, 1, &nonce);
        assert_eq!(
            block[..16].to_vec(),
            hex("10f1e7e4d13b5915500fdd1fa32071c4")
        );
    }

    #[test]
    fn poly1305_matches_rfc8439_2_5_2() {
        let key: [u8; 32] = hex("85d6be7857556d337f4452fe42d506a80103808afb0db2fd4abff6af4149f51b")
            .try_into()
            .unwrap();
        let mut mac = Poly1305::new(&key);
        mac.update(b"Cryptographic Forum Research Group");
        assert_eq!(mac.finish().to_vec(), hex("a8061dc1305136c6c22b8baf0c0127a9"));
    }

    #[test]
    fn aead_matches_rfc8439_2_8_2() {
        let key: [u8; 32] = hex("808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f")
            .try_into()
            .unwrap();
        let nonce: [u8; 12] = hex("070000004041424344454647").try_into().unwrap();
        let aad = hex("50515253c0c1c2c3c4c5c6c7");
        let plain = b"Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.";
        let mut data = plain.to_vec();
        seal(&key, &nonce, &aad, &mut data);
        assert_eq!(data[..16].to_vec(), hex("d31a8d34648e60db7b86afbc53ef7ec2"));
        assert_eq!(
            data[data.len() - 16..].to_vec(),
            hex("1ae10b594f09e26a7e902ecbd0600691")
        );
        assert!(open(&key, &nonce, &aad, &mut data));
        assert_eq!(data, plain);
    }

    #[test]
    fn a_changed_byte_is_rejected() {
        let key = [7_u8; 32];
        let nonce = [1_u8; 12];
        let mut data = b"some game file".to_vec();
        seal(&key, &nonce, b"", &mut data);
        data[3] ^= 1;
        assert!(!open(&key, &nonce, b"", &mut data));
    }
}
