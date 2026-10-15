//! 端到端：Apple Live Photo（HEIC + MOV）→ UltraHDR 动态照片 JPEG。
//!
//! 断言直接读输出文件的字节，不依赖 exiftool —— 这样 CI 里无需额外工具，
//! 且验证的是产物本身而非某个工具的解读。

use std::path::PathBuf;

/// 在字节流里找子串首次出现的下标。
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// 解析 GCamera XMP 的 `MicroVideoOffset` —— 追加在文件末尾的视频字节数。
///
/// XMP 是 exiftool 序列化的 XML，这里只做宽松扫描：定位到标签后跳过空白取数字，
/// 不假设元素是否被换行分割（不同版本的 exiftool 排版可能不同）。
fn micro_video_offset(jpeg: &[u8]) -> Option<usize> {
    const KEY: &[u8] = b"MicroVideoOffset>";
    let start = find(jpeg, KEY)? + KEY.len();
    let digits: Vec<u8> = jpeg[start..]
        .iter()
        .copied()
        .skip_while(|b| b.is_ascii_whitespace())
        .take_while(|b| b.is_ascii_digit())
        .collect();
    std::str::from_utf8(&digits).ok()?.parse().ok()
}

#[test]
fn live_photo_becomes_ultrahdr_motion_photo() {
    let output = PathBuf::from("./testoutput/MVIMG_3853.jpg");
    std::fs::remove_file(&output).ok();
    std::fs::create_dir_all("./testoutput").unwrap();

    heic2ultrahdr::i2a::ConvertRequest {
        image_path: "./tests/IMG_3853.HEIC".into(),
        video_path: Some("./tests/IMG_3853.MOV".into()),
        output_path: output.clone(),
        exiftool_path: Some("exiftool".into()),
        overwrite_existing: false,
        image_quality: 85,
        gainmap_quality: 85,
    }
    .convert()
    .unwrap();

    let bytes = std::fs::read(&output).unwrap();

    // 1. 输出是 JPEG 容器
    assert!(bytes.starts_with(b"\xFF\xD8"), "output is not a JPEG");

    // 2. 基础图带 UltraHDR 增益图：libultrahdr 写入的 ISO/IEC 21496-1 元数据段
    assert!(
        find(&bytes, b"urn:iso:std:iso:ts:21496:-1").is_some(),
        "missing ISO 21496-1 gain map metadata"
    );

    // 3. 动态照片标记（Android GCamera XMP 语义）。exiftool 序列化成元素而非属性，
    //    故匹配 `<GCamera:MicroVideo>1<`；该串不会误命中 MicroVideoVersion/Offset。
    assert!(find(&bytes, b"MicroVideo>1<").is_some(), "missing GCamera MicroVideo flag");

    // 4. MicroVideoOffset 指向的位置确实是一个 MP4 容器 ——
    //    播放器从 (文件大小 - MicroVideoOffset) 处开始读视频。
    let offset = micro_video_offset(&bytes).expect("unparsable MicroVideoOffset");
    assert!(offset < bytes.len(), "MicroVideoOffset points past the end of the file");
    let video = &bytes[bytes.len() - offset..];
    assert!(video.len() > 100_000, "appended video suspiciously small: {} bytes", video.len());
    // MP4 box 结构为 [4 字节长度][4 字节类型]
    assert_eq!(&video[4..8], b"ftyp", "appended video does not start with an MP4 ftyp box");

    // 5. 输出确实比源图大，多出来的部分就是追加的视频
    let src_image_len = std::fs::metadata("./tests/IMG_3853.HEIC").unwrap().len() as usize;
    assert!(bytes.len() > src_image_len, "output should carry the appended video");
}
