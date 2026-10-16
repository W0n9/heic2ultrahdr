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

【实现】调研当时，本仓库 `src/i2a/convert.rs` 对 gain map 做的是 **sRGB EOTF 的反变换**（`reverse_srgb_transform`：`u <= 0.04045 ? u/12.92 : ((u+0.055)/1.055)^2.4`），而 Apple 文档写的是 “Rec.709 transfer function”。两者不是同一个函数（分段点与指数都不同）。

> **已变更（2026-10-16）**：实测后本仓库已改用 **Rec.709 反变换**（`reverse_rec709_transform`），并以单元测试钉住期望值。理由与实测数据见 A.8.1。下方 A.6.1 记录的「继承自 `apple-hdr-heic` 的 sRGB 选择」是变更前的状态。

> **后续调研（A.5–A.8）**：这条差异的完整取证见 A.5（Apple 侧 API 与可用性）、A.6（六个独立实现各用什么函数）、A.7（有没有人实测过）、A.8（结论与实测方案），以及 B.8（ISO 21496-1 是否规定 gain map 像素的传递函数）。

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

### A.5 `kCGImageAuxiliaryDataTypeISOGainMap`、Apple 官方用法，以及无显示器环境下的第一方参照路径

本节回答「Q1」：两个 gain map 辅助图常量各自的可用性门槛与语义、Apple 官方怎么用它们，以及能否在无显示器的 macOS runner 上拿到 Apple 侧的参照像素。

#### A.5.1 可用性门槛

【规范】Apple 文档元数据中的 `platforms` 字段（`introducedAt`，逐字）：

| 常量 | iOS | iPadOS | Mac Catalyst | macOS | tvOS | visionOS | watchOS |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `kCGImageAuxiliaryDataTypeHDRGainMap` | 14.1 | 14.1 | 14.1 | 11.0 | 14.0 | 1.0 | 7.0 |
| `kCGImageAuxiliaryDataTypeISOGainMap` | 18.0 | 18.0 | 18.0 | **15.0** | 18.0 | 2.0 | 11.0 |

【推断】Apple 的 `introducedAt` 语义是「自该版本起可用」，故 **macOS 15.0 及以上（含 macOS 26）都能调用 `kCGImageAuxiliaryDataTypeISOGainMap`**。本仓库 release CI 的 macOS 任务是 `- { os: macos-26, arch: arm64 }`（`.github/workflows/release.yml:29`），已满足该门槛。

#### A.5.2 官方文档对这两个常量说了什么——以及它没说什么

【规范】`kCGImageAuxiliaryDataTypeHDRGainMap` 的 abstract 是其文档页上唯一一句说明：

> The type for High Dynamic Range (HDR) gain map information.

【规范】`kCGImageAuxiliaryDataTypeISOGainMap` 的文档页**没有任何说明文字**：其文档 JSON 的 `abstract` 为 `null`，`primaryContentSections` 只含 `declarations`（即 `let kCGImageAuxiliaryDataTypeISOGainMap: CFString`），`sections` 为空数组——该页除声明与可用性表外没有 Discussion 段。

【推断】A.1 中「两个常量并存说明 Apple 在 SDK 层面区分两种辅助图」**至今仍只是推断**：Apple 从未在 API 参考里解释 `kCGImageAuxiliaryDataTypeISOGainMap` 的语义。官方解释只出现在 WWDC24 的 session 里（见 A.5.3）。

#### A.5.3 Apple 官方怎么用它：WWDC24 session 10177

【规范】WWDC24 session **10177**（*Use HDR for dynamic image experiences in your app*）字幕原文（节选）：

> With the release of iOS 18, we are transitioning to Adaptive HDR Gain Maps and related metadata. This new generation is based on the latest draft.

> Last year we introduced the CIImage option expandToHDR to support the Apple Gain Map image format. This same API now also works to read Adaptive HDR files.

> This option is also available via the ImageIO API kCGImageSourceDecodeToHDR.

> To use the SDR and Gain approach, load both the SDR and Gain Map components of the file as two in-memory image objects. Use the auxiliaryHDRGainMap option to load the Gain Map from a file as a CIImage object.

> Note that the Gain image is typically half the size of the SDR image so edits need to account for the scale difference.

【规范】同一 session 给出**写入** ISO gain map 的官方路径：

> Lastly, your app can use ImageIO to save an SDR CGImage and Gain Map data. The first step is to call CGImageDestinationAddImage with an SDR CGImage with an SDR CGColorSpace. The next step is to create a dictionary that describes the Gain Map. This should contain the actual pixel data of the Gain Map, a sub-dictionary that describes the height, width, and format of that data and a CGImageMetadata that describes how the pixel data should be converted to linear gain values. In most cases you can re-use the CGImageMetadata from the source file. Then, all that is needed is to call CGImageDestinationAddAuxiliaryDataInfo and pass the new kCGImageAuxiliaryDataTypeISOGainMap key and the info dictionary.

【推断】由此可确定两者的分工：`kCGImageAuxiliaryDataTypeHDRGainMap` 对应 2020 年起 iPhone 拍摄的 **Apple Gain Map**（Rec.709 编码 + 单一 headroom，见 A.2）；`kCGImageAuxiliaryDataTypeISOGainMap` 对应 iOS 18 起 Apple 转向的 **Adaptive HDR / ISO 21496-1** gain map（B 节）。session 明说后者 “based on the latest draft”，且转写发生在 iOS 18。

【规范】`CIImage.applyingGainMap(_:)`（macOS 15.0+）文档给出了取 gain map 的官方写法与一个静默失败条件：

> The gain map image can be obtained by creating a `CIImage` instance from `NSURL`/`NSData` and setting the `auxiliaryHDRGainMap` option set to `@YES`.

> If the gain map `CIImage` instance doesn’t have the needed `properties` metadata, the received image will be returned as-is.

【推断】第二句意味着：**只拿到 gain map 的像素还不够，还必须带上它的 `properties`（即 Apple 那份 XMP/gain map 元数据）**，否则 `applyingGainMap` 不报错、直接原样返回 SDR 图。任何以 CoreImage 为参照的实测都必须断言输出与输入不同，否则会把「静默不生效」误判成「结果一致」。

#### A.5.4 它返回编码域还是已线性化的数据

【规范】Apple 那篇文章把「取数」与「线性化」写成两个独立步骤，且把 API 取到的数据与 URN 路径取到的数据等同：

> Get the existing HDR gain map from the image's auxiliary data using the `urn:com:apple:photo:2020:aux:hdrgainmap` image data type. The gain map is untagged and formatted as linear data. It's encoded using the Rec.709 transfer function and is 1/4 the resolution of the original image.

> For example, using Apple APIs, you can extract the gain map from a `CGImage` object by passing `kCGImageAuxiliaryDataTypeHDRGainMap` to the `CGImageSourceCopyAuxiliaryDataInfoAtIndex` function.

> 2. Linearize the gain map by inverting the gain map gamma using the Rec.709 transfer function.

【推断】这段把「用 API 取到的那份数据」与「Rec.709 编码的那份数据」等同起来，即 **`CGImageSourceCopyAuxiliaryDataInfoAtIndex(..., kCGImageAuxiliaryDataTypeHDRGainMap)` 返回的是编码域（Rec.709 传输函数编码后）的原始像素，线性化是调用方的责任**。需要指出该句自身有内在张力：“formatted as linear data” 与 “encoded using the Rec.709 transfer function” 不能同时按字面成立（Rec.709 传输函数是分段幂函数，非线性）；按上下文，“linear data” 应理解为「untagged、无 ICC 的单通道标量数据」，“encoded using the Rec.709 transfer function” 才是对像素值的操作性描述。这是【推断】，Apple 没有明说如何消解这个张力。

