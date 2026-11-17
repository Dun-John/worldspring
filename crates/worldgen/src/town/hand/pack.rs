//! Compact record encoding: base64, packed ids and quantised points.

use super::*;

pub(super) const WARDS: [Ward; 13] = [Ward::Plaza, Ward::Castle, Ward::Temple, Ward::Merchant, Ward::Craft, Ward::Noble, Ward::Common, Ward::Slum, Ward::Docks, Ward::Military, Ward::Farm, Ward::Rural, Ward::Park];
pub(super) const STRUCTURES: [Structure; 3] = [Structure::Roofed, Structure::Open, Structure::Ruin];
pub(super) const ROOFS: [RoofStyle; 3] = [RoofStyle::Hip, RoofStyle::Battlements, RoofStyle::Cone];
pub(super) const PLAZAS: [PlazaKind; 12] = [
    PlazaKind::Market,
    PlazaKind::MarketSparse,
    PlazaKind::MarketDay,
    PlazaKind::Civic,
    PlazaKind::Quay,
    PlazaKind::Green,
    PlazaKind::Yard,
    PlazaKind::TempleClose,
    PlazaKind::WellSquare,
    PlazaKind::FountainSquare,
    PlazaKind::Crossroads,
    PlazaKind::Bailey,
];
pub(super) const PARKS: [ParkKind; 4] = [ParkKind::Formal, ParkKind::Common, ParkKind::Cemetery, ParkKind::Horses];
pub(super) const KEEPS: [KeepKind; 6] = [KeepKind::Great, KeepKind::Hall, KeepKind::L, KeepKind::T, KeepKind::Shell, KeepKind::Plain];
pub(super) const PARTS: [PartKind; 6] = [PartKind::Body, PartKind::Tower { round: false }, PartKind::Tower { round: true }, PartKind::Forebuilding, PartKind::Wing, PartKind::Apse];

pub(super) fn index_of<T: PartialEq>(list: &[T], v: &T) -> u8 {
    list.iter().position(|x| x == v).expect("every kind is listed") as u8
}

pub(super) fn nth<T: Copy>(list: &[T], i: u8, what: &str) -> Result<T, String> {
    list.get(i as usize).copied().ok_or_else(|| format!("kept building: no {what} {i}"))
}

/// Bytes as base64url without padding.
fn b64(bytes: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len() * 4 / 3 + 3);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for k in 0..=c.len() {
            out.push(ABC[(n >> (18 - 6 * k) & 63) as usize] as char);
        }
    }
    out
}

/// `b64` undone.
fn unb64(s: &str) -> Result<Vec<u8>, String> {
    let val = |c: u8| -> Result<u32, String> {
        Ok(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            _ => return Err(format!("packed numbers: {:?} is no digit", c as char)),
        } as u32)
    };
    let mut bytes = Vec::with_capacity(s.len() * 3 / 4);
    for c in s.as_bytes().chunks(4) {
        if c.len() < 2 {
            return Err("packed numbers: cut short".into());
        }
        let mut n = 0;
        for (k, &d) in c.iter().enumerate() {
            n |= val(d)? << (18 - 6 * k);
        }
        bytes.extend((0..c.len() - 1).map(|k| (n >> (16 - 8 * k) & 255) as u8));
    }
    Ok(bytes)
}

/// Numbers packed exactly: each f64's bits, little-endian, base64url without padding.
pub fn pack(fs: &[f64]) -> String {
    b64(&fs.iter().flat_map(|f| f.to_bits().to_le_bytes()).collect::<Vec<u8>>())
}

/// `pack` undone.
pub fn unpack(s: &str) -> Result<Vec<f64>, String> {
    let bytes = unb64(s)?;
    if bytes.len() % 8 != 0 {
        return Err("packed numbers: not whole numbers".into());
    }
    Ok(bytes.chunks(8).map(|b| f64::from_bits(u64::from_le_bytes(b.try_into().unwrap()))).collect())
}

/// Points packed on a 1/64-ft grid (`~`, then each number's step from the last, zigzag varints,
/// base64url): a quarter of `pk`'s size, for long lines whose places need no more (walls, towers).
pub(in crate::town) fn pkq(pts: &[P]) -> String {
    if pts.is_empty() {
        return String::new();
    }
    let mut bytes: Vec<u8> = Vec::with_capacity(pts.len() * 4);
    let mut last = [0i64; 2];
    for p in pts {
        for k in 0..2 {
            let v = crate::core::round(p[k] * 64.0) as i64;
            let d = v - last[k];
            last[k] = v;
            let mut z = ((d << 1) ^ (d >> 63)) as u64;
            loop {
                let b = (z & 127) as u8;
                z >>= 7;
                if z == 0 {
                    bytes.push(b);
                    break;
                }
                bytes.push(b | 128);
            }
        }
    }
    format!("~{}", b64(&bytes))
}

/// Towers' forms packed short (`~` centres `|` shapes, two hex digits each `|` ways, each on a
/// 1/4096 grid), or as `pack` (five numbers each) without the `~`.
pub(super) fn forms_pk(forms: &[(P, u8, P)]) -> String {
    if forms.is_empty() {
        return String::new();
    }
    let at: Vec<P> = forms.iter().map(|f| f.0).collect();
    let ways: Vec<P> = forms.iter().map(|f| mul(f.2, 64.0)).collect();
    let shapes: String = forms.iter().map(|f| format!("{:02x}", f.1)).collect();
    format!("{}|{shapes}|{}", pkq(&at), pkq(&ways))
}

pub(super) fn forms_unpk(s: &str) -> Vec<(P, u8, P)> {
    if !s.starts_with('~') {
        return unpack(s).unwrap_or_default().chunks_exact(5).map(|c| ([c[0], c[1]], c[2] as u8, [c[3], c[4]])).collect();
    }
    let mut it = s.split('|');
    let (at, shapes, ways) = (unpk(it.next().unwrap_or("")), it.next().unwrap_or(""), unpk(it.next().unwrap_or("")));
    let shapes: Vec<u8> = shapes.as_bytes().chunks(2).map(|c| std::str::from_utf8(c).ok().and_then(|h| u8::from_str_radix(h, 16).ok()).unwrap_or(0)).collect();
    at.into_iter().zip(shapes).zip(ways).map(|((a, k), w)| (a, k, mul(w, 1.0 / 64.0))).collect()
}

pub(super) fn unpkq(s: &str) -> Vec<P> {
    let Ok(bytes) = unb64(s) else { return Vec::new() };
    let (mut nums, mut z, mut shift, mut last) = (Vec::new(), 0u64, 0, [0i64; 2]);
    for b in bytes {
        z |= ((b & 127) as u64) << shift;
        shift += 7;
        if b & 128 == 0 {
            let k = nums.len() % 2;
            last[k] += (z >> 1) as i64 ^ -((z & 1) as i64);
            nums.push(last[k] as f64 / 64.0);
            (z, shift) = (0, 0);
        }
    }
    nums.chunks_exact(2).map(|c| [c[0], c[1]]).collect()
}

pub(super) fn pts_of(fs: &[f64]) -> Vec<P> {
    fs.chunks(2).map(|c| [c[0], c[1]]).collect()
}
