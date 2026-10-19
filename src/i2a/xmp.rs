//! 动态照片 XMP 的生成与写入。
//!
//! 按现行 Android Motion Photo 1.0 规范生成：`GCamera:MotionPhoto` 标记加
//! `Container:Directory` 目录定位文件尾部的视频。旧版 `GCamera:MicroVideo*`
//! 属性已从规范中删除（规范要求读取方忽略），按项目决策不写——微信、Google
//! Photos 等规范读取方只认现行形态，只认 MicroVideo 的旧版相册属刻意放弃。
//! 只写 MicroVideo 的输出会被微信判为「无法预览此实况照片」而降级为静态图片。
//!
//! exiftool 12.76 对 `MotionPhoto` 与 `Container:Directory` 只读不写
//! （写入报 `Tag 'XMP-GCamera:MotionPhoto' is not defined`），因此 XMP 包
//! 由本模块直接生成，对 JPEG 做段级写入。

use anyhow::{bail, Context, Result};
use std::path::Path;

/// JPEG APP1 段标识 XMP 的固定前缀。
const XMP_APP1_ID: &[u8] = b"http://ns.adobe.com/xap/1.0/\x00";

/// 封面帧在视频中的位置。Apple 实况照片的 key photo 约定在视频第 1.5 秒。
const PRESENTATION_TIMESTAMP_US: u64 = 1_500_000;

/// 生成动态照片 XMP 包。`video_size` 是追加在文件末尾的视频字节数，
/// 即 `Container:Directory` 视频项的 `Item:Length`。
pub(crate) fn motion_photo_packet(video_size: u64) -> Vec<u8> {
    format!(
        r##"<x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="heic2ultrahdr">
  <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
    <rdf:Description rdf:about=""
        xmlns:GCamera="http://ns.google.com/photos/1.0/camera/"
        xmlns:Container="http://ns.google.com/photos/1.0/container/"
        xmlns:Item="http://ns.google.com/photos/1.0/container/item/"
      GCamera:MotionPhoto="1"
      GCamera:MotionPhotoVersion="1"
      GCamera:MotionPhotoPresentationTimestampUs="{PRESENTATION_TIMESTAMP_US}">
      <Container:Directory>
        <rdf:Seq>
          <rdf:li rdf:parseType="Resource">
            <Container:Item
              Item:Mime="image/jpeg"
              Item:Semantic="Primary"/>
          </rdf:li>
          <rdf:li rdf:parseType="Resource">
            <Container:Item
              Item:Mime="video/mp4"
              Item:Semantic="MotionPhoto"
              Item:Length="{video_size}"
              Item:Padding="0"/>
          </rdf:li>
        </rdf:Seq>
      </Container:Directory>
    </rdf:Description>
  </rdf:RDF>
</x:xmpmeta>
"##
    )
    .into_bytes()
}

/// 把动态照片 XMP 写入输出 JPEG。在视频追加**之前**调用：此时输出还是纯图片，
/// 读写量小；两个偏移字段都以文件尾为基准、等于视频字节数，视频追加后自然成立。
pub(crate) fn write_motion_photo_xmp(path: &Path, video_size: u64) -> Result<()> {
    let jpeg = std::fs::read(path).context("read output for XMP write failed")?;
    let out = replace_or_insert_xmp(&jpeg, &motion_photo_packet(video_size))?;
    std::fs::write(path, out).context("write XMP segment failed")?;
    Ok(())
}

/// 宽松识别动态照片标记：读取宽容，新旧两代 XMP 的属性与元素两种序列化形态
/// 都认（写入则从严，只写现行规范形态）。用于判断输出是否已是动态照片
/// （避免重复追加视频），输入可能来自旧版工具或旧版相册。
pub(crate) fn is_motion_photo(jpeg: &[u8]) -> bool {
    const PATTERNS: [&[u8]; 4] = [b"MicroVideo=\"1\"", b"MicroVideo>1<", b"MotionPhoto=\"1\"", b"MotionPhoto>1<"];
    PATTERNS.iter().any(|p| find(jpeg, p).is_some())
}

/// 在字节流里找子串首次出现的下标。
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// 构造一个带长度的 JPEG 段（marker + length + payload）。
fn segment(marker: u8, payload: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(payload.len() + 4);
    v.extend_from_slice(&[0xff, marker]);
    v.extend_from_slice(&(payload.len() as u16 + 2).to_be_bytes());
    v.extend_from_slice(payload);
    v
}