【规范】WWDC24 对 ISO gain map 的对应说法指向同一结论——元数据的作用是「描述像素如何转成线性增益值」，即存储的像素本身不是线性的：

> … and a CGImageMetadata that describes how the pixel data should be converted to linear gain values.

【实现】WebKit 是目前能找到的、公开调用这两个常量的非 Apple 代码，其取数方式提供了额外线索（`Source/WebCore/platform/graphics/cg/ImageDecoderCG.cpp`，`ImageDecoderCG::frameGainMapAtIndex()`）：

```cpp
CFDictionarySetValue(auxiliaryOptions.get(), kCGImageAuxiliaryDataRepresentation, kCGImageAuxiliaryDataRepresentationPixelBuffer);

RetainPtr auxiliaryInfo = adoptCF(CGImageSourceCopyAuxiliaryDataInfoAtIndexWithOptions(m_nativeDecoder.get(), index, kCGImageAuxiliaryDataTypeISOGainMap, auxiliaryOptions.get()));
if (!auxiliaryInfo)
    auxiliaryInfo = adoptCF(CGImageSourceCopyAuxiliaryDataInfoAtIndexWithOptions(m_nativeDecoder.get(), index, kCGImageAuxiliaryDataTypeHDRGainMap, auxiliaryOptions.get()));
...
RetainPtr metadata = dynamic_cf_cast<CGImageMetadataRef>(CFDictionaryGetValue(auxiliaryInfo.get(), kCGImageAuxiliaryDataInfoMetadata));
RetainPtr gainMapPixelBuffer = dynamic_cf_cast<CVPixelBufferRef>(CFDictionaryGetValue(auxiliaryInfo.get(), kCGImageAuxiliaryDataInfoPixelBuffer));
RetainPtr colorSpace = dynamic_cf_cast<CGColorSpaceRef>(CFDictionaryGetValue(auxiliaryInfo.get(), kCGImageAuxiliaryDataInfoColorSpace));
```

三点【实现】观察：

1. WebKit **先试 ISO、失败再回落 Apple**，方向与 Android 规范「两种元数据共存时优先 ISO」（B.1）一致；
2. 它用的是带 options 的变体 `CGImageSourceCopyAuxiliaryDataInfoAtIndexWithOptions` 加 `kCGImageAuxiliaryDataRepresentationPixelBuffer`，从而拿到 `CVPixelBuffer`；这两个键**不在公开的 `CGImageSource.h` 里**（我读到的 iPhoneOS18.0 SDK 头文件中没有这两个符号），在 developer.apple.com 上也没有对应文档页，属于未公开键；
3. 取回的 gain map **带一个可选 `CGColorSpace`**。

【推断】若 Apple 已经替调用方完成线性化，返回的 `CGColorSpace` 理应是一个线性空间；它作为「可选 color space」随像素一起返回，更像是在标注「这 8-bit 像素属于哪个色彩空间」。但 WebKit 代码没有说明该 color space 具体是什么，Apple 也没有文档描述这条带 options 的路径。因此**「ImageIO/CoreImage 是否已经替调用方线性化」无法从公开一手资料定论**，只能靠实测（把 API 返回的字节与文件内 gain map 码流的解码结果逐字节比对）回答。

#### A.5.5 无显示器 macOS runner 上能否拿到 Apple 侧参照像素

【规范】可用的第一方入口（可用性取自各页 `introducedAt`）：

| API | macOS 可用性 | 文档原文 / 头文件注释 |
| --- | --- | --- |
| `CIImage(contentsOf:options: [.expandToHDR: true])` | 14.0+ | “A Boolean value that indicates whether to read Gain Map HDR images as HDR.” |
| `kCGImageSourceDecodeRequest` = `kCGImageSourceDecodeToHDR` | 14.0+ | `kCGImageSourceDecodeRequest - value is one of the predefined keys (kCGImageSourceDecodeToHDR, kCGImageSourceDecodeToSDR, ...)` |
| `CIImage(data:options: [.auxiliaryHDRGainMap: true])` | 11.0+ | 见 A.5.3（session 10177 与 Apple 论坛 DTS 回复均用此法） |
| `CIImage.applyingGainMap(_:)` | 15.0+ | “Create an image that applies a gain map Core Image image to the received Core Image image.” |
| `CIContext.render(_:toBitmap:rowBytes:bounds:format:colorSpace:)` | 10.4+ | “Renders to the given bitmap.” |

【规范】CoreImage 的默认工作色彩空间是线性的：

> The default working space is the extended sRGB color space with linear gamma.

【推断】这条默认值是「无显示器也能拿参照」的关键：`CIContext` 是渲染器而非显示层，`render(_:toBitmap:...)` 把结果直接写进调用方提供的缓冲区，不需要任何 window/display；只要目的 `CGColorSpace` 与 `CIFormat`（如 `.RGBAh` / `.RGBAf`）取扩展线性空间，读回的浮点值就是 Apple 解码链给出的线性 HDR 值。`kCGImageSourceDecodeToHDR` 这条路同样不需要显示器（走 ImageIO 解码器），拿到 `CGImage` 后画进 `CGColorSpace.extendedLinearDisplayP3` 的浮点 `CGContext` 即可读出参照像素。

【规范】一个会改变「参照值」定义的细节（SDK 头文件注释，拼写照录）：

> kCGImageSourceGenerateImageSpecificLumaScaling - generate a global tone mapping function based on the gain map. Dafault value is "YES" (kCFBooleanTrue)

【推断】即 `kCGImageSourceDecodeToHDR` **默认会基于 gain map 生成一条全局 tone mapping 曲线**，读回的值因此未必是「SDR × 纯 gain map」的裸结果，而是经 Apple 自选曲线映射后的结果。若要复现 Apple 的 gain map 数学，应把该选项置为 `kCFBooleanFalse`，或改走 `.auxiliaryHDRGainMap` + `applyingGainMap(_:)`。此条**未实测**，仅作为待验证项记录。

【推断】以下风险必须在 runner 上实测才能定：

1. GitHub 的 `macos-26` 是虚拟化环境，Metal 设备可能不可用。`CIContext` 在无 Metal 时是否自动回落、`useSoftwareRenderer`（其文档只写 “This option has no effect if the platform does not support OpenCL.”）能否覆盖，均**未验证**。相较之下 `CGImageSource` + `CGContext`（CoreGraphics）路径是纯 CPU 的，风险更低。
2. `expandToHDR` / `kCGImageSourceDecodeToHDR` 对**2020 年格式的 Apple gain map HEIC**（而非 ISO/Adaptive 文件）是否生效：session 10177 说 `expandToHDR` “support the Apple Gain Map image format”，方向上支持，但 macOS 26 上的实际行为**未实测**。
3. 色域：本仓库断言输入为 Display P3（`src/i2a/convert.rs:253-257`），参照路径必须以 Display P3 的扩展线性空间读回，否则会引入色彩管理差异，使比对结果失去意义。

### A.6 各独立实现线性化 Apple gain map 时用的是哪个函数

本节回答「Q3」。逐个读源码确认。结论先行（下表每行的依据见下文）：

| 实现 | 是否处理 Apple gain map | 对 gain map **像素**用的函数 | 出处 |
| --- | --- | --- | --- |
| `johncf/apple-hdr-heic` | 是（唯一完整实现） | **sRGB EOTF**（`colour.models.eotf_sRGB`） | `src/apple_hdr_heic/lib.py:38-43` |
| `strukturag/libheif` | 只取出辅助图，不解释语义 | 无 | 全仓库 843 个文件无 gainmap 命名 |
| `monogrid/gainmap-js` | 否（只做 Adobe/ISO 一路） | 无（只实现 ISO 数学） | `src/decode/**/GainMapDecoderMaterial.ts` |
| `darktable` | 否 | 无 | 全仓库 2360 个文件无 gainmap 命名 |
| `ImageMagick` | 否 | 无（全部委托 libultrahdr） | `coders/uhdr.c`、`coders/heic.c` |
| `AOMediaCodec/libavif` | 是（只做元数据换算） | **不做任何传递函数**，像素原样搬运 | `apps/shared/avifjpeg.c` |

