use anyhow::{ensure, Result};
pub fn png_payload_offset(bytes: &[u8]) -> Result<usize> {
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Ok(0);
    }
    let mut pos = 8;
    loop {
        ensure!(pos + 12 <= bytes.len(), "PNG prefix bị cắt hoặc quá lớn");
        let size = u32::from_be_bytes(bytes[pos..pos + 4].try_into()?) as usize;
        let next = pos
            .checked_add(size)
            .and_then(|s| s.checked_add(12))
            .ok_or_else(|| anyhow::anyhow!("PNG chunk quá lớn"))?;
        ensure!(next <= bytes.len(), "PNG chunk bị cắt");
        if &bytes[pos + 4..pos + 8] == b"IEND" {
            ensure!(
                size == 0 && next < bytes.len(),
                "PNG không có payload media"
            );
            return Ok(next);
        }
        pos = next;
    }
}
pub fn validate_media(head: &[u8], png_allowed: bool) -> Result<usize> {
    let offset = if png_allowed {
        png_payload_offset(head)?
    } else {
        0
    };
    let p = &head[offset..];
    let mp4 = p.len() > 8
        && [b"ftyp", b"styp", b"moof", b"moov", b"sidx", b"mdat"]
            .iter()
            .any(|k| &p[4..8] == *k);
    let ts = (0..p.len().min(188)).any(|i| p[i] == 0x47 && p.get(i + 188) == Some(&0x47));
    let audio = p.starts_with(b"ID3") || (p.len() > 2 && p[0] == 0xff && (p[1] & 0xf0) == 0xf0);
    ensure!(
        mp4 || ts || audio,
        "Response không phải media hỗ trợ (có thể là trang lỗi)"
    );
    Ok(offset)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strips_chunks_not_fixed_bytes() {
        let mut x = b"\x89PNG\r\n\x1a\n".to_vec();
        x.extend_from_slice(&[0, 0, 0, 0]);
        x.extend_from_slice(b"IEND");
        x.extend_from_slice(&[0; 4]);
        x.extend_from_slice(b"\0\0\0\x10ftypisom");
        assert_eq!(validate_media(&x, true).unwrap(), 20);
        assert!(validate_media(&x, false).is_err());
    }
    #[test]
    fn rejects_images_and_html() {
        assert!(png_payload_offset(b"\x89PNG\r\n\x1a\n").is_err());
        assert!(validate_media(b"<html>access denied</html>", false).is_err());
    }
}
