# HDR gain map 格式调研（Apple HEIC 侧 vs Android UltraHDR / ISO 21496-1）

## 本文件用途

本文件是技术报告 **Background** 与 **Related Work** 两章的素材库：为「把 Apple HEIC（含 HDR gain map）转换为 Android UltraHDR JPEG」这一课题，整理两侧格式的定义、数学、字段语义、标准状态，以及已有的开源实现与学术工作。

内容按 A（Apple 侧）、B（Android UltraHDR / ISO 21496-1）、C（Adobe hdrgm 命名空间）、D（相关工作）分节；每节末尾附该节的引用清单（编号形如 `[A-1]`）。

### 证据标注约定

每条断言后标注证据等级，请勿混用：

| 标注 | 含义 |
| --- | --- |
| 【规范】 | 标准文本、官方规范或第一方开发者文档中明确写出的内容 |
| 【实现】 | 第一方源码、库代码、真实文件中的可观察事实（含本仓库代码与实测） |
| 【二手】 | 只能追到第三方转述/未读到原文的来源，已显式标注 |
| 【推断】 | 我基于上述事实的推断，非任何来源的直接陈述 |

其他约定：

- 引文中出现的乘法符号（`*`）在网页抓取过程中常被吞掉（例如 Apple 文档里的 `-20.0 * maker48 + 1.8` 抓成 `-20.0  maker48 + 1.8`）。本文件按上下文补回 `*`，并在此声明该处理。
- 引文一律保留英文原文；行内解释用中文。
- 仓库内文件引用使用仓库相对路径（如 `src/i2a/convert.rs:119`）。

---

## A. Apple 的 HDR gain map（HEIC 侧）

### A.1 辅助图类型 URN 的官方定义

Apple 的公开定义只存在于一篇第一方开发者文档：**“Applying Apple HDR effect to your photos”**（Apple Developer Documentation，AppKit 章节）。

【规范】该文档 “Extract the existing HDR gain map from the image” 一节原文：

> Get the existing HDR gain map from the image's auxiliary data using the `urn:com:apple:photo:2020:aux:hdrgainmap` image data type. The gain map is untagged and formatted as linear data. It's encoded using the Rec.709 transfer function and is 1/4 the resolution of the original image.

同一文档的 Overview 给出该图的性质：

> You can apply Apple high dynamic range (HDR) effect to digital photography images without using Apple SDKs. You compute the Apple HDR gain map from the image's auxiliary data and then apply it to the image. The *Apple HDR gain map* is an 8-bit, single-channel luminance map that's stored with an image.

【规范】对应的 Apple SDK 侧 API 常量（Image I/O）：

- `kCGImageAuxiliaryDataTypeHDRGainMap` — “The type for High Dynamic Range (HDR) gain map information.” 可用性：iOS 14.1+ / iPadOS 14.1+ / Mac Catalyst 14.1+ / macOS 11.0+ / tvOS 14.0+ / visionOS 1.0+ / watchOS 7.0+。类型 `let kCGImageAuxiliaryDataTypeHDRGainMap: CFString`。
- `kCGImageAuxiliaryDataTypeISOGainMap` — 可用性：iOS 18.0+ / iPadOS 18.0+ / Mac Catalyst 18.0+ / macOS 15.0+ / tvOS 18.0+ / visionOS 2.0+ / watchOS 11.0+。类型 `let kCGImageAuxiliaryDataTypeISOGainMap: CFString`。

【推断】`kCGImageAuxiliaryDataTypeISOGainMap` 的存在说明 Apple 自 iOS 18 / macOS 15 起在 SDK 层面区分「Apple 自有 HDR gain map」与「ISO 21496-1 gain map」两种辅助图类型；Apple 文档没有解释这两者关系，故仅作为线索记录。

**URN 的注册状态。** 【实现】IANA 的 *Uniform Resource Names (URN) Namespaces* 注册表中：

- 不存在命名空间标识 `com`（正式表与不正式表都没有）；
- 存在 `iso`（正式命名空间，Reference: RFC 5141）；
- 没有任何 Apple 相关的命名空间条目。

该注册表页面自身标注 “Last Updated: 2026-10-12”。【推断】因此 `urn:com:apple:photo:2020:aux:hdrgainmap` 使用的是一个**未在 IANA 注册的** URN 命名空间，其语义完全由 Apple 的文档与实现定义，不存在标准组织层面的注册记录。

**在 HEIC 里的承载方式。** 【实现】libheif 的公共头 `libheif/api/libheif/heif_aux_images.h` 提供辅助图访问 API：`heif_image_handle_get_number_of_auxiliary_images()`（“List the number of auxiliary images assigned to this image handle.”）、`heif_image_handle_get_auxiliary_type()`（返回类型字符串，调用方需用 `heif_image_handle_release_auxiliary_type()` 释放）、`heif_image_handle_get_auxiliary_image_handle()`（“Get the image handle of an auxiliary image.”）。该头文件本身没有解释类型字符串的 URN 语义（无 “auxC” / URN 字样），URN 的语义来源仍是 Apple 文档。

【实现】本仓库正是通过这条路径取图：`src/i2a/convert.rs:70-89` 遍历 `handle.auxiliary_images(...)`，比较 `aux_handle.auxiliary_type()?` 是否等于 `"urn:com:apple:photo:2020:aux:hdrgainmap"`。

### A.2 gain map 的编码方式（位深、传递函数、分辨率）

【规范】Apple 文档原文（见 A.1 引文）给出三条硬性描述：**8-bit**、**single-channel**、**untagged**、**Rec.709 transfer function**、**1/4 resolution**。

【规范】应用 gain map 时 Apple 给出的三步（“Apply the HDR gain map to the image” 一节原文）：

> 1. Resize the gain map by scaling it so it matches the size of the original image.
> 2. Linearize the gain map by inverting the gain map gamma using the Rec.709 transfer function.
> 3. Linearize the SDR RGB image by inverting the gamma of the image according to the colorspace.

【规范】然后：

> let hdr_rgb = sdr_rgb * (1.0 + (headroom - 1.0) * gainmap)

> The result of applying the gain map is a linear HDR image with a scale where 1.0 is the reference white and the peak value is no greater than the headroom value.

【实现】本仓库 `src/i2a/convert.rs:107-108` 与 `146-152` 对 gain map 做的是 **sRGB EOTF 的反变换**（`reverse_srgb_transform`：`u <= 0.04045 ? u/12.92 : ((u+0.055)/1.055)^2.4`），而 Apple 文档写的是 “Rec.709 transfer function”。两者不是同一个函数（分段点与指数都不同）。此处只记录「文档说法」与「实现做法」的差异，不断言哪个更正确；若要写进报告，建议表述为「实现采用了 sRGB 传递函数近似」。

【规范】Apple 文档同页还给出应用公式的语义边界：结果线性 HDR 图的峰值不超过 headroom。

### A.3 headroom 语义：`HDRGainMapHeadroom` / `MakerNotes:HDRHeadroom`(33) / `HDRGain`(48)

**headroom 的定义。**【规范】Apple 文档原文：

> The headroom is the ratio of the luminance of the image's brightest white to the luminance of standard dynamic range (SDR) white, in the image's native color space.

**前置校验。**【规范】

> Before you begin, check whether the image's metadata contains the `HDRGainMapVersion` key. If this key exists in the metadata, the gain map is valid and you can continue calculating the headroom.

> Next, extract the `MakerNote` dictionary from the image's metadata. If the `MakerNote` dictionary contains the `33` and `48` keys and they're set to 32-bit float values, the HDR gain map is valid and you can use it to calculate the headroom.

**从 MakerNote 33/48 计算 stops 的官方公式。**【规范】Apple 文档中的 Swift 代码（乘号按约定补回）：

```swift
let (maker33, maker48) = getMetadataFromImage()
let stops: Float
if maker33 < 1.0 {
    if maker48 <= 0.01 {
        stops = -20.0 * maker48 + 1.8
    } else {
        stops = -0.101 * maker48 + 1.601
    }
} else {
    if maker48 <= 0.01 {
        stops = -70.0 * maker48 + 3.0
    } else {
        stops = -0.303 * maker48 + 2.303
    }
}
let headroom = pow(2.0, max(stops, 0.0))
```

【实现】本仓库 `src/i2a/convert.rs:44-66` 逐字复刻了该公式（含 `stops.max(0.0)` 与 `2.0_f32.powf(...)`）。

**两个 MakerNote 标记的第三方逆向记录。**【二手】exiftool 的 `lib/Image/ExifTool/Apple.pm` 把这两个 tag 记为：

```perl
    0x0021 => { # (MeteorHeadroom, ref 2)
        Name => 'HDRHeadroom',
        Writable => 'rational64s',
    },
...
    0x0030 => { # (MeteorPlusGainMap, ref 2)
        Name => 'HDRGain',
        Writable => 'rational64s',
    },
```

其中注释里的 `ref 2` 指该模块的第二个来源，模块头部描述为 “Frank Rupprecht private communication”，另一来源是 `http://www.photoinvestigator.co/blog/the-mystery-of-maker-apple-metadata/`。**这是逆向工程结果，不是 Apple 的规范**；Apple 文档只用编号 33/48 指代，从未给出 tag 名称或数值类型以外的语义。

**`HDRGainMapHeadroom` 与 Apple 的 XMP 命名空间。**【实现】libavif 源码 `apps/shared/avifjpeg.c` 顶部常量原文：