#### A.6.1 `johncf/apple-hdr-heic` —— sRGB EOTF

【实现】`src/apple_hdr_heic/lib.py` 的 `apply_hdrgainmap()` 原文（含注释）：

```python
# note: Display P3 and sRGB have the same EOTF
dp3_sdr_linear = colour.models.eotf_sRGB(dp3_sdr)
assert dp3_sdr.shape[:2] == hdrgainmap.shape
assert headroom >= 1.0
hdrgainmap_linear = colour.models.eotf_sRGB(hdrgainmap)
scale_factor_map = 1.0 + (headroom - 1.0) * hdrgainmap_linear  # between 1.0 and headroom
dp3_hdr_linear = dp3_sdr_linear * scale_factor_map[:, :, None]
```

【实现】它对 SDR 图与 gain map 用的是**同一个** `colour.models.eotf_sRGB`，理由写在注释里：「Display P3 and sRGB have the same EOTF」。

【实现】`colour.models.eotf_sRGB` 的实现（`colour/models/rgb/transfer_functions/srgb.py`）原文，docstring 自述为 “Apply the *IEC 61966-2-1:1999* *sRGB* electro-optical transfer function (EOTF).”：

```python
L = xp.where(
    eotf_inverse_sRGB(0.0031308) >= V,
    V / 12.92,
    spow((V + 0.055) / 1.055, 2.4),
)
```

【推断】`eotf_inverse_sRGB(0.0031308)` 即 0.04045…，故该函数与**调研当时**本仓库的 `reverse_srgb_transform` **逐字等价**（同为分段点 0.04045、指数 2.4），而不是 Rec.709 传输函数（分段点 0.081、指数 1/0.45≈2.222）。本仓库在 `src/i2a/convert.rs:29` 只致谢了该项目的 metadata 读取逻辑，但传输函数的选择与之一致——**本仓库当时的 sRGB 反变换并非孤立选择，它继承了 `apple-hdr-heic` 的做法**。该选择已于 2026-10-16 改为 Rec.709，见 A.8.1。

【推断】该实现给出的理由是「Display P3 与 sRGB 有相同 EOTF」。这句话本身成立（Display P3 与 sRGB 共用 sRGB 传输函数，只差 primaries），但它回答的是「SDR 主图该用哪个 EOTF」，**不是**「Apple 的 gain map 用哪个传输函数」——把 SDR 图的前提推广到 gain map 上，是一个未经论证的跳跃。Apple 文档对 gain map 明确写的是 Rec.709（A.2）。

#### A.6.2 `strukturag/libheif` —— 不解释 gain map

【实现】`strukturag/libheif` 主分支全仓库 843 个文件中，路径含 `gainmap` / `gain_map` / `hdrgain` 的为 **0 个**。公共辅助图头 `libheif/api/libheif/heif_aux_images.h` 只提供通用的 auxiliary image 访问 API，文件内不含 `gain` / `hdr` / `urn` 任何字样。

【推断】libheif 能**取出** Apple gain map（本仓库正是走这条路，`src/i2a/convert.rs:70-89`），但不解释其语义，也不做任何反变换；因此它不构成「第三种取法」，也就无法为传输函数之争提供证据。

#### A.6.3 `monogrid/gainmap-js` —— 只做 Adobe/ISO 一路

【实现】仓库 281 个文件中，含 `gainmap` 命名的源码全部属于 Adobe/ISO 一路；README 自述为 “A Javascript (TypeScript) Encoder/Decoder Implementation of Adobe's Gain Map Technology for storing HDR Images using an SDR Image + a "Gain Map"”。

【实现】其 JPEG 提取函数 `src/decode/shared/extract.ts` 只走 XMP + MPF，无 Apple URN 分支：

```ts
export const extractGainmapFromJPEG = async (jpegFile: Uint8Array<ArrayBuffer>) => {
  const metadata = extractXMP(jpegFile)
  if (!metadata) throw new XMPMetadataNotFoundError('Gain map XMP metadata not found')

  const mpfExtractor = new MPFExtractor({ extractFII: true, extractNonFII: true })
  ...
}
```

【实现】解码 shader（`src/decode/webgl/materials/GainMapDecoderMaterial.ts`）只实现 ISO 数学，gain map 像素的唯一处理是 `map_gamma` 反变换：

```glsl
vec3 logRecovery = pow( recovery, gamma );
...
vec3 hdrColor = (rgb + offsetSdr) * exp2( logBoost * weightFactor ) - offsetHdr;
```

（uniform 侧传入 `1.0 / gamma[i]`，对应 B.3 的 `log_recovery = pow(recovery, 1.0 / map_gamma)`。）

【推断】gainmap-js 没有 Apple gain map 分支，对本题无直接证据；但它说明「Adobe/ISO 阵营的实现只处理 `map_gamma`，从不假设 gain map 像素带有额外传输函数」这一惯例（与 B.1/B.2 的规范一致）。

#### A.6.4 `darktable` —— 完全没有 gain map

【实现】`darktable-org/darktable` 主分支 2360 个文件中，路径含 `gainmap` / `gain_map` / `hdrgain` 的为 **0 个**。`src/imageio/imageio_heif.c` 与 `src/imageio/imageio_avif.c` 中与 HDR/传递函数相关的代码只有 CICP 标注，例如：

```c
/* mistagged Rec. 709 AVIFs exported before dt 3.6 */
if(... && profile_info_nclx->matrix_coefficients == heif_matrix_coefficients_ITU_R_BT_709_5)
{
  /* must be actual Rec. 709 instead of 2.2 gamma*/
  cicp->transfer_characteristics = (uint16_t)heif_transfer_characteristic_ITU_R_BT_709_5;
}
```

【实现】这段是对「darktable 3.6 之前导出的 AVIF 把 Rec.709 误标成 gamma 2.2」的历史 bug 的修正，与 gain map 无关。darktable 既不解码也不应用 Apple gain map。

#### A.6.5 `ImageMagick` —— 全部委托 libultrahdr

【实现】`coders/uhdr.c`（2172 行）是 libultrahdr 的封装：所有 gain map 数学都通过 `uhdr_*` C API 完成，文件自身只处理 base/HDR 图的颜色传输选择（`UHDR_CT_LINEAR` / `UHDR_CT_SRGB` / PQ / HLG，见 `map_ct_to_uhdr_ct()`）与 hdrgm 属性的读写。全文件不含 `apple` / `HDRGainMap` 字样。

【实现】`coders/heic.c`（2770 行）中与 auxiliary image / gain map 相关的匹配为 **0 处**。

【推断】ImageMagick 既不解码 Apple gain map，也就没有传输函数选择；它的 HDR 能力完全等于 libultrahdr 的能力。

#### A.6.6 `AOMediaCodec/libavif` —— 有 Apple 分支，但不碰像素

【实现】`apps/shared/avifjpeg.c` 中唯一处理 Apple 格式的函数是 `avifJPEGParseGainMapXMPPropertiesAppleFormat()`（第 797-826 行），它只做**元数据换算**，原文：