/// 把 XMP 包写入 JPEG 字节流：替换既有的 XMP APP1 段；没有则插入到起始的
/// APP0/APP1 段（JFIF、Exif）之后。SOS 之后的扫描数据与文件尾部（含追加的
/// 视频）原样保留。
///
/// MPF 段无需联动修正：MP Entry 偏移以 MPF 段内 TIFF 头为基准，XMP 段大小
/// 变化时段与增益图同步平移（实测验证）。
pub(crate) fn replace_or_insert_xmp(jpeg: &[u8], packet: &[u8]) -> Result<Vec<u8>> {
    if jpeg.len() < 4 || jpeg[..2] != [0xff, 0xd8] {
        bail!("not a JPEG file");
    }
    let mut payload = Vec::with_capacity(XMP_APP1_ID.len() + packet.len());
    payload.extend_from_slice(XMP_APP1_ID);
    payload.extend_from_slice(packet);
    let new_seg = segment(0xe1, &payload);

    let mut out = Vec::with_capacity(jpeg.len() + new_seg.len());
    out.extend_from_slice(&jpeg[..2]);
    let mut i = 2;
    // 没有可替换的 XMP 段时的插入点：起始连续 APP0/APP1 段的末尾（out 坐标）。
    let mut insert_at: Option<usize> = None;
    let mut replaced = false;

    loop {
        if jpeg.get(i) != Some(&0xff) {
            bail!("corrupt JPEG: expected marker at offset {i:#x}");
        }
        let marker = *jpeg.get(i + 1).context("truncated JPEG: dangling 0xFF")?;
        match marker {
            // SOS：段遍历到此为止，其余字节（扫描数据、EOI、追加的视频）原样拷贝
            0xda => {
                out.extend_from_slice(&jpeg[i..]);
                break;
            }
            // 独立 marker（TEM、RSTn、DRI 之外的 0xd0-0xd9 等）：无长度，原样拷贝
            0x01 | 0xd0..=0xd9 => {
                out.extend_from_slice(&jpeg[i..i + 2]);
                i += 2;
            }
            _ => {
                let hi = *jpeg.get(i + 2).context("truncated JPEG: no segment length")?;
                let lo = *jpeg.get(i + 3).context("truncated JPEG: no segment length")?;
                let len = u16::from_be_bytes([hi, lo]) as usize;
                if len < 2 {
                    bail!("corrupt JPEG: bad segment length at offset {i:#x}");
                }
                let end = i + 2 + len;
                let seg_payload = jpeg.get(i + 4..end).context("truncated JPEG: segment overruns file")?;
                if !replaced && marker == 0xe1 && seg_payload.starts_with(XMP_APP1_ID) {
                    out.extend_from_slice(&new_seg);
                    replaced = true;
                } else {
                    out.extend_from_slice(&jpeg[i..end]);
                    if !replaced && matches!(marker, 0xe0 | 0xe1) {
                        insert_at = Some(out.len());
                    }
                }
                i = end;
            }
        }
    }

    if !replaced {
        // 此时代码路径上 out 与 jpeg 前缀一致，insert_at 坐标有效
        out.splice(insert_at.unwrap_or(2)..insert_at.unwrap_or(2), new_seg);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 拼一个最小 JPEG：SOI + 给定段 + SOS + 假扫描数据 + EOI。
    fn fake_jpeg(segments: &[Vec<u8>]) -> Vec<u8> {
        let mut v = vec![0xff, 0xd8];
        for s in segments {
            v.extend_from_slice(s);
        }
        v.extend_from_slice(&[0xff, 0xda, 0x00, 0x0c]); // SOS 头
        v.extend_from_slice(&[0xaa, 0xbb, 0xcc]); // 扫描数据
        v.extend_from_slice(&[0xff, 0xd9]); // EOI
        v
    }

    #[test]
    fn inserts_after_leading_app0_app1() {
        let app0 = segment(0xe0, b"JFIF\x00");
        let exif = segment(0xe1, b"Exif\x00\x00tiff");
        let jpeg = fake_jpeg(&[app0, exif]);

        let out = replace_or_insert_xmp(&jpeg, b"<xmp/>").unwrap();

        // 段序：APP0、Exif APP1、XMP APP1、SOS
        assert_eq!(&out[2..4], &[0xff, 0xe0]);
        let app0_end = 2 + 2 + 2 + 5; // marker + len 字段 + 5 字节 payload
        assert_eq!(&out[app0_end..app0_end + 2], &[0xff, 0xe1]);
        let exif_end = app0_end + 2 + 2 + 10;
        assert_eq!(&out[exif_end..exif_end + 2], &[0xff, 0xe1]);
        let xmp_len = u16::from_be_bytes([out[exif_end + 2], out[exif_end + 3]]) as usize;
        let xmp_payload_end = exif_end + 4 + xmp_len - 2;
        assert!(out[exif_end + 4..xmp_payload_end].starts_with(XMP_APP1_ID));
        assert_eq!(&out[xmp_payload_end..xmp_payload_end + 2], &[0xff, 0xda]);
    }

    #[test]
    fn replaces_existing_xmp_in_place() {
        let old_xmp = segment(0xe1, &[XMP_APP1_ID.as_ref(), b"<old/>"].concat());
        let jpeg = fake_jpeg(&[old_xmp]);

        let out = replace_or_insert_xmp(&jpeg, b"<new/>").unwrap();

        assert!(find(&out, b"<new/>").is_some());
        assert!(find(&out, b"<old/>").is_none());
        // 尾部完整保留
        assert!(out.ends_with(&[0xff, 0xd9]));
    }

    #[test]
    fn idempotent() {
        let jpeg = fake_jpeg(&[]);
        let packet = motion_photo_packet(4079502);
        let once = replace_or_insert_xmp(&jpeg, &packet).unwrap();
        let twice = replace_or_insert_xmp(&once, &packet).unwrap();
        assert_eq!(once, twice);
    }

    #[test]
    fn detects_both_generations_of_motion_marks() {
        assert!(is_motion_photo(b"...GCamera:MicroVideo>1</GCamera:MicroVideo..."));
        assert!(is_motion_photo(br#"...GCamera:MotionPhoto="1"..."#));
        assert!(!is_motion_photo(b"plain jpeg"));
        // MicroVideoVersion / MotionPhotoVersion 不应误命中
        assert!(!is_motion_photo(br#"...GCamera:MicroVideoVersion="1"..."#));
        assert!(!is_motion_photo(br#"...GCamera:MotionPhotoVersion="1"..."#));
    }

    #[test]
    fn rejects_non_jpeg() {
        assert!(replace_or_insert_xmp(b"\x89PNG", b"x").is_err());
    }
}