```c
#define XML_NAME_SPACE_GAIN_MAP "http://ns.adobe.com/hdr-gain-map/1.0/"
#define XML_NAME_SPACE_APPLE_GAIN_MAP "http://ns.apple.com/HDRGainMap/1.0/"
#define XML_NAME_SPACE_RDF "http://www.w3.org/1999/02/22-rdf-syntax-ns#"
#define XML_NAME_SPACE_XMP_NOTE "http://ns.adobe.com/xmp/note/"
```

同文件 `avifJPEGParseGainMapXMPPropertiesAppleFormat()` 从 Apple 命名空间读取 `HDRGainMapHeadroom`（线性 headroom，`hdrHeadroomLinear`），拒绝非正值，然后 `log2(hdrHeadroomLinear)` 转成对数域并写成 ISO 21496-1 的分数形式；`avifJPEGFindAppleGainMapXMPNode()` 则通过 Apple 命名空间下的 `HDRGainMapVersion` 子元素来识别 Apple gain map 的 XMP 节点。

【二手】exiftool 的 XMP 标签文档把 `XMP HDRGainMap` 归为 “Used in Apple HDR GainMap images.”，当前只列出 `HDRGainMapVersion`（string）；同一页面还列出 Apple 的 `XMP apdi` 标签族（`AuxiliaryImageType` / `NativeFormat` / `StoredFormat`），同样标注 “Used in Apple HDR GainMap images.”。

【推断】`HDRGainMapHeadroom` 是真实文件里存在、且被 libavif 解析的键，但**不在 Apple 的公开开发者文档中**（Apple 那篇文档只提到 `HDRGainMapVersion`）。报告若要引用 `HDRGainMapHeadroom`，出处只能落在 libavif 源码 / exiftool 这一类实现侧来源上。

【实现】本仓库的读取优先级：先 `xmp:HDRGainMapHeadroom`（直接当线性 headroom），缺失时回落到 `MakerNotes:HDRHeadroom` + `MakerNotes:HDRGain` 的官方公式，见 `src/i2a/convert.rs:28-67`；读取的四个 exif 键在 `src/i2a/convert.rs:216-219` 列出。

### A.4 Apple 与 ISO 21496-1 的关系

【规范】Apple 文档末尾的 Note 原文：

> The technique described in this document has been proposed for standardization at the International Organization for Standardization (ISO) as part of ISO/NP 21496-1. Apple is prepared to commit to license essential patents in this technique with respect to a standard resulting from ISO/NP 21496-1 in accordance with the Common Patent Policy for ITU-T/ITU-R/ISO/IEC (as of April 2023).

【实现】ISO 项目时间线（见 B.4）显示 “New project approved 2023-07-06”，与 Apple 该注记的 “as of April 2023” 在时间上衔接。

### A 节引用清单

- [A-1] Apple Developer Documentation, *Applying Apple HDR effect to your photos* — https://developer.apple.com/documentation/appkit/applying-apple-hdr-effect-to-your-photos （Overview / “Extract the existing HDR gain map from the image” / “Calculate the headroom from the image metadata” / “Apply the HDR gain map to the image” 各节，含末尾 ISO/NP 21496-1 Note）
- [A-2] Apple Developer Documentation, `kCGImageAuxiliaryDataTypeHDRGainMap` — https://developer.apple.com/documentation/imageio/kcgimageauxiliarydatatypehdrgainmap
- [A-3] Apple Developer Documentation, `kCGImageAuxiliaryDataTypeISOGainMap` — https://developer.apple.com/documentation/imageio/kcgimageauxiliarydatatypeisogainmap
- [A-4] IANA, *Uniform Resource Names (URN) Namespaces* — https://www.iana.org/assignments/urn-namespaces/urn-namespaces.xhtml （registry Last Updated 2026-10-12；无 `com`，有 `iso`/RFC 5141）
- [A-5] libheif, `libheif/api/libheif/heif_aux_images.h` — https://github.com/strukturag/libheif/blob/master/libheif/api/libheif/heif_aux_images.h
- [A-6] exiftool, `lib/Image/ExifTool/Apple.pm` — https://github.com/exiftool/exiftool/blob/master/lib/Image/ExifTool/Apple.pm （0x0021 / 0x0030 定义与文件头来源说明）
- [A-7] libavif, `apps/shared/avifjpeg.c` — https://github.com/AOMediaCodec/libavif/blob/main/apps/shared/avifjpeg.c （命名空间常量、Apple 格式解析函数）
- [A-8] exiftool, *XMP Tags* 文档 — https://exiftool.org/TagNames/XMP.html （XMP HDRGainMap / XMP apdi / XMP hdrgm 三个标签族）
- [A-9] 本仓库代码：`src/i2a/convert.rs`（headroom 读取与换算、gain map 反变换）

---

## B. Android UltraHDR / ISO 21496-1（JPEG 侧）

### B.1 格式文档与文件结构

【规范】规范文本是 Android 官方的 **“Ultra HDR Image Format v1.1”** 页面（`developer.android.com/media/platform/hdr-image-format`）。页面自身的 “Change Log” 一节原文：

> v1.1 — All changes in this version of the Ultra HDR specification are informational and about ISO 21496-1 compatibility. There are no changes to the actual file format.
> v1.0 — The initial Ultra HDR specification publication.

页面页脚另有两处关键信息：`Last updated 2024-10-25 UTC`，以及 “This product includes Gain Map technology under license by Adobe.”

【规范】文件组织（“Gain map container” 一节）：主图为 JPEG，gain map 为 secondary image，通过 **MPF（CIPA DC-x 007-2009 Multi-Picture Format）** 存储；主图与 gain map 的 XMP 用 **GContainer** 描述：

- `Item:Semantic` 取值 `Primary`（“Indicates that the media item is the primary image, ready for display, in the container. The directory must contain one "Primary" item.”）与 `GainMap`（“Indicates that the media item is a gain map. The directory might contain at most one "GainMap" item.”）；
- `Item:Mime` 取值 `image/jpeg`。

【规范】格式信号（“Decode / Signal of the format” 一节）：

> A JPEG file conforming to this format may be identified by the presence of `hdrgm:Version="1.0"` in the primary image's XMP packet, where `hdrgm` is the namespace URI `http://ns.adobe.com/hdr-gain-map/1.0/`.

【规范】无效元数据的处理：“Metadata is considered invalid if a required field is not present, or if any field is present with an invalid value. … If invalid metadata is encountered, the gain map should be ignored and the SDR image should be displayed.”

【规范】gain map 的存储与采样约束（“Gain map generation” 末尾）：

- 必须是 8-bit 无符号整数、JPEG 编码的 secondary image；压缩质量建议 “85 to 90 out of 100”；
- 分辨率可与主图不同，缩放采样 “must be bilinear or better”；建议降到主图的 1/16 面积（每维 1/4）；
- 方向必须与主图一致，gain map 自身的 EXIF orientation 与 ICC profile 都不被使用；
- 单通道 gain map 优先（“Use a single-channel gain map whenever possible.”）。

### B.2 增益图的数学定义（Encode 侧）

以下公式均【规范】摘自 “Encode / Gain map generation” 一节（原文即为等宽代码块，逐字保留）：

```
pixel_gain(x, y) = (Yhdr(x, y) + offset_hdr) / (Ysdr(x, y) + offset_sdr)

map_min_log2 = log2(min_content_boost)
map_max_log2 = log2(max_content_boost)

log_recovery(x, y) = (log2(pixel_gain(x, y)) - map_min_log2)
                   / (map_max_log2 - map_min_log2)
clamped_recovery(x, y) = clamp(log_recovery(x, y), 0.0, 1.0)
recovery(x, y) = pow(clamped_recovery(x, y), map_gamma)

encoded_recovery(x, y) = floor(recovery(x, y) * 255.0 + 0.5)
```

配套的取值范围与约定（同节原文）：

- `Ysdr(x, y)` 定义域 0.0–1.0，是 SDR 主图的线性亮度：`Ysdr(x, y) = primary_color_profile_to_luminance(SDR(x, y))`；
- `Yhdr(x, y)` 定义在 0.0 到 max content boost；
- “Max content boost must be greater than or equal to 1.0. Min content boost must be in the range (0.0, 1.0].”；
- `map_gamma` 必须 > 0.0，建议取 1.0；
- offset 建议值：“Use a value of 0.015625 (1/64) for `offset_sdr` and `offset_hdr`.”；
- `pixel_gain` 在 `Ysdr` 与 `offset_sdr` 同时为 0 时行为由实现定义（可定义为 1.0）；`recovery` 在 `pixel_gain = 0` 时建议定义为 0.0；
- “The gain map is stored in a secondary image JPEG, and therefore must be encoded using 8-bit, unsigned integer values, thus in the range [0, 255].”；
- “The color profile of the SDR image defines the color space of the HDR image.”（若 SDR 主图是 Display-P3，HDR 也按该 primaries 解释）。

**与本课题的直接关系。**【实现】本仓库的转换正是一次「重参数化」：令 `offset_sdr = offset_hdr = 0`、`min_content_boost = 1.0`、`max_content_boost = apple_headroom`、`map_gamma = 1.0`，于是

```
log_recovery = (log2(pixel_gain) - log2(1)) / (log2(max_content_boost) - log2(1))
             = log2(pixel_gain) / log2(apple_headroom)
             = ln(pixel_gain) / ln(apple_headroom)
             = ln(1 + (hr - 1) * gainmap_linear) / ln(apple_headroom)
```