```c
static avifBool avifJPEGParseGainMapXMPPropertiesAppleFormat(const xmlNode * descNode, avifGainMap * gainMap)
{
    double hdrHeadroomLinear = 1.0;
    AVIF_CHECK(avifJPEGFindGainMapPropertyDoubles(descNode, "HDRGainMapHeadroom", &hdrHeadroomLinear, /*numDoubles=*/1, XML_NAME_SPACE_APPLE_GAIN_MAP));
    if (hdrHeadroomLinear <= 0) {
        return AVIF_FALSE;
    }
    const double hdrHeadroom = log2(hdrHeadroomLinear);
    ...
    for (int i = 0; i < 3; ++i) {
        gainMap->gainMapMin[i].n = 0; // Min = 0 (log2)
        gainMap->gainMapMin[i].d = 1;
        gainMap->gainMapMax[i] = hdrHeadroomSFraction;
        gainMap->gainMapGamma[i].n = 1; // Gamma = 1.
        gainMap->gainMapGamma[i].d = 1;
        gainMap->baseOffset[i].n = 0; // Base offset = 0.
        gainMap->baseOffset[i].d = 1;
        gainMap->alternateOffset[i].n = 0; // Alternate offset = 0.
        gainMap->alternateOffset[i].d = 1;
    }
    gainMap->baseHdrHeadroom.n = 0; // Base headroom = 0 (SDR)
    gainMap->baseHdrHeadroom.d = 1;
    AVIF_CHECK(avifDoubleToUnsignedFraction(hdrHeadroom, &gainMap->alternateHdrHeadroom));
    return AVIF_TRUE;
}
```

【实现】XMP 里没有 headroom 时回落到 Exif（`avifGetExifAppleHeadroom()`，第 363-476 行），它读 MakerNote 33/48 并**逐字复刻** Apple 文档的分段公式（含四条系数），但**没有** Apple 公式里的 `max(stops, 0.0)` 钳制，且返回的是 `stops` 本身而非 `pow(2, stops)`：

```c
    *altHeadroom = stops;
```

【推断】把 `stops` 直接写进 log2 域的 `gainMapMax` / `alternateHdrHeadroom` 在数学上是对的（`log2(headroom) = stops`），但与 XMP 路径相比少了 `max(stops,0)` 钳制，负 stops 会得到负的 log2 headroom。这条差异未见 libavif 有说明或测试覆盖。

【实现】gain map 图像被**原样**赋给 `gainMap->image`，其间没有任何传递函数（`avifJPEGExtractGainMapImage()`，第 1124 行）：

```c
            gainMap->image = image;
            return AVIF_TRUE;
```

【实现】同文件第 1614-1616 行的 `AVIF_TRANSFER_CHARACTERISTICS_PQ` 只作用于 **alternate（HDR）图像的输出标注**，不是 gain map 像素的传递函数：

```c
            // Since jpeg doesn't provide this metadata, assume the values are the same as the base image
            // with a PQ transfer curve.
            gainMap->altColorPrimaries = avif->colorPrimaries;
            gainMap->altTransferCharacteristics = AVIF_TRANSFER_CHARACTERISTICS_PQ;
```

【推断】**libavif 对 Apple gain map 不做任何像素传递函数**：它把 Apple 的 8-bit gain map 像素直接当作 ISO 21496-1 的 recovery 值，并令 `gainMapMin = 0`、`gainMapMax = log2(headroom)`、`gamma = 1`、offset 全 0。这与本仓库的重参数化参数完全一致（`src/i2a/convert.rs:273-281`），**但本仓库在写入前先对像素做 sRGB 反变换再重算 recovery**（`src/i2a/convert.rs:107-122`），libavif 没有这一步。

【推断】两条路线的差别恰好落在 Apple 那句自相矛盾的话的两半上：

- 若 Apple 的 gain map 确实带传递函数编码（“encoded using the Rec.709 transfer function” 按字面成立），则 libavif 的 Apple→ISO 映射在数学上**不等价**于 Apple 的应用式，只是一个近似；本仓库多做的 sRGB 反变换方向是对的（但函数选错）。
- 若 Apple 的 gain map 是无传递函数编码的（“formatted as linear data” 按字面成立），则 libavif 是对的，本仓库多做的 sRGB 反变换**引入**了误差。

> **已变更（2026-10-16）**：本仓库已改用 **Rec.709 反变换**（见 A.8.1）。本节中「本仓库做 sRGB 反变换」「函数选错」等表述描述的是变更前的状态。

我没有找到 libavif 就这一选择给出的任何说明、注释或测试。**这正说明该问题必须靠实测判定，而不能靠读实现投票。**

### A.7 有没有人拿 Apple 的实际输出实测过

本节回答「Q4」。**结论先行：没有找到任何人做过「Rec.709 vs sRGB」的受控实测对比；只找到一条以 Apple 框架输出为基准的端到端定量对比，而它没有隔离传输函数这个变量。**

#### A.7.1 检索过的位置与结果（负结果也是结果）

| 位置 | 是否讨论 gain map 像素的传递函数 | 是否有实测 |
| --- | --- | --- |
| libavif PR #2960（加入 Apple gain map 支持，2026-01 合入） | 否 | 否 |
| libheif issue #1685（Apple HEIC gain map） | 否 | 否（评论者提到其仓库有 PSNR/体积结果，但那是端到端 PSNR，非传输函数隔离） |
| awxkee/gainforge issue #10 | 否 | 否 |
| m13253/heif-hdrgainmap-decode issue #3 “Wrong conversion for gain map” | 否 | 否 |
| sam-dumont/immich-memories issue #107 | 否（讨论 headroom） | 有，但测的是 headroom / nits |
| Apple 专利 US 2024/0153053 A1 | **否**（通篇未规定 gain map 像素的传递函数） | 否 |
| Apple Developer Forums（含 DTS 工程师回复的 #772498、#737012、#770437） | 未找到 | 未找到 |
| jackchou00 博客 | 是 | 有（见 A.7.2） |

【规范】Apple 专利 US 2024/0153053 A1 值得单独记一笔，因为它是 Apple 关于 gain map 生成的第一方技术披露。它的 gain map 值定义**只有除法与乘法**，没有任何编码层：

> dividing the value of each pixel of the multiple-channel SDR image 221 by the value of the corresponding pixel

> multiplying the pixel denoted “P1,1” of the multiple-channel HDR image 211 (having a value of “5”) by the pixel denoted “P1,1” of multiple-channel gain map 231

【实现】该专利全篇不含 `Rec.709` / `sRGB` / `BT.709` / `OETF` / `EOTF` / `gamma 2.2` 等任何传递函数字样，也没有描述播放端的线性化步骤。**它不能用来支持任何一侧。**（另需注意：该专利描述的是“多重增益图”的生成与调整，未必覆盖 2020 年那版单通道 Apple gain map 的编码。）

【实现】libavif PR #2960（“Add support for Apple gain maps (iPhone jpegs)”，2026-01-16 由 maryla-uc 合入）的描述只涉及 XMP 语法差异与从 Exif Maker Notes 补 headroom：

> These use a different XMP syntax on the gain map image, and do not carry XMP on the main image.

【实现】其评审记录里没有任何关于传输函数的讨论（唯一实质评论是 vrabaud 的 “Perfect !”）。**给 libavif 加上 Apple gain map 支持的人，也没有讨论过像素传递函数。**

【二手】m13253/heif-hdrgainmap-decode issue #3 是唯一一条标题直指「转换错误」的记录，但正文与回复都没有技术细节：维护者在 2024-12-14 回复称该项目启动时文档不足，“it is highly likely my conversion is wrong”，并把 README 指向 `johncf/apple-hdr-heic` 后关闭。**它既没说错在哪，也没说对的是哪个函数。**

#### A.7.2 唯一一条定量对比：jackchou00 的 CoreImage 基准实验

【二手】博客《HDR 图像格式解析（六）：iPhone HEIC 照片的解码与转换》（jackchou00.com，有中英双语版本）。这是目前能找到的唯一一条把「手写转换」与「Apple 框架输出」放在一起量的记录。

实验的两条管线（原文代码）：

