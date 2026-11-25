//! WOFF (Web Open Font Format 1.0, W3C REC 2012) to the plain font it wraps: its tables, each
//! zlib-compressed when that is smaller, become an sfnt (TrueType / OpenType) again for the text
//! engine, which reads those only.

/// The sfnt of WOFF font `data`, `None` when it isn't one (or is malformed).
pub(crate) fn sfnt(data: &[u8]) -> Option<Vec<u8>> {
    let be32 = |at: usize| data.get(at..at + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]));
    let be16 = |at: usize| data.get(at..at + 2).map(|b| u16::from_be_bytes([b[0], b[1]]));
    if data.get(..4)? != b"wOFF" {
        return None;
    }
    let flavor = be32(4)?;
    let tables = usize::from(be16(12)?);
    let total = be32(16)? as usize;
    // the sfnt's header: its tables' count and search hints, then a record for each
    let mut out = Vec::with_capacity(total.min(64 << 20));
    out.extend_from_slice(&flavor.to_be_bytes());
    out.extend_from_slice(&(tables as u16).to_be_bytes());
    let pow = if tables == 0 { 0 } else { usize::BITS - 1 - tables.leading_zeros() };
    let range = (1u16 << pow) * 16;
    out.extend_from_slice(&range.to_be_bytes());
    out.extend_from_slice(&(pow as u16).to_be_bytes());
    out.extend_from_slice(&((tables as u16 * 16).wrapping_sub(range)).to_be_bytes());
    let dir = 12 + 16 * tables;
    out.resize(dir, 0);
    for k in 0..tables {
        let e = 44 + 20 * k;
        let (tag, offset, comp, orig, sum) = (be32(e)?, be32(e + 4)? as usize, be32(e + 8)? as usize, be32(e + 12)? as usize, be32(e + 16)?);
        let src = data.get(offset..offset.checked_add(comp)?)?;
        let at = out.len();
        if comp < orig {
            let table = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(src, orig).ok()?;
            if table.len() != orig {
                return None;
            }
            out.extend_from_slice(&table);
        } else {
            out.extend_from_slice(src);
        }
        // records point at each table, 4-byte aligned
        let r = 12 + 16 * k;
        out[r..r + 4].copy_from_slice(&tag.to_be_bytes());
        out[r + 4..r + 8].copy_from_slice(&sum.to_be_bytes());
        out[r + 8..r + 12].copy_from_slice(&(at as u32).to_be_bytes());
        out[r + 12..r + 16].copy_from_slice(&(orig as u32).to_be_bytes());
        out.resize(out.len().next_multiple_of(4), 0);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::sfnt;

    /// A WOFF of one uncompressed and one compressed table unwraps to an sfnt with both.
    #[test]
    fn unwraps_tables() {
        let plain = b"abcd".to_vec();
        let deflated = miniz_oxide::deflate::compress_to_vec_zlib(&[7u8; 64], 9);
        let mut w = Vec::new();
        w.extend_from_slice(b"wOFF");
        w.extend_from_slice(&0x0001_0000u32.to_be_bytes()); // flavor: TrueType
        w.extend_from_slice(&0u32.to_be_bytes()); // length (unchecked)
        w.extend_from_slice(&2u16.to_be_bytes());
        w.extend_from_slice(&0u16.to_be_bytes());
        w.extend_from_slice(&0u32.to_be_bytes()); // totalSfntSize
        w.resize(44, 0);
        let first = 44 + 40;
        let second = first + 4;
        for (tag, at, comp, orig) in [(*b"head", first, 4, 4), (*b"glyf", second, deflated.len(), 64)] {
            w.extend_from_slice(&tag);
            w.extend_from_slice(&(at as u32).to_be_bytes());
            w.extend_from_slice(&(comp as u32).to_be_bytes());
            w.extend_from_slice(&(orig as u32).to_be_bytes());
            w.extend_from_slice(&0u32.to_be_bytes());
        }
        w.extend_from_slice(&plain);
        w.extend_from_slice(&deflated);
        let s = sfnt(&w).unwrap();
        assert_eq!(&s[..4], &[0, 1, 0, 0]);
        assert_eq!(&s[12..16], b"head");
        let glyf = u32::from_be_bytes([s[36], s[37], s[38], s[39]]) as usize;
        assert_eq!(&s[glyf..glyf + 64], &[7u8; 64]);
        assert!(sfnt(b"OTTO....").is_none());
    }
}