见 `src/i2a/convert.rs:110-119` 的注释与实现（`log_hr = apple_headroom.ln()`、`hr_1 = apple_headroom - 1.0`、`log_recovery = (1.0 + hr_1 * apple_gainmap_linear).ln() / log_hr`）；最后的量化 `(recovery * 255.0 + 0.5).floor() as u8`（`convert.rs:122`）与规范中的 `encoded_recovery` 公式一致。写入 libultrahdr 的元数据为 `max_content_boost: [apple_headroom; 3]`、`hdr_capacity_max: apple_headroom`（`src/i2a/convert.rs:273-281`）。

【推断】用户问题中的假想公式 `log_recovery = log2(pixel_gain) / log2(headroom)` 正是上式在 `min_content_boost = 1` 时的特例；规范原文的一般形式是带 `map_min_log2` 的归一化形式（见上）。这一特例的推导在规范里没有单独写出，是本仓库代码注释中的推导。

### B.3 增益图的数学定义（Display 侧）

【规范】“Display / Use the gain map to create the adapted HDR rendition” 一节原文：

```
recovery(x, y) = encoded_recovery(x, y) / 255.0
log_recovery(x, y) = pow(recovery(x, y), 1.0 / map_gamma)

unclamped_weight_factor = (log2(max_display_boost) - hdr_capacity_min)
                            / (hdr_capacity_max - hdr_capacity_min)
weight_factor = clamp(unclamped_weight_factor, 0.0, 1.0)

log_boost(x, y) = gain_map_min * (1.0f - log_recovery(x, y))
                    + gain_map_max * log_recovery(x, y)
HDR(x, y) = (SDR(x, y) + offset_sdr) * exp2(log_boost(x, y) * weight_factor)
          - offset_hdr
```

其中（同节原文）：

- “Max display boost is a scalar floating point value defined as the ratio between the current HDR white point and divided by the current SDR white point. This value is provided by the display system and can change over time.”；
- `hdr_capacity_max` / `hdr_capacity_min` 来自 `hdrgm:HDRCapacityMax` / `hdrgm:HDRCapacityMin`；
- `BaseRenditionIsHDR` 为 “True” 时权重取补：`weight_factor = 1.0 - clamp(unclamped_weight_factor, 0.0, 1.0)`；
- 规范明确要求用指数形式应用：“The gain map must be applied as shown earlier (using exponentiation) to properly produce the needed relative tonal relationships … If the Gain Map is applied any other way, such as via simple linear interpolation, then the relative tonal relationships within the image are correct only at max content boost, and are compromised at other boost factors.”

【实现】libultrahdr v1.4.0 的 `lib/src/gainmapmath.cpp` 是该数学的 C++ 实现：

- 编码侧 `encodeGain()`：

```cpp
uint8_t encodeGain(float y_sdr, float y_hdr, uhdr_gainmap_metadata_ext_t metadata,
                   float log2MinContentBoost, float log2MaxContentBoost, int index) {
    float gain = 1.0f;
    if (y_sdr > 0.0f) {
        gain = y_hdr / y_sdr;
    }
    if (gain < metadata->min_content_boost[index]) gain = metadata->min_content_boost[index];
    if (gain > metadata->max_content_boost[index]) gain = metadata->max_content_boost[index];
    float gain_normalized =
        (log2(gain) - log2MinContentBoost) / (log2MaxContentBoost - log2MinContentBoost);
    float gain_normalized_gamma = powf(gain_normalized, metadata->gamma[index]);
    return static_cast<uint8_t>(gain_normalized_gamma * 255.0f);
}
```

（注意：这里没有 +0.5 的舍入，与 Android 规范写的 `floor(x*255+0.5)` 略有差异；截断而非四舍五入。）

- 解码侧 `applyGain()` 片段：

```cpp
    float logBoostR = log2(metadata->min_content_boost) * (1.0f - gain.r) + log2(metadata->max_content_boost) * gain.r;
    float gainFactorR = exp2(logBoostR);
    ...
    return {{{((e.r + metadata->offset_sdr) * gainFactorR) - metadata->offset_hdr,
              ((e.g + metadata->offset_sdr) * gainFactorG) - metadata->offset_hdr,
              ((e.b + metadata->offset_sdr) * gainFactorB) - metadata->offset_hdr}}};
```

以及 gamma 反变换：`if (metadata->gamma != 1.0f) gain.r = pow(gain.r, 1.0f / metadata->gamma);`（三个通道各一次）。

### B.4 `GainmapMetadata` 字段定义（语义、范围、默认值）

#### B.4.1 格式层字段（XMP `hdrgm`，Android 规范表）

【规范】Android 规范 “HDR Gain map metadata” 一节的字段表（原文逐条）：

| 名称 | 类型 | 语义（原文要点） | 必填/默认 | 范围 |
| --- | --- | --- | --- | --- |
| `hdrgm:Version` | Text | “The version of the gain map format in use. This version is "1.0".” | Required | — |
| `hdrgm:BaseRenditionIsHDR` | Boolean | “False” 表示主图是 SDR、gain map 可与主图合成 HDR；“True” 表示主图是 HDR、gain map 可合成 SDR。“Must be "False".” | Optional；默认 “False” | — |
| `hdrgm:GainMapMin` | Real 或 Real 数组 | 存 `map_min_log2`，即 `log2` of min content boost——“the minimum allowed ratio of the linear luminance for the target HDR rendition relative to (divided by) that of the SDR image, at a given pixel” | Optional；默认 0.0 | ≤ `GainMapMax` |
| `hdrgm:GainMapMax` | Real 或 Real 数组 | 存 `map_max_log2`，即 `log2` of max content boost——“the maximum allowed ratio of the linear luminance for the Target HDR rendition relative to (divided by) that of the SDR image, at a given pixel” | Required | ≥ `GainMapMin` |
| `hdrgm:Gamma` | Real 或 Real 数组 | 存 `map_gamma`，“the gamma to apply to the stored map values” | Optional；默认 1.0 | > 0.0 |
| `hdrgm:OffsetSDR` | Real 或 Real 数组 | 存 `offset_sdr`，“the offset to apply to the SDR pixel values during gain map generation and application” | Optional；默认 0.015625 (1/64) | ≥ 0.0 |
| `hdrgm:OffsetHDR` | Real 或 Real 数组 | 存 `offset_hdr`，“the offset to apply to the HDR pixel values during gain map generation and application” | Optional；默认 0.015625 (1/64) | ≥ 0.0 |
| `hdrgm:HDRCapacityMin` | Real | 存 `hdr_capacity_min`，“`log2` of the minimum display boost value for which the map is applied at all” | Optional；默认 0.0 | ≥ 0.0 |
| `hdrgm:HDRCapacityMax` | Real | 存 `hdr_capacity_max`，“`log2` of the maximum display boost value for which the map is applied completely” | Required | > `HDRCapacityMin` |

数组语义（原文）：“When an ordered array of Reals, it may contain one item which applies to all channels or three items for the Red, Green, and Blue channels respectively.”

规范的实现建议（Tip，原文）：`HDRCapacityMax` 取与 `GainMapMax` 相等，`HDRCapacityMin` 取 `max(GainMapMin, 0.0)`；且“whenever possible, use only a single Real value for each preceding property that permits one value or an array of values”。

规范给出的完整 XMP 示例（原文）：

```xml
<x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="XMP Core 5.5.0">
  <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
    <rdf:Description rdf:about=""
     xmlns:hdrgm="http://ns.adobe.com/hdr-gain-map/1.0/"
     hdrgm:Version="1.0"
     hdrgm:GainMapMin="-0.57609993"
     hdrgm:GainMapMax="4.7090998"
     hdrgm:Gamma="1"
     hdrgm:OffsetSDR="0.015625"
     hdrgm:OffsetHDR="0.015625"
     hdrgm:HDRCapacityMin="0"
     hdrgm:HDRCapacityMax="4.7090998"
     hdrgm:BaseRenditionIsHDR="False"/>
  </rdf:RDF>
</x:xmpmeta>
```

#### B.4.2 libultrahdr 的 C API 结构体

【实现】libultrahdr **v1.4.0**（本仓库 `scripts/linux-build-libuhdr.sh:10` 固定 `--branch v1.4.0`）的公共头 `ultrahdr_api.h` 原文（节选，注释逐字）：

```c
/**\brief Gain map metadata. */
typedef struct uhdr_gainmap_metadata {
    float max_content_boost; /**< Value to control how much brighter an image can get, when shown
                                  on an HDR display, relative to the SDR rendition. This is constant for
                                  a given image. Value MUST be in linear scale. */
    float min_content_boost; /**< Value to control how much darker an image can get, when shown on
                                  an HDR display, relative to the SDR rendition. This is constant for a
                                  given image. Value MUST be in linear scale. */
    float gamma;             /**< Encoding Gamma of the gainmap image. */
    float offset_sdr;        /**< The offset to apply to the SDR pixel values during gainmap generation
                                  and application. */
    float offset_hdr;        /**< The offset to apply to the HDR pixel values during gainmap generation
                                  and application. */
    float hdr_capacity_min;  /**< Minimum display boost value for which the map is applied completely.
                                  Value MUST be in linear scale. */
    float hdr_capacity_max;  /**< Maximum display boost value for which the map is applied completely.
                                  Value MUST be in linear scale. */
    int use_base_cg;         /**< Is gainmap application space same as base image color space */
} uhdr_gainmap_metadata_t;   /**< alias for struct uhdr_gainmap_metadata */
```