- **Apple 侧**：`CIImage(contentsOf: fileURL, options: [.expandToHDR: true])`，再 `CIContext.createCGImage(hdrCIImage, from:, format: .RGBAh, colorSpace: CGColorSpace(name: .displayP3_PQ)!)`；
- **手写侧**：pillow-heif 提取 Baseline / Gainmap，`Exiftool` 取 headroom，按文档用 **Rec.709** 线性化 gain map、用 sRGB 线性化 SDR，再套 `hdr_rgb = sdr_rgb * (1.0 + (headroom - 1.0) * gainmap)`。

核心结论原文：

> 以 Core Image 的转换结果为基准，手动转换后的结果平均下来约有 1% 的差距，主要就是出现在高亮部分的截断上。

作者自己同时报告了三处**无法解释的偏差**，这几条比那个 1% 更重要：

> 也可以通过 Core Image 直接转换，按照 Apple 的说法，转换后的最大值不应该超过 Headroom，但 Core Image 的结果却显著超过，也许和之前提到的 Baseline Image 的范围有关。

> 如果使用 Core Image 提取的三个部分，再手动的进行转换，虽然高亮部分不会截断，但结果也会存在一些偏差，尚不清楚原因。

> 当 format 为 RGBA8 时，Core Image 解码后的图像范围在 0-255 之间，且大部分像素值与使用 pillow-heif 提取的图像一致，在高亮和暗部的地方会存在差异，也许是 Core Image 存在一些色调压缩的处理

【推断】该实验**不能判定传输函数**，三条理由：

1. 作者只做了一条手写管线（Rec.709），**没有 sRGB 对照组**，因此那个 1% 无法归因到传输函数上；
2. 他自报的两处无法解释的偏差（CoreImage 输出超出 headroom、用 CoreImage 取数再手动转换仍有偏差）说明 CoreImage 端还叠加了别的处理，**基准本身不干净**；
3. 差异「主要出现在高亮部分的截断」说明主导项是钳制，而非中段线性化。

【推断】不过有一点值得记录：如果 Apple 的 gain map 实际是 sRGB 编码，而手写管线按 Rec.709 线性化，中段误差约 20%（见 A.2 的对比），经 `1 + (hr-1)*L` 传到 HDR 后仍有约 4%–7%（取 hr = 2–3、L ≈ 0.16–0.20，即 immich issue #107 观测到的典型 gain map 均值附近）。这与作者观察到的「平均约 1%、且集中在高亮截断」不太吻合。**这是一条弱到中等强度的旁证，方向偏向 Rec.709 正确**；但受上述三条方法学问题限制，**不能当作定论**。

#### A.7.3 一条常被引用的旁证其实不适用：`x^1.961`

【规范】Apple 论坛帖 “Why does Apple use simplified 1.961 gamma instead of precise ITU-R 709 transfer function?” 与 Apple 文档 *Evaluating an app's video color using light-measurement Instruments* 讨论的是 **Core Video 色彩配置与 macOS 显示系统**用 `f(x) = x^1.961` 近似 BT.709 传输函数：

> The system uses a 1.961 gamma based on the assumption of a bright surround environment.

【推断】**这条与 gain map 无关**：它解释的是视频/显示链路的色彩配置近似（`CVImageBufferCreateColorSpaceFromAttachments` 生成的自定义 profile、`vImageBuffer_CopyToCVPixelBuffer` 的 Core Video 兼容行为），不是 Apple gain map 编码器实际用的函数。把它拿来论证 gain map 传输函数是误用。**记在这里是为了防止后续调研再被它误导。**

#### A.7.4 与 headroom（而非传输函数）有关的实测

【二手】`sam-dumont/immich-memories` issue #107（2026-03-21 创建，同日经 PR #113 修复，commit `6f01a52`）。作者报告 HDR gain map 照片在他的管线里「比原 HEIC 更暗、更不饱和」，根因是 Apple 路径把 headroom 硬编码成 2.3（`animator.py:238`：`headroom = 2.3`）。给出的实测数据：

- 测试照片：iPhone 16 Pro，5712×4284；
- **gain map 取值范围 [83, 255]，均值 111.6**；
- 由机型推断的 headroom：iPhone 14 Pro ≈ 3.0、iPhone 15/16 Pro ≈ 3.5–4.0；
- 多组 npl（203 / 500 / 999）下的均值 RGB 分别为 (38,35,31) / (56,52,46) / (74,69,61)，SDR 基线为 (119,115,102)，其照片管线实际输出为 (151,146,133)。

【实现】该 issue 与本题正交：它测的是 headroom 缺失，不是 gain map 像素的传递函数。但它给出两条对后续实测有用的信息：

1. **真实照片的 gain map 值域可以很窄且偏高**（最小 83 而非 0），说明实测样本必须覆盖足够宽的 gain map 取值，否则两个候选函数可能都落在量化噪声内而无法区分；
2. Apple 的 headroom 不在 XMP / gain map 元数据里（作者称已核实），需从 Exif MakerNote 或 ISO 21496-1 取——与本仓库 `src/i2a/convert.rs:28-67` 的读取优先级一致。

【二手】另有一份 headroom 提取方法的三方对比（jackchou00 博客）：以 Core Image 的 `contentHeadroom` 为基准，`XMP:HDRGainMapHeadroom` 「与之最接近（但会少几位有效数字）」，Apple 文档推荐的 MakerNote 33/48 公式「有一点点偏差」。该文未给出具体数值。

#### A.7.5 一条对后续实测极重要的旁证：Apple 的 API 似乎不做线性化

【二手】同一条博客在比较 gain map 提取方式时写道（原语言）：

> 比如 Python 中，可以用 pillow-heif 提取，结果与 Core Image 得到的是**完全一致**的。

其 Core Image 侧代码把 gain map 渲染为 `format: .RGBA8`、`colorSpace: extendedLinearDisplayP3`，pillow-heif 侧则直接读 `urn:com:apple:photo:2020:aux:hdrgainmap` 辅助图的原始字节。

【推断】若 CoreImage 在取出 gain map 时应用了任何传递函数，渲染成 8-bit 后不可能与文件里的原始码值逐字节一致。因此这条旁证指向：**CoreImage 的 `.auxiliaryHDRGainMap` 返回的就是未线性化的编码域像素，与 Apple 文档要求的「由调用方线性化」一致**。这同时回答了 A.5.4 留下的问题（方向为「返回编码域」），但**这是第三方博客的一句断言，没有附逐字节证据**，只能算【二手】，必须用 A.8 的步骤 2 自行复核。

### A.8 结论：这件事能否靠读一手资料定论

**不能定论。** 按三个不同命题分开陈述（这正是 issue #1 困境的根源——把 (a) 当成了 (b)）：

**(a) Apple 文档建议开发者怎么做** —— **有明确答案，无歧义。** 用 **Rec.709 传输函数的反函数**线性化 gain map，用图像自身色彩空间的传递函数线性化 SDR 图，再套 `hdr = sdr * (1 + (headroom-1) * gainmap)`（A.2）。这是【规范】级结论。

**(b) Apple 自己的编解码器/渲染器实际怎么做** —— **无一手资料可查，且现有证据互相牵制。**

- Apple 从未说明 `CGImageSourceCopyAuxiliaryDataInfoAtIndex(..., kCGImageAuxiliaryDataTypeHDRGainMap)` 或 CoreImage 取出的像素是否已被线性化（A.5.2、A.5.4）；
- Apple 专利 US 2024/0153053 A1 通篇不含传递函数（A.7.1）；
- WWDC24 session 10177 只讲 API 用法，不讲像素编码（A.5.3）；
- 唯一一条旁证（A.7.5）指向「API 返回编码域像素、不代做线性化」，但只是第三方的一句断言；
- 而**编码器当时究竟用了 Rec.709 还是 sRGB，至今没有任何一手来源**。

**(c) 各独立实现怎么做** —— **三个实现给出三个不同答案，且没有一个是实测得出的**：

