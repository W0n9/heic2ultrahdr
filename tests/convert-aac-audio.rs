//! 端到端：把 Apple Live Photo 视频里的音频转码成 AAC。
//!
//! Apple 用 pcm_s16le 编码音频，多数 Android 播放器不支持，因此动态照片
//! 追加视频前需要转码。断言验证产物确实换成了 AAC。

use std::path::Path;

/// 在字节流里找子串首次出现的下标。
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// 遍历 MP4 顶层 box，断言 moov 先于 mdat（faststart）。
fn moov_before_mdat(mp4: &[u8]) -> bool {
    let (mut moov_at, mut mdat_at) = (None, None);
    let mut i = 0usize;
    while i + 8 <= mp4.len() {
        let size = u32::from_be_bytes(mp4[i..i + 4].try_into().unwrap()) as usize;
        let typ = &mp4[i + 4..i + 8];
        if typ == b"moov" && moov_at.is_none() {
            moov_at = Some(i);
        }
        if typ == b"mdat" && mdat_at.is_none() {
            mdat_at = Some(i);
        }
        if moov_at.is_some() && mdat_at.is_some() {
            break;
        }
        let next = match size {
            0 => break,
            1 => i + u64::from_be_bytes(mp4[i + 8..i + 16].try_into().unwrap()) as usize,
            s => i + s,
        };
        if next <= i {
            break;
        }
        i = next;
    }
    matches!((moov_at, mdat_at), (Some(m), Some(d)) if m < d)
}

#[test]
fn transcodes_pcm_audio_to_aac() {
    let output = Path::new("./testoutput/IMG_3853-aac.mp4");
    std::fs::remove_file(output).ok();
    std::fs::create_dir_all("./testoutput").unwrap();

    heic2ultrahdr::i2a::video::VideoAudioEncodeRequest {
        input: "./tests/IMG_3853.MOV".as_ref(),
        output,
        encoder: "aac",
        bit_rate: 128_000,
    }
    .execute()
    .unwrap();

    let bytes = std::fs::read(output).unwrap();

    // MP4 容器：首个 box 必须是 ftyp
    assert!(bytes.len() > 100_000, "transcoded video suspiciously small: {} bytes", bytes.len());
    assert_eq!(&bytes[4..8], b"ftyp", "output is not an MP4 container");
    // 动态照片内嵌视频的封装形态：mp42 brand、moov 前置（faststart）
    assert_eq!(&bytes[8..12], b"mp42", "output is not branded mp42");
    assert!(moov_before_mdat(&bytes), "output is not faststart (moov after mdat)");

    // AAC 在 MP4 里的 sample entry 类型是 mp4a；源视频是 pcm_s16le，不含它，
    // 所以这个断言同时证明了转码确实发生了。
    assert!(
        find(&bytes, b"mp4a").is_some(),
        "no AAC sample entry (mp4a) in the transcoded video"
    );
}