注意两点（【实现】，直接读自源码）：

1. v1.4.0 的公开结构体是**标量 float**，不是每通道数组；每通道版本是内部/扩展类型 `uhdr_gainmap_metadata_ext_t`（`lib/include/ultrahdr/ultrahdrcommon.h`，继承 `uhdr_gainmap_metadata`，把 `max_content_boost` 等扩为 `[3]` 数组并附加 `std::string version`；构造函数中有 `std::copy(metadata.max_content_boost, metadata.max_content_boost + 3, max_content_boost);` 这类按通道拷贝，以及 `are_all_channels_identical()` 判断）。
2. 字段名 `use_base_cg` 对应 Android/ISO 概念里的 “use base color space”；C 注释为 “Is gainmap application space same as base image color space”。

【实现】XMP 层的量纲转换在 `lib/src/jpegrutils.cpp`：写 XMP 时对 content boost 取 log2——原文行：

```cpp
writer.WriteAttributeNameAndValue(kMapGainMapMin, log2(metadata.min_content_boost));
writer.WriteAttributeNameAndValue(kMapGainMapMax, log2(metadata.max_content_boost));
writer.WriteAttributeNameAndValue(kMapGamma, metadata.gamma);
writer.WriteAttributeNameAndValue(kMapOffsetSdr, metadata.offset_sdr);
writer.WriteAttributeNameAndValue(kMapOffsetHdr, metadata.offset_hdr);
```

读 XMP 时反向填充（同一文件）：`getMaxContentBoost(...)` 缺失时直接报错 “xml parse error, could not find attribute …”，`getHdrCapacityMax(...)` 缺失同样报错；可选字段则回落默认值，例如 `metadata->min_content_boost = 1.0f;`（GainMapMin 缺失时）、`metadata->gamma = 1.0f;`（Gamma 缺失时）。

【推断】由于 C API 注释明确 “Value MUST be in linear scale”，而 XMP/ISO 存的是 log2 域，libultrahdr 在序列化层做 `log2`/`exp2` 转换；`hdr_capacity_min/max` 的转换在同层（由 `getHdrCapacityMax` 这类 XMP handler 方法完成），但我未逐行读到其实现，故不写成确定事实。

#### B.4.3 ISO 21496-1 的字段命名（第三方公开实现给出的对照）

ISO 21496-1 正文付费（见 B.5），但 FFmpeg 的公开头文件 `libavutil/gain_map.h` 用另一套命名描述了同一份元数据，可作为字段语义的公开对照。原文：

```c
typedef struct AVGainMapParams {
    int version;
    int use_base_color_space;
    AVRational base_hdr_headroom;
    AVRational alternate_hdr_headroom;
    struct AVGainMapChannel {
        AVRational gain_map_min;
        AVRational gain_map_max;
        AVRational gamma;
        AVRational base_offset;
        AVRational alternate_offset;
    } channels; /* R, G, B */
    int nb_channels;
} AVGainMapParams;
```

对应注释原文：

- `version`：“The version of this gain map struct. Reserved to allow future extensions of the struct. Must be <= AV_ISO21496_VERSION.”
- `use_base_color_space`：“If non-zero, the gain map is applied in the colour space of the base rendition; otherwise in that of the alternate rendition.”
- `base_hdr_headroom` / `alternate_hdr_headroom`：“log2 of the display headroom at which the base rendition is shown unmodified, and at which the map is applied in full.”
- `gain_map_min` / `gain_map_max`：“log2-domain minimum and maximum gain applied by the map, i.e. the values the map's 0.0 and 1.0 endpoints decode to.”
- `gamma`：“Encoding gamma of the stored map values. Must be > 0.”
- `base_offset` / `alternate_offset`：“Small constants added before the log-domain math to avoid a singularity at zero.”
- `nb_channels`：“Must be 1 or 3. If 1, all channels share the same set of parameters.”
- 整个结构体的说明：“Parameters describing how to combine a gain map rendition with its base rendition to recover the alternate rendition, as defined by ISO 21496-1:2025 "Digital photography - Gain map metadata for image conversion".”

【推断】这套命名与 Adobe/Android 的命名是同一份元数据的两种表述：`base_hdr_headroom` ≈ `HDRCapacityMin`、`alternate_hdr_headroom` ≈ `HDRCapacityMax`、`base_offset`/`alternate_offset` ≈ `OffsetSDR`/`OffsetHDR`、`use_base_color_space` ≈ “use base color space” 标志位、`gain_map_min/max` ≈ `GainMapMin/Max`（均为 log2 域）。FFmpeg 头文件本身没有给出这组对应关系，故标为推断。

#### B.4.4 Android Java API 的字段对应

【规范】`android.graphics.Gainmap`（API reference）的方法与其一句话说明（原文）：

- `setRatioMin(float r, float g, float b)` — “Sets the gainmap ratio min.”
- `setRatioMax(float r, float g, float b)` — “Sets the gainmap ratio max.”
- `setGamma(float r, float g, float b)` — “Sets the gainmap gamma.”
- `setEpsilonSdr(float r, float g, float b)` — “Sets the sdr epsilon which is used to avoid numerical instability.”
- `setEpsilonHdr(float r, float g, float b)` — “Sets the hdr epsilon which is used to avoid numerical instability.”
- `setDisplayRatioForFullHdr(float max)` — “Sets the hdr/sdr ratio at which point applying the gainmap results in an HDR rendition.”
- `setMinDisplayRatioForHdrTransition(float min)` — “Sets the hdr/sdr ratio below which applying the gainmap results in an SDR rendition.”
- `setGainmapDirection(int direction)` — “Sets the direction that the gainmap math should be applied in.”
- `setAlternativeImagePrimaries(ColorSpace)` / `getAlternativeImagePrimaries()` — “Gets the colorspace that the gainmap math should be applied in.”

【推断】`RatioMin/Max` ↔ `GainMapMin/Max`（linear ratio 语义），`EpsilonSdr/Hdr` ↔ `OffsetSDR/OffsetHDR`，`DisplayRatioForFullHdr` ↔ `HDRCapacityMax`，`MinDisplayRatioForHdrTransition` ↔ `HDRCapacityMin`，`GainmapDirection` ↔ `BaseRenditionIsHDR`，`AlternativeImagePrimaries` ↔ `use_base_color_space` 的对侧颜色空间。方法文档没有写出这些对应，故标为推断。

### B.5 ISO 21496-1 标准本身

【规范】ISO 官方目录页（`iso.org/standard/86775.html`）的信息：

| 项 | 值 |
| --- | --- |
| 标准号 | **ISO 21496-1:2025** |
| 全名 | *Digital photography — Gain map metadata for image conversion — Part 1: Dynamic range conversion* |
| 版次/日期 | Edition 1，2025-07 |
| 状态 | Published；Stage: International Standard published [60.60] |
| 页数 | 16 |
| 技术委员会 | ISO/TC 42 |
| ICS | 37.040.99 |
| 价格 | CHF 100（付费，非公开免费） |

Abstract 原文：

> This document defines a gain map used in HDR digital photography applications, for dynamic range conversion between two image representations. This includes the definition of the gain map metadata and its attributes, how to specify the gain map and associated metadata, and how to apply the gain map using this metadata.

生命周期（同页 “Life cycle” 段，原文日期）：New project approved 2023-07-06；Working draft (WD) study initiated 2023-10-04；Committee draft (CD) registered 2024-03-12；CD approved for registration as DIS 2024-09-08；DIS registered 2024-09-10；DIS ballot initiated 2024-11-11。

**是否公开可得**：目录页显示为付费标准（CHF 100，含 Online Browsing Platform 预览）。未发现 ISO 提供的免费全文。【二手】公开渠道中对字段与布局的等价描述主要来自 Adobe 的 Gain Map Specification（C 节）、Android 的 Ultra HDR 文档（B.1–B.4）以及 FFmpeg / libultrahdr 的源码（B.4.3、B.6）。

**标准范围的一手旁证。**【二手（ISO 委员会演示材料）】ISO/TC 42/WG 23 在 ICC 东京 HDR Expert Day 的演示材料中写道：

> Now ISO/TC42 is developing a standard ISO 21496-1 Digital photography - Gain map metadata for image conversion –Part 1: Dynamic range conversion, which defines a gain map used in HDR digital photography applications, for dynamic range conversion between image representations. This includes the definition of the gain map metadata and its attributes, how to apply a gain map, and how to specify the gain map and other associated metadata.

同一材料还说明其与 ICC 的分工：ISO/TC 42/WG 23 是 “Joint ISO/TC 42 - ISO/TC 130 - CIE WG: Extended colour encodings for digital image storage, manipulation and interchange”，并提到为 ICC 架构补充 Annex E、tag type `‘gain’` 等。

### B.6 URN `urn:iso:std:iso:ts:21496:-1` 的含义

**URN 语法。**【规范】IETF **RFC 5141**（*A Uniform Resource Name (URN) Namespace for the International Organization for Standardization (ISO)*）给出的 ABNF（Appendix B，原文）：

```
NSS           = std-nss
std-nss       = "std:" docidentifier *supplement *docelement [addition]
docidentifier = originator [":" type] ":" docnumber [":" partnumber]
                [[":" status] ":" edition]
                [":" docversion] [":" language]
```