| 实现 | gain map 像素用什么 | 依据 |
| --- | --- | --- |
| `johncf/apple-hdr-heic` | sRGB EOTF | 自定（注释只说 “Display P3 and sRGB have the same EOTF”） |
| `jackchou00/hdr-conversion` | Rec.709 | 照抄 Apple 文档，README 自称「可能不正确」 |
| `AOMediaCodec/libavif` | 不做任何传递函数 | 未说明理由，无测试 |

其余实现（libheif / darktable / ImageMagick / gainmap-js）根本不处理 Apple gain map（A.6）。**所以「多数实现怎么做」这个问题没有意义——不存在多数。**

#### 实测参照最可行的一条路径

**首选：以「gain map 平面本身」为参照，而不是以「Apple 渲染出的 HDR 图」为参照。**

理由：A.7.2 的实测显示 CoreImage 的最终 HDR 输出叠加了额外处理（超出 headroom、用 Apple 取数再手动转换仍有无法解释的偏差），不适合做数学基准；而 gain map 平面本身在 CoreImage 与 pillow-heif 之间被报告为逐像素一致，干净得多。

具体步骤（全部需要 macOS 11+ 即可，**不需要 macOS 15+**；`macos-26` 满足）：

1. **平台**：`macos-26` runner（`.github/workflows/release.yml:29`），无需显示器。
2. **取 Apple 侧 gain map 原始字节**：`CGImageSourceCopyAuxiliaryDataInfoAtIndex(src, 0, kCGImageAuxiliaryDataTypeHDRGainMap)`（macOS 11.0+，纯 ImageIO，不碰 Metal）。把它与仓库自己用 libheif 解出的 gain map 平面**逐字节比对**——这一步直接回答「Apple 的 API 是否已线性化」，且完全不依赖 GPU。
3. **取 Apple 侧 HDR 参照值**：`CGImageSourceCreateImageAtIndex` 配 `[kCGImageSourceDecodeRequest: kCGImageSourceDecodeToHDR]`（macOS 14.0+），画进 `CGColorSpace.extendedLinearDisplayP3` 的浮点 `CGContext`（纯 CPU）。**建议同时把 `kCGImageSourceGenerateImageSpecificLumaScaling` 置为 `kCFBooleanFalse`**，以关掉 ImageIO 基于 gain map 生成的全局 tone mapping（A.5.5），否则测到的是 Apple 的 tone mapping 而不是 gain map 数学。
4. **拟合**：对同一批像素，分别用 Rec.709 反函数与 sRGB 反函数算出 `hdr = sdr * (1 + (hr-1)*L)` 的预测值，与步骤 3 的参照值做残差比较。样本要挑 gain map 取值覆盖中段的区域（A.7.4 提醒真实照片 gain map 值域可能窄且偏高，中段样本是关键）。
5. **次选（仅当步骤 3 的 CoreGraphics 路径不可用时）**：CoreImage `applyingGainMap(_:headroom:)`（macOS 15.0+，显式接收 headroom，比 `applyingGainMap(_:)` 可控）。但必须处理两个坑：gain map `CIImage` 必须带 `properties`，否则**静默返回原图**（A.5.3）；以及虚拟化 runner 上 Metal 可能不可用（A.5.5）。

**必须靠实测才能回答的问题**（读一手资料解决不了）：

1. Apple 的 gain map 编码器实际用的是 Rec.709、sRGB，还是别的函数？
2. `CGImageSourceCopyAuxiliaryDataInfoAtIndex` / CoreImage `.auxiliaryHDRGainMap` 返回的到底是不是未线性化的原始像素？（A.7.5 倾向「是」，需逐字节确认）
3. CoreImage 的最终 HDR 渲染除 gain map 数学外还叠加了什么，使得结果超出 headroom？
4. 两个候选函数在**真实 iPhone 照片**上的端到端差异有多大，是否落在 8-bit 量化噪声之内（即：这个差异对最终产物是否真的可见）？

【推断】第 4 条决定了这件事值不值得改：如果差异落在量化噪声内，issue #1 的「记录而非解决」就是正确的工程决策；如果不在，则需要按实测结果切换函数。

#### A.8.1 第 4 条已实测（2026-10-16）

【实现】在本仓库代码里对两条路各算一遍并量化到 8-bit，语料为 `iCloud 照片/` 跨库均匀抽样的 60 张真实照片（每 8 张取 1 张，避免集中在同一次拍摄）：

| 指标 | 结果 |
| --- | --- |
| 有差异的像素比例 | 平均 95.24%（最低 7.61%，最高 100%） |
| max_delta | 平均 15.2 LSB，最高 17 LSB |
| mean_delta | 平均 11.49 LSB |
| gain 域单张最大差 | 平均 0.1012 stop，全局最大 0.1617 stop |
| gain 域单张平均差 | 平均 0.0802 stop |

【实现】结论分两层，两者并不矛盾：

1. **数值上差异真实。** 8-bit 舍入的量化噪声量级是 ±0.5 LSB，实测平均 11–15 LSB，是噪声的 20–30 倍。第 4 条设想的「落在量化噪声内」不成立。
2. **感知上差异不可见。** `log_recovery → pixel_gain` 是指数映射，把差异大幅压缩：显示域平均差 3–5 级、峰值 9–11 级（满量程 255），即约 1.5% 平均、4% 峰值亮度偏移，合 0.08–0.16 stop。在 headroom 最高的 4 张（5.46–5.84）上做并排对比，HDR 显示设备上分不出。

【实现】差异是**单向**的：在 u ∈ (0, 1) 上恒有 `r709(u) > sRGB(u)`（仅两个端点相等），实测在显示域亦然——Rec.709 恒不低于 sRGB。它不是噪声，而是系统性偏移。

> **两个统计量不可混读。** 上表「有差异的像素比例（最低 7.61%）」是**量化域**统计：60 张全样本、gain map 量化值相差 ≥ 1 LSB 的像素占比。而下句「Rec.709 更亮 85–96%、sRGB 更亮约 0%」是**显示域**统计：仅限用于并排对比的那 4 张 headroom 最高的照片（5.46–5.84），重建 HDR 后亮度差 > 0.5 级。两者量纲不同、样本也不同，不可直接比较。

> **可复现性**：本次测量的代码是一次性实验补丁（环境变量切换两个函数并统计 LSB 直方图），未随主线保留，故上表数字无法从当前 `src/` 直接复算。原始日志与语料清单在 `testoutput/exp/`（gitignored，不随仓库分发）。若需重跑，见 A.8.1 末尾的重建说明。

【推断】第 4 条给出的决策依据因此失效——差异既不落在量化噪声内（「记录而非解决」的理由不成立），也看不见（「按实测结果切换函数」没有可感知的收益）。2026-10-16 的决定是：**改用 Rec.709**。理由是它是 Apple 文档写明的行为，改后代码与文档不再自相矛盾；风险为零，因为两个候选的产物在视觉上不可区分。落地时以单元测试钉住期望值（`src/i2a/convert.rs` 的 `rec709_inverse_is_pinned_at_representative_points`、`rec709_inverse_is_linear_below_the_breakpoint`、`rec709_inverse_is_not_the_srgb_one`）。

【推断】第 1 条（Apple 编码器实际用哪个函数）**仍未解决**，且上述实测无法解决它——实测只说明两个候选差多少，不说明哪个对。鉴于差异不可见，未再投入 macOS 参照实验（A.8 的步骤 2–5 未执行）。

【实现】副作用：本仓库输出的 HDR 在 iOS 上能正确渲染（iPhone 上实测有 HDR 效果），这为 `README.md` §6 里「输出只有 ISO 21496-1 APP2、没有 `hdrgm` XMP」那条限制补了一个 Android 之外的正面数据点。

