// Synthetic charset shift of the existing OFL Noto fixture. Glyph outlines are unchanged.
use skrifa::{FontRef, raw::TableProvider};
pub fn shifted_cids_font(font_bytes: &[u8]) -> Vec<u8> {
    let face = FontRef::new(font_bytes).unwrap();
    let table = face.cff().unwrap();
    let raw = table.offset_data().as_bytes();
    let base = raw.as_ptr() as usize - font_bytes.as_ptr() as usize;
    let top = table.top_dicts().get(0).unwrap();
    let top_base = top.as_ptr() as usize - font_bytes.as_ptr() as usize;
    let offset = skrifa::raw::ps::cff::dict::entries(top, None)
        .find_map(|entry| match entry.unwrap() {
            skrifa::raw::ps::cff::dict::Entry::Charset(offset) => Some(offset),
            _ => None,
        })
        .unwrap();
    assert_eq!(&raw[offset..offset + 5], &[2, 0, 1, 255, 253]);
    let cid_count = top
        .windows(7)
        .position(|bytes| bytes == [29, 0, 0, 255, 255, 12, 34])
        .unwrap();
    let mut result = font_bytes.to_vec();
    result[base + offset + 1..base + offset + 3].copy_from_slice(&2u16.to_be_bytes());
    result[top_base + cid_count + 1..top_base + cid_count + 5]
        .copy_from_slice(&65536u32.to_be_bytes());
    let num_tables = usize::from(u16::from_be_bytes([font_bytes[4], font_bytes[5]]));
    let mut cff_record = None;
    let mut head = None;
    for (index, record) in font_bytes[12..12 + num_tables * 16]
        .chunks_exact(16)
        .enumerate()
    {
        if &record[..4] == b"CFF " {
            cff_record = Some(12 + index * 16);
        }
        if &record[..4] == b"head" {
            head = Some(u32::from_be_bytes(record[8..12].try_into().unwrap()) as usize);
        }
    }
    fn checksum(bytes: &[u8]) -> u32 {
        bytes.chunks(4).fold(0u32, |sum, chunk| {
            let mut value = [0u8; 4];
            value[..chunk.len()].copy_from_slice(chunk);
            sum.wrapping_add(u32::from_be_bytes(value))
        })
    }
    let checksum_offset = cff_record.unwrap() + 4;
    let cff_checksum = checksum(&result[base..base + raw.len()]);
    result[checksum_offset..checksum_offset + 4].copy_from_slice(&cff_checksum.to_be_bytes());
    let adjustment = head.unwrap() + 8;
    result[adjustment..adjustment + 4].fill(0);
    let correction = 0xB1B0AFBAu32.wrapping_sub(checksum(&result));
    result[adjustment..adjustment + 4].copy_from_slice(&correction.to_be_bytes());
    assert_eq!(checksum(&result), 0xB1B0AFBA);
    let changed = FontRef::new(&result).unwrap();
    let changed_cff = changed.cff().unwrap();
    let font = skrifa::raw::ps::cff::CffFontRef::new(changed_cff.offset_data().as_bytes(), 0, None)
        .unwrap();
    let charset = font.charset().unwrap();
    assert_eq!(
        charset
            .string_id(skrifa::GlyphId::new(34))
            .unwrap()
            .to_u16(),
        35
    );
    assert_eq!(
        charset
            .string_id(skrifa::GlyphId::new(9544))
            .unwrap()
            .to_u16(),
        9545
    );
    result
}