RFC 原文说明：`std-nss` 前缀 “restricts identifiers to documents developed under ISO's standards-development procedures”；示例包括 `urn:iso:std:iso:9999:-1:ed-1:en`（“refers to the 1st edition of ISO 9999-1, in English”）。

据此逐段拆解 `urn:iso:std:iso:ts:21496:-1`：

| 段 | 值 | 含义（按 RFC 5141 语法） |
| --- | --- | --- |
| `std:` | — | ISO 标准类文档 |
| `originator` | `iso` | ISO 单独发布（非 ISO/IEC 联合） |
| `type` | `ts` | Technical Specification |
| `docnumber` | `21496` | 标准号 21496 |
| `partnumber` | `-1` | 第 1 部分 |

【推断】URN 里写 `ts`（Technical Specification），而 ISO 目录页显示最终以 **International Standard**（`ISO 21496-1:2025`，Stage 60.60）发布；URN 文本是在 TS 阶段铸定的标识符，与最终出版物类别不一致。这一点在 ISO 目录页与 RFC 5141 里都没有说明，属于我基于两者对照的推断，建议报告中表述为「URN 中的 `ts` 对应其 TS 阶段的文档类型」。

**URN 在文件里的作用（这是本仓库输出 JPEG 里出现该字符串的原因）。**【实现】libultrahdr v1.4.0 的 `lib/src/jpegdecoderhelper.cpp` 原文：

```cpp
static const uint32_t kAPP0Marker = JPEG_APP0;      // JFIF
static const uint32_t kAPP1Marker = JPEG_APP0 + 1;  // EXIF, XMP
static const uint32_t kAPP2Marker = JPEG_APP0 + 2;  // ICC, ISO Metadata
...
static constexpr uint8_t kIsoMetadataNameSpace[] = {
    'u', 'r', 'n', ':', 'i', 's', 'o', ':', 's', 't', 'd', ':', 'i', 's',
    'o', ':', 't', 's', ':', '2', '1', '4', '9', '6', ':', '-', '1', '\0',
};
```

即：ISO 21496-1 元数据被放在 **APP2** 段中，以该 URN（含结尾 NUL）作为命名空间/标识前缀。

【实现】FFmpeg 使用同一字符串作为标识：`libavutil/gain_map.h` 原文

```c
#define AV_ISO21496_VERSION    0
#define AV_ISO21496_IDENTIFIER "urn:iso:std:iso:ts:21496:-1"
#define AV_GAIN_MAP_MAX_PAYLOAD_SIZE 141 /* = 5 + 2*8 + 3*5*8 */
```

（该注释抓取时曾被破坏为 `= 5 + 28 + 358`，即 `*` 全部丢失；按原文 `5 + 2*8 + 3*5*8 = 5 + 16 + 120 = 141` 校验成立。）这条注释本身也描述了 ISO 21496-1 载荷布局【推断】：5 字节头（`min_version` u16 + `writer_version` u16 + `flags` u8）+ 2 个 headroom 值 × 8 字节（分子分母各 u32）+ 3 通道 × 5 个字段 × 8 字节；与 libultrahdr `gainmapmetadata.cpp` 的实际写入顺序（见 B.7）一致。

【规范】Android Ultra HDR v1.1 的 “ISO 21496-1 Compatibility” 一节说明了两种元数据共存时的位置与优先级（原文）：

> ISO 21496-1 provides an alternative encapsulation mechanism for encoding gain map metadata in an image file. You can encode both Ultra HDR metadata and ISO 21496-1 metadata in one JPEG file using a single gainmap image in the file.

> The ISO 21496-1 metadata appears immediately after the APP1 XMP segment in both JPEG images

> For maximum cross-platform compatibility, Android implementations and apps that implement their own encoding or decoding of JPEG files with gain maps should support encoding and decoding for both Ultra HDR v1 and ISO 21496-1 metadata. During an encode operation, the implementation or app should encode both metadata formats. If both types of metadata are present during a decode operation, the implementation or app should prefer using the ISO 21496-1 metadata.

【实现（本仓库实测）】对 `testoutput/MVIMG_3853.jpg`（本仓库转换产物）做字节级检查：

- 偏移 8808 处：`FF E2 00 22` 后紧跟 `urn:iso:std:iso:ts:21496:-1\0` —— 即主图的 APP2 ISO 21496-1 元数据段；
- 偏移 3604202 处：同一 URN 再次出现 —— 即 gain map 图像内的对应 APP2 段（与 Android 规范 “in both JPEG images” 一致）；
- 全文件未出现 `hdrgm`、`GainMap`、`GContainer` 等字符串；`exiftool -a -G1 -s` 只报出 `[MPF0] MPFVersion / NumberOfImages` 与 `[XMP-GCamera] MicroVideo*`，没有 `XMP-hdrgm` / `XMP-GContainer` 标签。

【推断】也就是说，本仓库当前产物携带的是 **ISO 21496-1 元数据 + MPF**，而**没有**写 Android v1.1 建议同时写入的 hdrgm（Ultra HDR v1）元数据与 GContainer XMP；libultrahdr 源码里确实存在写 hdrgm XMP 的代码（B.4.2 引用的 `jpegrutils.cpp`），本仓库走的是 `uhdr_enc_set_gainmap_image()` 这条「外部提供 gain map + 元数据」的路径（`src/i2a/convert.rs:273-283`）。两条路径的差异原因未逐行核实，报告若提及建议标注为「实测现象」。

### B.7 libultrahdr 对 ISO 21496-1 元数据的序列化

【实现】libultrahdr v1.4.0 的 `lib/include/ultrahdr/gainmapmetadata.h` 定义了分数形式的元数据结构与编解码：

```cpp
constexpr uint8_t kIsMultiChannelMask = (1u << 7);
constexpr uint8_t kUseBaseColorSpaceMask = (1u << 6);

// Gain map metadata, for tone mapping between SDR and HDR.
// This is the fraction version of {@code uhdr_gainmap_metadata_ext_t}.
struct uhdr_gainmap_metadata_frac {
    int32_t gainMapMinN;   uint32_t gainMapMinD;
    int32_t gainMapMaxN;   uint32_t gainMapMaxD;
    uint32_t gainMapGammaN; uint32_t gainMapGammaD;
    int32_t baseOffsetN;   uint32_t baseOffsetD;
    int32_t alternateOffsetN; uint32_t alternateOffsetD;
    uint32_t baseHdrHeadroomN; uint32_t baseHdrHeadroomD;
    uint32_t alternateHdrHeadroomN; uint32_t alternateHdrHeadroomD;
    bool backwardDirection;
    bool useBaseColorSpace;
    static uhdr_error_info_t encodeGainmapMetadata(const uhdr_gainmap_metadata_frac in_metadata,
                                                   std::vector<uint8_t>& out_data);
    static uhdr_error_info_t decodeGainmapMetadata(const std::vector<uint8_t>& in_data,
                                                   uhdr_gainmap_metadata_frac out_metadata);
    static uhdr_error_info_t gainmapMetadataFractionToFloat(const uhdr_gainmap_metadata_frac from,
                                                            uhdr_gainmap_metadata_ext_t to);
    ...
};
```

字段名与 B.4.3 中 FFmpeg 的命名一一对应（`baseHdrHeadroom` / `alternateHdrHeadroom` 等）。

【实现】`lib/src/gainmapmetadata.cpp` 的 `encodeGainmapMetadata()` 写入顺序（原文节选）：

```cpp
    const uint16_t min_version = 0, writer_version = 0;
    streamWriteU16(out_data, min_version);
    streamWriteU16(out_data, writer_version);
    uint8_t flags = 0u;
    // Always write three channels for now for simplicity.
    // TODO(maryla): the draft says that this specifies the count of channels of the
    // gain map. But tone mapping is done in RGB space so there are always three
    ...
        flags |= 8;   // useCommonDenominator
    streamWriteU8(out_data, flags);
    if (useCommonDenominator) {
        streamWriteU32(out_data, denom);
        streamWriteU32(out_data, in_metadata->baseHdrHeadroomN);
        streamWriteU32(out_data, in_metadata->alternateHdrHeadroomN);
        for (int c = 0; c < channelCount; ++c) {
            streamWriteS32(out_data, in_metadata->gainMapMinN[c]);
            streamWriteS32(out_data, in_metadata->gainMapMaxN[c]);
            streamWriteU32(out_data, in_metadata->gainMapGammaN[c]);
            streamWriteS32(out_data, in_metadata->baseOffsetN[c]);
            streamWriteS32(out_data, in_metadata->alternateOffsetN[c]);
        }
    } else {
        streamWriteU32(out_data, in_metadata->baseHdrHeadroomN);
        streamWriteU32(out_data, in_metadata->baseHdrHeadroomD);
        streamWriteU32(out_data, in_metadata->alternateHdrHeadroomN);
        streamWriteU32(out_data, in_metadata->alternateHdrHeadroomD);
        for (int c = 0; c < channelCount; ++c) {
            streamWriteS32(out_data, in_metadata->gainMapMinN[c]);
            streamWriteU32(out_data, in_metadata->gainMapMinD[c]);
            streamWriteS32(out_data, in_metadata->gainMapMaxN[c]);
            streamWriteU32(out_data, in_metadata->gainMapMaxD[c]);
            streamWriteU32(out_data, in_metadata->gainMapGammaN[c]);
            ...
        }
    }
```

解码侧 `decodeGainmapMetadata()` 先读 `min_version`，若不为 0 则返回 `UHDR_CODEC_UNSUPPORTED_FEATURE`，detail 为 “received unexpected minimum version %d, expected 0”。