#### A.8.1.1 如何重跑这次测量

实验补丁未随主线保留（它只是两个函数各算一遍再相减）。重建步骤：

1. 在 `src/i2a/convert.rs` 里加一个 `reverse_rec709_transform`：`u < 0.081 ? u / 4.5 : ((u + 0.099) / 1.099).powf(1.0 / 0.45)`，保留原有 `reverse_srgb_transform`；
2. 在 `create_gainmap_jpg` 的内层循环里对每个像素分别用两个函数算出 `encoded_recovery`，取绝对差累计成 256 桶直方图（即上表的量化域数字）；
3. 同时累计 `gain = 1 + (hr - 1) * L` 两路之比，取 `log2(gain_a / gain_b).abs()` 的均值与最大值（即 stop 域数字）；
4. 统计打到 stderr，语料用 `iCloud 照片/` 每 8 张取 1 张（清单见 `testoutput/exp/corpus-list.txt`），加 `-j` 并行。

注意口径不同：`src/i2a/convert.rs` 里的单元测试钉的是**函数本身**的期望值，用于防回归；这次测量回答的是**差异有多大**，两者不可互相替代。

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
- [A-10] Apple Developer, WWDC24 session 10177, *Use HDR for dynamic image experiences in your app* — https://developer.apple.com/videos/play/wwdc2024/10177/ （iOS 18 转向 Adaptive HDR / ISO gain map 的官方说明、`auxiliaryHDRGainMap` 与 `kCGImageSourceDecodeToHDR` 用法、`kCGImageAuxiliaryDataTypeISOGainMap` 写入路径）
- [A-11] Apple Developer Documentation, `CIImage.applyingGainMap(_:)` — https://developer.apple.com/documentation/coreimage/ciimage/applyinggainmap(_:) （gain map 的 `properties` 缺失时静默返回原图）
- [A-12] Apple Developer Documentation, `CIImage.applyingGainMap(_:headroom:)` — https://developer.apple.com/documentation/coreimage/ciimage/applyinggainmap(_:headroom:)
- [A-13] Apple Developer Documentation, `CIImageOption.auxiliaryHDRGainMap` / `expandToHDR` / `kCGImageSourceDecodeRequest` / `kCGImageSourceDecodeToHDR` / `CIContextOption.workingColorSpace` / `useSoftwareRenderer` / `kCGImageDestinationPreserveGainMap` 各页（可用性与原文均取自 developer.apple.com 的文档 JSON 与 `.md` 端点；`workingColorSpace` 页给出 “The default working space is the extended sRGB color space with linear gamma.”）
- [A-14] Apple SDK 头文件 `ImageIO.framework/Headers/CGImageSource.h`（iPhoneOS18.0 SDK）— 辅助数据键与 `kCGImageSourceDecodeRequest` / `kCGImageSourceDecodeToHDR` / `kCGImageSourceGenerateImageSpecificLumaScaling` 的注释原文
- [A-15] WebKit, `Source/WebCore/platform/graphics/cg/ImageDecoderCG.cpp`、`Source/WebCore/platform/graphics/GainMap.h` — https://github.com/WebKit/WebKit （`ImageDecoderCG::frameGainMapAtIndex()` 对两个 gain map 常量的实际调用顺序与未公开键 `kCGImageAuxiliaryDataRepresentationPixelBuffer`）
- [A-16] johncf/apple-hdr-heic — `src/apple_hdr_heic/lib.py`（sRGB EOTF 线性化 gain map）— https://github.com/johncf/apple-hdr-heic
- [A-17] colour-science, `colour/models/rgb/transfer_functions/srgb.py`（`eotf_sRGB` 定义）— https://github.com/colour-science/colour
- [A-18] Jackchou00/hdr-conversion — `src/hdrconv/convert/apple.py`（Rec.709 线性化 gain map）、`README.md` — https://github.com/jackchou00/hdr-conversion
- [A-19] jackchou00, *HDR 图像格式解析（六）：iPhone HEIC 照片的解码与转换*（中英双语）— https://jackchou00.com/posts/iphone-heic-hdr-format/ ；英文版 https://jackchou00.com/en/posts/iphone-heic-hdr-format/ （CoreImage 基准实验、gain map 提取一致性、headroom 三方对比）
- [A-20] AOMediaCodec/libavif PR #2960, *Add support for Apple gain maps (iPhone jpegs)* — https://github.com/AOMediaCodec/libavif/pull/2960
- [A-21] sam-dumont/immich-memories issue #107, *fix: HDR gain map photos appear dim/desaturated compared to original HEIC* — https://github.com/sam-dumont/immich-memories/issues/107
- [A-22] Apple 专利 US 2024/0153053 A1, *Techniques for Generating Gain Maps Based on Acquired Images* — https://patents.google.com/patent/US20240153053A1/en （全文未规定 gain map 像素的传递函数）
- [A-23] Apple Developer Forums thread 772498, *How to access HDRGainMap from AVCapturePhoto*（DTS 工程师回复，给出 `CIImage(data:options:[.auxiliaryHDRGainMap: true])` 写法）— https://developer.apple.com/forums/thread/772498
- [A-24] Apple Developer Forums thread 113337, *Why does Apple use simplified 1.961 gamma instead of precise ITU-R 709 transfer function?* — https://developer.apple.com/forums/thread/113337 ；Apple, *Evaluating an app's video color using light-measurement Instruments* — https://developer.apple.com/documentation/avfoundation/evaluating-an-app-s-video-color-using-light-measurement-instruments （两者均属 Core Video/显示链路，与 gain map 无关）
- [A-25] m13253/heif-hdrgainmap-decode issue #3, *Wrong conversion for gain map* — https://github.com/m13253/heif-hdrgainmap-decode/issues/3 （维护者承认转换很可能有误，但未说明错在哪）

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

### B.8 ISO 21496-1 是否规定了 gain map 像素的传递函数

本节回答「Q2」。**结论：我没有拿到标准原文，因此无法以原文回答；但所有免费可得的一手替代都指向同一结论——这一系格式在 gain map 像素上只有 `map_gamma` 一个自由度，不存在「像素传递函数」这个概念。**

#### B.8.1 原文获取情况（如实记录）

- 【规范】ISO 目录页（[B-2]）只提供 abstract 与生命周期，无正文条款（见 B.5）。
- ISO Online Browsing Platform 的预览地址 `https://www.iso.org/obp/ui/en/#iso:std:iso:ts:21496:-1:ed-1:v1:en` 返回 **HTTP 403 Forbidden**，我未能读到任何目录、范围条款或术语定义。
- 【二手】因此，「ISO 21496-1 正文是否规定了 gain map 像素的传递函数」**这个问题本身悬而未决**，本节以下全部内容都只能算**免费可得的一手替代**（Android 规范）与**实现**（库源码），**不等同于标准原文**。

#### B.8.2 免费可得的第一方替代：Android Ultra HDR v1.1

【规范】Android Ultra HDR v1.1 与 ISO 21496-1 的关系由规范自己在 Change Log 里声明（B.1）：

> v1.1 — All changes in this version of the Ultra HDR specification are informational and about ISO 21496-1 compatibility. There are no changes to the actual file format.

【规范】该规范对 gain map 像素的规定只有一条——**直接量化 recovery 值，不施加任何传递函数**：

> The gain map is stored in a secondary image JPEG, and therefore must be encoded using 8-bit, unsigned integer values, thus in the range [0, 255]. Each value represents a `recovery(x, y)` value and is stored in one pixel of the secondary image.

> `encoded_recovery(x, y) = floor(recovery(x, y) * 255.0 + 0.5)`

【规范】`map_gamma` 是**唯一**作用于 gain map 存储值的整形参数：

> `map_gamma` is a floating point number that must be greater than 0.0 and is chosen by the implementation.

