//! Independent minimal baseline two-by-two grayscale JPEG.
pub fn jpeg() -> Vec<u8> {
    fn segment(bytes: &mut Vec<u8>, marker: u8, payload: &[u8]) {
        bytes.extend([0xff, marker]);
        bytes.extend(((payload.len() + 2) as u16).to_be_bytes());
        bytes.extend(payload);
    }
    let mut bytes = vec![0xff, 0xd8];
    let mut quantization = vec![0];
    quantization.extend([1; 64]);
    segment(&mut bytes, 0xdb, &quantization);
    segment(&mut bytes, 0xc0, &[8, 0, 2, 0, 2, 1, 1, 0x11, 0]);
    let mut huffman = Vec::new();
    for table in [0, 0x10] {
        huffman.extend([table, 1]);
        huffman.extend([0; 15]);
        huffman.push(0);
    }
    segment(&mut bytes, 0xc4, &huffman);
    segment(&mut bytes, 0xda, &[1, 1, 0, 0, 63, 0]);
    bytes.extend([0x3f, 0xff, 0xd9]);
    bytes
}