【实现】`gainmapmetadata.h` 中的 `dump()` 方法还给出了全部字段的日志名（可用于核对字段集合）：`gainMapMinN/D`、`gainMapMaxN/D`、`gainMapGammaN/D`、`baseOffsetN/D`、`alternateOffsetN/D`、`baseHdrHeadroomN/D`、`alternateHdrHeadroomN/D`、`backwardDirection`、`useBaseColorSpace`。

【推断】该二进制布局（u16 版本 ×2 + u8 flags + 分数对）是 libultrahdr 对 ISO 21496-1 元数据载荷的实现，与 FFmpeg 头文件描述的 “ISO 21496-1 gain map metadata blob” 一致（FFmpeg 的 `av_gain_map_params_from_iso21496()` 文档写明 “data payload, excluding container-specific identifiers”），但我没有 ISO 21496-1 正文可逐字比对。

### B 节引用清单

- [B-1] Android Developers, *Ultra HDR Image Format v1.1* — https://developer.android.com/media/platform/hdr-image-format （Encode / Gain map generation、Gain map container、Decode、Display、ISO 21496-1 Compatibility、Change Log 各节；页脚 Last updated 2024-10-25）
- [B-2] ISO, *ISO 21496-1:2025 — Digital photography — Gain map metadata for image conversion — Part 1: Dynamic range conversion* — https://www.iso.org/standard/86775.html
- [B-3] IETF RFC 5141, *A Uniform Resource Name (URN) Namespace for the International Organization for Standardization (ISO)* — https://www.rfc-editor.org/rfc/rfc5141.txt （§2.4.2 示例、Appendix B ABNF）
- [B-4] ISO/TC 42/WG 23, *ICC and ISO 21496-1*（ICC Tokyo HDR Expert Day 材料）— https://www.color.org/events/hdr/tokyo2024_expert_day/ISO-TC42_WG_23_N7088_ICC_and_ISO_21496-1.pdf
- [B-5] google/libultrahdr **v1.4.0** 源码：`ultrahdr_api.h`、`lib/include/ultrahdr/gainmapmetadata.h`、`lib/include/ultrahdr/ultrahdrcommon.h`、`lib/src/gainmapmetadata.cpp`、`lib/src/gainmapmath.cpp`、`lib/src/jpegrutils.cpp`、`lib/src/jpegdecoderhelper.cpp` — https://github.com/google/libultrahdr/tree/v1.4.0
- [B-6] FFmpeg 源码：`libavutil/gain_map.h`、`libavcodec/gain_map.h` — https://github.com/FFmpeg/FFmpeg/blob/master/libavutil/gain_map.h ；文档页 https://ffmpeg.org/doxygen/trunk/libavutil_2gain__map_8h.html
- [B-7] Android Developers, `android.graphics.Gainmap` API reference — https://developer.android.com/reference/android/graphics/Gainmap
- [B-8] 本仓库代码与产物：`src/i2a/convert.rs`、`scripts/linux-build-libuhdr.sh`、`testoutput/MVIMG_3853.jpg`（字节偏移 8808 / 3604202 处的 APP2 段，实测）

---

## C. Adobe hdrgm 命名空间

### C.1 官方规范与命名空间

【规范】Adobe 发布的规范文档是 **“Gain Maps”**（文件名 `Gain_Map_1_0d14.pdf`，即 Version 1.0 draft 14），托管在 Adobe Camera Raw 帮助中心 `helpx.adobe.com/camera-raw/using/gain-map.html` 的下载区。该帮助页原文：

> Gain Map provides a solution for a consistent and adaptive display of HDR images. Conceptually, this method combines both SDR and HDR renditions within a single image and interpolates between the two dynamically at display time. A Gain Map represents the quotient between the two renditions. The image file stores a Base rendition (SDR or HDR), the Gain Map, and some associated metadata. At display time, the Base image is combined with the scaled version of the Gain Map, where the scale factor depends on the image metadata and the HDR capacity of the display. Gain Maps can be downsampled and compressed to save space.

> Gain Maps have been standardized in ISO 21496-1.

命名空间（【规范】Android 规范原文）：`The XMP namespace URI for the gain map metadata XMP extension is http://ns.adobe.com/hdr-gain-map/1.0/. The default namespace prefix is hdrgm.` 同一 URI 亦见于 libavif 的常量 `#define XML_NAME_SPACE_GAIN_MAP "http://ns.adobe.com/hdr-gain-map/1.0/"`（【实现】）。

【二手】Adobe 的 `Gain_Map_1_0d14.pdf` 里 “Full name” 一栏在抓取中为空（PDF 表格排版导致），但同页明确给出 “Short name: hdrgm”。

### C.2 字段表（Adobe 规范原文）

【规范】Adobe 规范中的 tag 表（逐字，列：Tag / Type / Count / Default / Notes）：

| Tag | Type | Count | Default | Notes |
| --- | --- | --- | --- | --- |
| `Version` | Text | Byte length of string | 1.0 | |
| `BaseRenditionIsHDR` | Boolean | 1 | false | |
| `GainMapMin` | Real or list of Real | 1 or 3 | 0 | |
| `GainMapMax` | Real or list of Real | 1 or 3 | 1 | |
| `Gamma` | Real or list of Real | 1 or 3 | 1 | |
| `OffsetSDR` | Real or list of Real | 1 or 3 | 1/64 | 𝑘_sdr |
| `OffsetHDR` | Real or list of Real | 1 or 3 | 1/64 | 𝑘_hdr |
| `HDRCapacityMin` | Real | 1 | 0 | |
| `HDRCapacityMax` | Real | 1 | 1 | |

必填性（原文）：

> The `Version`, `GainMapMax`, and `HDRCapacityMax` fields are required. The other fields are optional.

> If one of the required fields is missing or invalid, then the reader should ignore the gain map entirely.

缺省与元素个数（原文）：

> If an optional field is not specified, it is assumed to have the value indicated in the “Default” column of the table above; for multi-plane fields (such as `GainMapMin`), this default value applies to all color planes.

> If a field is specified, it must have the number of elements indicated in the “Count” column … for multi-plane fields, specifying a single element means that its value applies to all color profiles. For example, `GainMapMin` is optional, but if it’s specified, then it must have either 1 or 3 elements.

不等式约束（原文）：

> • `HDRCapacityMax` > `HDRCapacityMin` >= 0
> • `GainMapMax` >= `GainMapMin`
> • `OffsetSDR` >= 0
> • `OffsetHDR` >= 0
> • `Gamma` > 0

### C.3 示例与伪代码

【规范】规范给出的多通道 XMP 示例（原文节选，展示 Real 数组写法）：

```xml
<rdf:Description rdf:about=""
  xmlns:hdrgm="http://ns.adobe.com/hdr-gain-map/1.0/"
  hdrgm:BaseRenditionIsHDR="False"
  hdrgm:GainMapMin="0"
  <hdrgm:GainMapMax>
    <rdf:Seq>
      <rdf:li>3.554440</rdf:li>
      <rdf:li>3.473331</rdf:li>
      <rdf:li>3.374647</rdf:li>
    </rdf:Seq>
  </hdrgm:GainMapMax>
  <hdrgm:Gamma>
    <rdf:Seq>
      <rdf:li>0.394995</rdf:li>
      <rdf:li>0.440560</rdf:li>
      <rdf:li>0.482004</rdf:li>
    </rdf:Seq>
  </hdrgm:Gamma>
  hdrgm:OffsetSDR="0.015625"
  hdrgm:OffsetHDR="0.015625"
  hdrgm:HDRCapacityMin="0"
  hdrgm:HDRCapacityMax="2.3"/>
```

【规范】生成 gain map 的伪代码（原文节选）：

```
// Offset parameters, which need to be written to the
// Gain Map metadata so that the transform can be reversed later.
constexpr vec3 k_sdr = vec3(1.0/64.0f);
constexpr vec3 k_hdr = vec3(1.0/64.0f);

// Calculate gain map in log2 space.
return log2((hdr + k_hdr) / (sdr + k_sdr));
```

（抓取把 `1.0/64.0f` 显示为 `1.0-5f`，按上下文与 B.2 的默认值 1/64 判定为 `1.0/64.0f`，此处理标为【推断】。）

规范还给出 `ApplyGainMap(baseImage, gainMap, position, w, gainMapMin, gainMapMax, gainMapGamma, offsetBase, offsetOther)` 的伪代码签名，其中注释 “Read base image (e.g., SDR). This should be gamma 1.0.”。

### C.4 Adobe 的许可与标准化说明

【规范】Adobe 帮助页原文（“Gain Map Specification Patent License” 一节）：

> Adobe is the publisher of the Gain Map Specification describing a Gain Map-based approach for the storage, display, and tone mapping of High Dynamic Range images. Adobe provides the Gain Map Specification to the public to encourage the implementation of this file format in a compliant manner. This License is a patent license granted by Adobe to individuals and organizations that desire to develop, market, and/or distribute hardware and software that reads and/or writes image files compliant with the Gain Map Specification.

Android 文档页脚另有一句相呼应：“This product includes Gain Map technology under license by Adobe.”（【规范】，B-1）。

### C.5 工具侧支持