> `recovery(x, y) = pow(clamped_recovery(x, y), map_gamma)`

【规范】规范明确声明 gain map 自带的色彩信息不被使用：

> If present, the gain map's color profile isn't used.

【规范】规范中出现的 “transfer function” / “sRGB” 全部指向**主图（SDR）**，与 gain map 无关：

> `SDR(x, y)` is the linear version of the three-channel primary image, obtained by transforming to a linear version of the primary image color space. For example, from a color space with a sRGB transfer function to a linear color space that preserves sRGB color primaries.

> `SDR'(x, y)` is the three-channel, non-linear (typically gamma-encoded) primary image.

【实现】该规范全页**不含 “Rec.709” 字样**，也没有任何一句说 gain map 像素是 gamma 编码的。

【推断】**Android Ultra HDR 规范对 gain map 像素的处理是「把 recovery 值直接量化成 8-bit」，即隐含假设 gain map 像素不带额外的传输函数编码。** 这与 Apple 文档「gain map 用 Rec.709 传输函数编码」（A.2）形成正面冲突——**这正是本仓库 issue #1 的根源**：两个格式对「gain map 像素该带不带传递函数」的前提不同，转换时必须在某处消解掉这个差异。

#### B.8.3 其他免费一手旁证（实现侧）

- 【实现】libultrahdr v1.4.0（[B-5]）：`lib/src/gainmapmath.cpp` 的 `encodeGain()` 是 `powf(gain_normalized, metadata->gamma[index]) * 255.0f`，解码侧只在 `metadata->gamma != 1.0f` 时做 `pow(gain.r, 1.0f / metadata->gamma)`。**除 `map_gamma` 外没有任何传递函数。**
- 【实现】FFmpeg（[B-6]）：`libavutil/gain_map.h` 中与 gain map 存储值相关的字段说明只有一句——“Encoding gamma of the stored map values. Must be > 0.”。没有传输函数概念。
- 【实现】libavif（A.6.6）：对 Apple gain map 也只写 `gainMapGamma = 1`，不做传递函数。
- 【二手】Adobe 的 Gain Map 规范（[C-2]）同为 `map_gamma`（`Gamma` 字段）单一自由度，未见像素传递函数条款（C.2、C.3）。

【推断】综合看，**ISO 21496-1 这一系的格式在 gain map 像素上只有 `map_gamma` 一个自由度，没有「传输函数」这个概念**。因此把 Apple 的 gain map 转成 ISO 21496-1 时，Apple 侧的传输函数必须在转换时被「吃掉」，只有两条路：

1. **先把像素线性化，再重算 recovery**（本仓库的做法，`src/i2a/convert.rs:107-122`）；
2. **直接搬运像素**（libavif 的做法，`apps/shared/avifjpeg.c`，见 A.6.6）。

这个二选一正是问题的实质。但必须强调：**「ISO 21496-1 原文是否真的如此」我无法用原文确认**，故本段整体为【推断】，其可靠性完全依赖 Android 规范与实现侧的一致程度。

### B 节引用清单

- [B-1] Android Developers, *Ultra HDR Image Format v1.1* — https://developer.android.com/media/platform/hdr-image-format （Encode / Gain map generation、Gain map container、Decode、Display、ISO 21496-1 Compatibility、Change Log 各节；页脚 Last updated 2024-10-25）
- [B-2] ISO, *ISO 21496-1:2025 — Digital photography — Gain map metadata for image conversion — Part 1: Dynamic range conversion* — https://www.iso.org/standard/86775.html
- [B-3] IETF RFC 5141, *A Uniform Resource Name (URN) Namespace for the International Organization for Standardization (ISO)* — https://www.rfc-editor.org/rfc/rfc5141.txt （§2.4.2 示例、Appendix B ABNF）
- [B-4] ISO/TC 42/WG 23, *ICC and ISO 21496-1*（ICC Tokyo HDR Expert Day 材料）— https://www.color.org/events/hdr/tokyo2024_expert_day/ISO-TC42_WG_23_N7088_ICC_and_ISO_21496-1.pdf
- [B-5] google/libultrahdr **v1.4.0** 源码：`ultrahdr_api.h`、`lib/include/ultrahdr/gainmapmetadata.h`、`lib/include/ultrahdr/ultrahdrcommon.h`、`lib/src/gainmapmetadata.cpp`、`lib/src/gainmapmath.cpp`、`lib/src/jpegrutils.cpp`、`lib/src/jpegdecoderhelper.cpp` — https://github.com/google/libultrahdr/tree/v1.4.0
- [B-6] FFmpeg 源码：`libavutil/gain_map.h`、`libavcodec/gain_map.h` — https://github.com/FFmpeg/FFmpeg/blob/master/libavutil/gain_map.h ；文档页 https://ffmpeg.org/doxygen/trunk/libavutil_2gain__map_8h.html
- [B-7] Android Developers, `android.graphics.Gainmap` API reference — https://developer.android.com/reference/android/graphics/Gainmap
- [B-8] 本仓库代码与产物：`src/i2a/convert.rs`、`scripts/linux-build-libuhdr.sh`、`testoutput/MVIMG_3853.jpg`（字节偏移 8808 / 3604202 处的 APP2 段，实测）
- [B-9] ISO Online Browsing Platform 预览页 `https://www.iso.org/obp/ui/en/#iso:std:iso:ts:21496:-1:ed-1:v1:en` —— **本次抓取返回 HTTP 403，未能读到任何目录、范围条款或术语定义**（B.8.1 如实记录）
- [B-10] darktable / ImageMagick / monogrid/gainmap-js 三个仓库的源码树与关键文件（B.8 与 A.6 的实现侧核对）— https://github.com/darktable-org/darktable ；https://github.com/ImageMagick/ImageMagick （`coders/uhdr.c`、`coders/heic.c`）；https://github.com/monogrid/gainmap-js （`src/decode/webgl/materials/GainMapDecoderMaterial.ts`、`src/decode/shared/extract.ts`）

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
7. **Apple 的 gain map 编码器实际用的是哪个传输函数**（Rec.709 / sRGB / 其他）：Apple 文档只写「建议开发者怎么做」（A.2），Apple 专利 US 2024/0153053 A1 全文未规定 gain map 像素的传递函数（A.7.1），WWDC24 session 10177 只讲 API 用法（A.5.3）。**没有任何一手来源描述 Apple 编码器（命题 (b)）的实际做法**，详见 A.8。
8. **`CGImageSourceCopyAuxiliaryDataInfoAtIndex` / CoreImage `.auxiliaryHDRGainMap` 返回的像素是否已被线性化**：Apple 文档未说明（A.5.4）；唯一旁证是第三方博客称 CoreImage 提取结果与 pillow-heif 原始字节「完全一致」（A.7.5），属【二手】，需逐字节实测复核。
9. **CoreImage 的最终 HDR 渲染除 gain map 数学外还叠加了什么处理**：第三方实测报告其输出「显著超过 headroom」，且用 CoreImage 取数再手动转换仍有无法解释的偏差（A.7.2）。原因未知，直接影响「以 CoreImage 为参照」这条路线的可信度。
10. **ISO 21496-1 原文是否规定 gain map 像素的传递函数**：ISO OBP 预览页返回 HTTP 403（[B-9]），标准正文付费（[B-2]），**原文未获取**。B.8 的结论来自 Android 规范与实现侧的一致旁证，属【推断】，不等同于标准原文。
11. **`x^1.961` 与 Apple gain map 的关系**：经核实两者无关——`1.961` 属 Core Video 色彩配置与 macOS 显示链路的近似（[A-24]），不是 gain map 编码函数。记录于此以免后续被误引。
12. **是否存在未公开的「Rec.709 vs sRGB」受控实测**：本次检索未找到（A.7.1 列出检索位置）。若将来出现，应优先采信。
