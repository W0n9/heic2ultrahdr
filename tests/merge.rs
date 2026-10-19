//! 端到端：Apple Live Photo（HEIC + MOV）→ UltraHDR 动态照片 JPEG。
//!
//! 断言直接读输出文件的字节，不依赖 exiftool —— 这样 CI 里无需额外工具，
//! 且验证的是产物本身而非某个工具的解读。

use std::path::PathBuf;

/// 在字节流里找子串首次出现的下标。
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// 从 GCamera XMP 中解析属性形式的整数标签，如 `Item:Length="4078970"`。
fn xmp_attr(jpeg: &[u8], key: &str) -> Option<usize> {
    let mut pat = key.as_bytes().to_vec();
    pat.extend_from_slice(b"=\"");
    let start = find(jpeg, &pat)? + pat.len();
    let digits: Vec<u8> = jpeg[start..].iter().copied().take_while(|b| b.is_ascii_digit()).collect();
    std::str::from_utf8(&digits).ok()?.parse().ok()
}

/// 遍历 MP4 顶层 box，断言 moov 先于 mdat（faststart）。
/// 微信等读取方对封装形态敏感，moov 后置的视频会被判为无法预览。
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
            0 => break, // size=0 表示延伸到文件尾
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

    // 3. 动态照片标记：现行 Motion Photo 1.0 规范形态。微信等按规范实现的
    //    读取方只认这套；旧版 MicroVideo 约定已从规范删除，本工具不写。
    assert!(find(&bytes, br#"MotionPhoto="1""#).is_some(), "missing GCamera MotionPhoto flag");
    assert!(find(&bytes, b"MicroVideo").is_none(), "legacy MicroVideo tags must not be written");
    // Container:Directory 里的视频项：规范据此定位文件尾部的视频
    assert!(
        find(&bytes, br#"Item:Semantic="MotionPhoto""#).is_some(),
        "missing Container:Directory MotionPhoto item"
    );

    // 4. Container Item:Length 记录追加视频的字节数，
    //    播放器从 `文件大小 - Item:Length` 处开始读视频
    let length = xmp_attr(&bytes, "Item:Length").expect("unparsable Container Item:Length");
    assert!(length < bytes.len(), "Container Item:Length points past the end of the file");
    let video = &bytes[bytes.len() - length..];
    assert!(video.len() > 100_000, "appended video suspiciously small: {} bytes", video.len());
    // MP4 box 结构为 [4 字节长度][4 字节类型]
    assert_eq!(&video[4..8], b"ftyp", "appended video does not start with an MP4 ftyp box");
    assert_eq!(&video[8..12], b"mp42", "appended video is not branded mp42");

    // 5. 视频封装为 faststart（moov 前置）
    assert!(moov_before_mdat(video), "embedded video is not faststart (moov after mdat)");

    // 6. 输出确实比源图大，多出来的部分就是追加的视频
    let src_image_len = std::fs::metadata("./tests/IMG_3853.HEIC").unwrap().len() as usize;
    assert!(bytes.len() > src_image_len, "output should carry the appended video");
}