【二手】exiftool 的 XMP 标签文档把 `hdrgm` 标签族描述为 “Tags used in Adobe gain map images.”，可读写标签为：`BaseRenditionIsHDR`(boolean)、`GainMapMax`(real+)、`GainMapMin`(real+)、`Gamma`(real/+)、`HDRCapacityMax`(real)、`HDRCapacityMin`(real)、`OffsetHDR`(real+)、`OffsetSDR`(real+)、`Version`(string/)。exiftool 源码 `lib/Image/ExifTool/XMP.pm` 的命名空间表中同时登记了 `hdrgm`、`HDRGainMap`、`apdi`、`GContainer` 等前缀。

### C 节引用清单

- [C-1] Adobe, *Gain Map in Adobe Camera Raw*（帮助页，含 “Gain Map Specification Patent License”）— https://helpx.adobe.com/camera-raw/using/gain-map.html
- [C-2] Adobe, *Gain Maps*（Gain Map Specification v1.0 draft 14，PDF）— https://helpx.adobe.com/content/dam/help/en/camera-raw/using/gain-map/jcr_content/root/content/flex/items/position/position-par/table/row-io13dug-column-4a63daf/download_section/download-1/Gain_Map_1_0d14.pdf
- [C-3] Android Developers, *Ultra HDR Image Format v1.1*（hdrgm 命名空间 URI 与字段表的第一方重述）— https://developer.android.com/media/platform/hdr-image-format
- [C-4] exiftool, *XMP Tags*（XMP hdrgm 标签族）— https://exiftool.org/TagNames/XMP.html
- [C-5] exiftool 源码 `lib/Image/ExifTool/XMP.pm`（命名空间登记表）— https://github.com/exiftool/exiftool/blob/master/lib/Image/ExifTool/XMP.pm
- [C-6] libavif 源码 `apps/shared/avifjpeg.c`（`XML_NAME_SPACE_GAIN_MAP` 常量）— https://github.com/AOMediaCodec/libavif/blob/main/apps/shared/avifjpeg.c

---

## D. 相关工作

### D.1 开源实现与工具

**libultrahdr（Google 参考实现）**【规范（项目自述）】README 原文：

> libultrahdr is an image compression library that uses gain map technology to store and distribute HDR images.

> a Gain Map (quotient between the two renditions) is computed … uses backward compatible means to store the base image (SDR), gain map image and some associated metadata

> Legacy readers that do not support handling the gain map image and/or metadata, will display the base image.

README 列出 5 种编码场景（API-0 … API-4），其中 **API-4 = “SDR compressed, gain map compressed and GainMap metadata exist”**——正是本仓库使用的路径（`uhdr_enc_set_gainmap_image()`，见 `src/i2a/convert.rs:273-283` 与 `notes/libultrahdr.md`）。许可证为 Apache-2.0 + MIT 双许可（源码文件头，如 `lib/include/ultrahdr/gainmapmetadata.h`： “Licensed under the Apache License, Version 2.0”）。

【二手】发行说明（FreshPorts 包页）称 1.1.0（2024-08）加入 ISO 21496-1 支持、1.0.0（2023-10）首发。本仓库固定的版本是 **v1.4.0**（`scripts/linux-build-libuhdr.sh:10`）。

**libavif**【实现】`CHANGELOG.md` 原文：

> [1.1.0] - 2024-07-11 … Add experimental API for reading and writing gain maps in AVIF files. If enabled at compile time, add gainMap field to avifImage, add qualityGainMap field to avifEncoder, add gainMapPresent, enableDecodingGainMap, enableParsingGainMapMetadata and ignoreColorAndAlpha to avifDecoder. … This feature is highly experimental.

后续版本有 “Update gain map metadata to current ISO 21496-1 draft.” 的条目。`apps/shared/avifjpeg.c` 同时实现两种 JPEG 侧元数据的解析：Adobe/hdrgm 形式（读 `HDRCapacityMin`、`HDRCapacityMax`、`OffsetSDR`、`OffsetHDR`、`GainMapMin`、`GainMapMax`、`Gamma`）与 **Apple 形式**（读 `HDRGainMapHeadroom`，转成 `log2` 后写入 ISO 21496-1 的分数结构）——见 A.3 引文。命令行入口为 `avifenc --qgain-map`（【二手】，libheif issue #1685 正文）。

【实现】libavif 的 AVIF 侧 gain map 走 HEIF 的 `'tmap'` 派生图项；AVIF 规范 §4.2.2 原文：“A tone map derived image item (`'tmap'`) as defined in [HEIF] may be used in an AVIF file.” 该规范把 `'tmap'` 的定义归于 [HEIF] = ISO/IEC 23008-12，未在本文件中展开其元数据语法，也未引用 ISO 21496-1。

**FFmpeg**【实现】`libavutil/gain_map.h` 提供 `AVGainMapParams` 与 `av_gain_map_params_from_iso21496()` / `av_gain_map_params_to_iso21496()`；`libavcodec/gain_map.h` 的模块说明原文：

> Serialization of AVGainMapParams into the metadata blobs that image formats carry it in, and back. Kept separate from the codecs so that an encoder writing the blob and a bitstream filter splicing it into an existing bitstream can share the same code.

其中 `ff_gain_map_params_to_xmp()` 的文档为 “Serialize an AVGainMapParams as an Ultra HDR XMP packet.”，并被 `jpeg_put_comments()`、`mpf_write_xmp()`、`parse_app1()` 引用——即 FFmpeg 的 JPEG 编解码器同时支持 Ultra HDR XMP 与 ISO 21496-1 blob 两种写法。

**ImageMagick**【实现】仓库中存在 `coders/uhdr.c` 与 `coders/uhdr.h`；`uhdr.c` 直接调用 libultrahdr 的 C API（`uhdr_raw_image_t`、`uhdr_compressed_image_t`、`uhdr_gainmap_metadata_t`、`uhdr_mem_block_t`），并通过 `GetHDRGMProperty3("GainMapMax", …)`、`GetHDRGMProperty3("GainMapMin", …)`、`GetHDRGMProperty3("Gamma", …)`、`GetHDRGMProperty3("OffsetSDR", …)`、`GetHDRGMProperty3("OffsetHDR", …)`、`GetHDRGMProperty("HDRCapacityMin", …)`、`GetHDRGMProperty("HDRCapacityMax", …)`、`GetHDRGMPropertyInt("UseBaseColorGrade", …)` 读取 hdrgm 元数据。文件内还有 gain map 的方向/裁剪状态机（`UltraHDRGainMapTransformState`、`UltraHDRGainMapOrientation`）。【二手】发行信息（Fedora 打包日志、CVE-2026-93590 描述）称 Ultra HDR 支持出现在 7.1.2.x 系列；我未在 ImageMagick 官方 changelog 中核对到引入版本。

**libheif**【实现】仓库 `libheif/` 目录下没有任何名字含 gain/hdr/tmap 的源文件；公共 API 只有通用辅助图接口（A.1 的 `heif_aux_images.h`）。因此 libheif 能**取出** Apple gain map（作为 auxiliary image），但不解释其 gain map 语义，也不生成 Ultra HDR。【二手】社区 issue “Apple iPhone's HEIC/HEIF gain maps (HDR support)”（#1685，2026-01-27 创建，2026-09-08 仍为 open）正文称 `heif-dec --quality 100 test.heic test.jpeg` 不保留 HDR，并请求 libheif 支持 Apple gain map。

**exiftool**【实现】三套标签族：`XMP-hdrgm`（Adobe gain map）、`XMP-HDRGainMap`（“Used in Apple HDR GainMap images.”，含 `HDRGainMapVersion`）、`XMP-apdi`（同为 Apple HDR GainMap 图像使用，含 `AuxiliaryImageType` / `NativeFormat` / `StoredFormat`）；MakerNotes 侧见 A.3。本仓库用 exiftool 读取 Apple 侧四个键并回写元数据（`src/i2a/exiftool.config`、`src/i2a/convert.rs:216-219`）。

**apple-hdr-heic（第三方）**【二手（项目自述）】README 原文：

> A library/tool to decode photos (HEIC files) taken on an iPhone that contain HDR gain map, and convert it to standard HDR representations such as: 1. A 16- or 32-bit per channel float representation in a given *linear* color space, which could be saved to an EXR file. 2. A 48-bit (16-bit per channel) representation as per Rec. 2100 with PQ transfer function, which could be saved to a PNG file.

该 README 亦说明其不保留 HDR→SDR 的 tone-mapping 信息（“the HDR-to-SDR tone-mapping information will be lost. To preserve that, an additional layer of information is required (like the HDR gain map in the original image, which is an inverse tone-map)”）。本仓库的 `src/i2a/convert.rs:29` 亦致谢其 metadata 读取逻辑。

**本仓库自身**（作为 pipeline 实例，供 Background 举例）：读 Apple HEIC 的辅助图（URN，A.1）→ 反变换 gain map（A.2）→ 重参数化为 UltraHDR 的 `log_recovery`（B.2）→ 由 libultrahdr API-4 写出带 ISO 21496-1 元数据的 JPEG（B.6 实测），并追加 Live Photo 视频（GCamera XMP `MicroVideo*`，见 `src/i2a/exiftool.config` 与产物实测）。

### D.2 学术与技术报告

