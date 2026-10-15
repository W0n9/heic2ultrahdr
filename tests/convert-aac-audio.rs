//! 端到端：把 Apple Live Photo 视频里的音频转码成 AAC。
//!
//! Apple 用 pcm_s16le 编码音频，多数 Android 播放器不支持，因此动态照片
//! 追加视频前需要转码。断言验证产物确实换成了 AAC。

use std::path::Path;

/// 在字节流里找子串首次出现的下标。
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
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

    // AAC 在 MP4 里的 sample entry 类型是 mp4a；源视频是 pcm_s16le，不含它，
    // 所以这个断言同时证明了转码确实发生了。
    assert!(find(&bytes, b"mp4a").is_some(), "no AAC sample entry (mp4a) in the transcoded video");
}