- **Gamma Maps: Non-linear Gain Maps for HDR Reconstruction** — Trevor D. Canham; SaiKiran K. Tedla; Michael J. Murdoch; Michael S. Brown. *London Imaging Meeting* 2024 (Image Capture: From Photons to Photos), Vol. 5, Issue 1, Article 13, pp. 52–58, Published Online June 2024. 【二手（摘要为转述）】摘要要点：既有框架在 SDR 图里嵌入 residual 元数据以恢复 HDR，典型做法是逐像素 HDR/SDR 比值的压缩 gain map；作者提出用 “a pixel-wise exponent map instead of the multiplicative gain map”，实验显示多种指标下 HDR 重建质量更高。出处：https://library.imaging.org/lim/articles/5/1/13
- **Gain-MLP: Improving HDR Gain Map Encoding via a Lightweight MLP** — Trevor D. Canham, SaiKiran Tedla, Michael J. Murdoch, Michael S. Brown；arXiv:2503.11883（v1, 2025-03-14），DOI 10.48550/arXiv.2503.11883；CVF 站点有 ICCV 2025 论文 PDF（`Canham_Gain-MLP_Improving_HDR_Gain_Map_Encoding_via_a_Lightweight_MLP_ICCV_2025_paper.pdf`）。【二手（摘要为转述）】用轻量 MLP 编码 gain map，固定 10 KB 开销，不依赖图像尺寸或编码参数调整。
- **HDR Image Generation via Gain Map Decomposed Diffusion** — ICCV 2025（CVF open access，含 supplementary）。【二手】我仅从检索结果看到标题与 CVF URL，未读到正文：https://openaccess.thecvf.com/content/ICCV2025/supplemental/Guan_HDR_Image_Generation_ICCV_2025_supplemental.pdf
- **Apple 专利 US 2024/0153053 A1**，“Techniques for Generating Gain Maps Based on Acquired Images”。【二手】仅从检索结果得到题名与公开号，未读原文；如报告要引用需先核实。

### D.3 Apple 与 Android 两套方案的差异（逐条对照）

下表每行都给出两侧的一手出处；「差异」列中未标注的即为两侧文档字面差异。

| 维度 | Apple（HEIC） | Android UltraHDR（JPEG） |
| --- | --- | --- |
| 容器 | HEIC；gain map 作为 auxiliary image，类型 URN `urn:com:apple:photo:2020:aux:hdrgainmap`（[A-1]） | JPEG；gain map 作为 MPF secondary image，主图与 gain map 的 XMP 用 GContainer 描述（[B-1]） |
| 格式信号 | `HDRGainMapVersion`（XMP，Apple 命名空间）存在即有效（[A-1]） | 主图 XMP 中 `hdrgm:Version="1.0"`（[B-1]） |
| 位深/通道 | 8-bit、single-channel（[A-1]） | 8-bit 无符号、单通道优先（[B-1]） |
| 传递函数 | gain map “untagged and formatted as linear data”，以 Rec.709 transfer function 编码（[A-1]） | 规范未规定 gain map 像素的传递函数，只规定 recovery 值到 8-bit 的量化与可选 `map_gamma`（[B-1]） |
| 分辨率 | 1/4（[A-1]） | 实现定义，建议 1/16 面积（每维 1/4）（[B-1]） |
| headroom 语义 | 单一标量 headroom；应用式 `hdr = sdr * (1 + (headroom-1) * gainmap)`（[A-1]） | 元数据存 log2 域的 `GainMapMin/Max`、`HDRCapacityMin/Max`，应用式带 display boost 权重与 offset（[B-1]） |
| headroom 元数据位置 | XMP `HDRGainMapHeadroom`（Apple 命名空间，[A-7]）或 MakerNote 33/48 公式（[A-1]） | 主图/gain map XMP 的 hdrgm 字段，或 APP2 的 ISO 21496-1 元数据（[B-1][B-6]） |
| 元数据标准归属 | Apple 私有格式；Apple 声明已提交 ISO/NP 21496-1 标准化并承诺专利许可（[A-1]） | Adobe 规范 + ISO 21496-1；v1.1 明确「两种元数据都写、解码优先 ISO」（[B-1][C-1]） |
| 多通道 | 单通道（[A-1]） | 允许每通道元数据（`GainMapMin/Max/Gamma/Offset*` 可为 1 或 3 个元素）（[B-1][C-2]） |

【推断】两套方案在「SDR 基图 + 增益图 + 元数据」这一模型上同构；差异集中在元数据的参数化方式（Apple 用单一 headroom + 固定线性插值 vs Android 用 log2 域区间 + 显示能力权重）与容器承载方式（HEIC auxC URN vs JPEG MPF + XMP/APP2）。我没有找到 Apple 或 Google 官方发布的正面差异分析文档；上述对照是我从两侧一手文档整理的。

### D.4 工具支持矩阵（按本文已核实的来源）

| 工具 | Apple HEIC gain map | Android UltraHDR（hdrgm XMP） | ISO 21496-1 元数据 | 出处 |
| --- | --- | --- | --- | --- |
| libultrahdr v1.4.0 | 未见支持（其输入为 SDR/HDR 或压缩图，无 HEIC 解析）【推断】 | 写/读（`jpegrutils.cpp`）【实现】 | 写/读（`gainmapmetadata.cpp`、APP2 URN）【实现】 | [B-5] |
| libavif | 读（`avifJPEGParseGainMapXMPPropertiesAppleFormat`）【实现】 | 读（hdrgm 字段解析）【实现】 | 读/写（tmap + ISO 元数据；1.1.0 起实验性 API）【实现】 | [D-1] |
| FFmpeg | 未见（仅 hdrgm/ISO）【推断】 | 写/读（`ff_gain_map_params_to_xmp`）【实现】 | 写/读（`AV_ISO21496_IDENTIFIER`）【实现】 | [B-6] |
| ImageMagick | 未见【推断】 | 读 hdrgm 属性；经 libultrahdr 读写 Ultra HDR【实现】 | 间接（经 libultrahdr）【推断】 | [D-2] |
| libheif | 可取出 auxiliary image，但不解释 gain map【实现】 | 未见【推断】 | 未见【推断】 | [A-5][D-3] |
| exiftool | 读 XMP `HDRGainMap*`、MakerNote 33/48【实现】 | 读/写 `XMP-hdrgm` 标签族【二手】 | 未见专用标签族【推断】 | [A-6][A-8] |
| apple-hdr-heic | 读（转 EXR / PQ PNG）【二手】 | 无【二手】 | 无【二手】 | [D-4] |

### D 节引用清单

- [D-1] libavif：`CHANGELOG.md`、`apps/shared/avifjpeg.c` — https://github.com/AOMediaCodec/libavif
- [D-2] ImageMagick：`coders/uhdr.c`、`coders/uhdr.h` — https://github.com/ImageMagick/ImageMagick/tree/main/coders
- [D-3] strukturag/libheif issue #1685, *Apple iPhone's HEIC/HEIF gain maps (HDR support)* — https://github.com/strukturag/libheif/issues/1685
- [D-4] johncf/apple-hdr-heic — https://github.com/johncf/apple-hdr-heic
- [D-5] google/libultrahdr README — https://github.com/google/libultrahdr/blob/main/README.md
- [D-6] AV1 Image File Format (AVIF) Specification — https://aomediacodec.github.io/av1-avif/ （§4.2.2 Tone Map Derived Image Item）
- [D-7] Canham et al., *Gamma Maps: Non-linear Gain Maps for HDR Reconstruction*, London Imaging Meeting 2024 — https://library.imaging.org/lim/articles/5/1/13
- [D-8] Canham et al., *Gain-MLP: Improving HDR Gain Map Encoding via a Lightweight MLP* — https://arxiv.org/abs/2503.11883 ；ICCV 2025 PDF: https://openaccess.thecvf.com/content/ICCV2025/papers/Canham_Gain-MLP_Improving_HDR_Gain_Map_Encoding_via_a_Lightweight_MLP_ICCV_2025_paper.pdf
- [D-9] *HDR Image Generation via Gain Map Decomposed Diffusion*, ICCV 2025（未读原文）— https://openaccess.thecvf.com/content/ICCV2025/supplemental/Guan_HDR_Image_Generation_ICCV_2025_supplemental.pdf

---

## 附录：尚未追到一手来源的问题（写作时勿写成定论）

1. **`urn:com:apple:photo:2020:aux:hdrgainmap` 是否有标准组织层面的注册或定义**：IANA URN 注册表无 `com` 命名空间（[A-4]）；未在 ISO/IEC 23008-12（HEIF）公开信息中找到该 URN 的登记证据。Apple 文档是唯一权威来源。
2. **`HDRGainMapHeadroom` 的官方定义**：只在 libavif 源码（[A-7]）与 exiftool 标签族（[A-8]）中出现，Apple 公开文档未提及；Apple 命名空间 URI 亦只有实现侧常量。
3. **Apple 是否在 iOS 18+ 的 HEIC 中改用 `urn:iso:std:iso:ts:21496:-1` 作为辅助图类型**：仅确认 SDK 常量 `kCGImageAuxiliaryDataTypeISOGainMap` 的存在（[A-3]），未找到 Apple 说明其容器写法的文档。
4. **ISO 21496-1 正文的逐字条款**：标准付费（[B-2]），本文件的 ISO 侧描述来自 Adobe 规范（C 节）、Android 文档（B.1–B.4）与 FFmpeg/libultrahdr 源码（B.4.3、B.7），三者一致但都不等同于标准原文。
5. **libultrahdr 为何在本仓库使用的 API-4 路径下不写 hdrgm XMP**：仅记录了实测现象与代码路径存在（B.6），未做因果核实。
6. **ImageMagick Ultra HDR 支持的引入版本**：只有打包侧与 CVE 描述等二手信息。
