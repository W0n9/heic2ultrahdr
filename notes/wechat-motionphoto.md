# 微信实况照片降级：根因与修复

2026-10-19 定位并修复。

## 现象

转换输出在微信发送时提示「无法预览此实况照片，若选择将作为静态图片发表」；
同一张图经小米相册编辑一次后即可正常发送。原图（HEIC + MOV 双文件）在 iPhone
上走 Apple 的 Live Photo 通道，本就正常，与 Android 单文件格式无关。

## 三方对比

样本：`debug/IMG_0489.jpg`（本工具转换输出）对 `debug/IMG_20261018_233837.jpg`
（小米相册编辑后的同一照片）。

| 维度 | 转换输出 | 小米编辑后 |
| --- | --- | --- |
| XMP | 仅旧版 `GCamera:MicroVideo*` | 规范形态：`GCamera:MotionPhoto=1` + `Container:Directory`（Primary image/jpeg + MotionPhoto video/mp4，`Item:Length`=视频字节数，`Padding=0`） |
| 内嵌视频 | Lavf remux 的 isom/mp41，moov 后置 | mp42，faststart（moov 紧跟 ftyp） |
| 视频编码 | HEVC | HEVC（编码不是差异点） |
| UltraHDR | MPF 双图 + ISO 21496 APP2 | 同样保留 |
| 小米标记 | `Exif 0x8897=1` | 同样有，另多一个 APP4 `XIAOMI_CUSTOMIZE` 编辑戳记（与识别无关） |

## 根因

1. 主因：Android Motion Photo 1.0 规范删除了 MicroVideo 属性并要求读取方忽略，
   视频定位改由 `Container:Directory` 的 `Item:Length` 承担。微信按现行规范实现，
   只写 MicroVideo 的文件能被判为实况候选却定位不到视频 → 降级提示。
2. 次因：内嵌视频非 faststart；音频为 AAC 时旧实现甚至原样追加 QuickTime MOV
   （`ftyp qt  `）。

## 修复（已落地）

- `src/i2a/xmp.rs`：直接生成 XMP 包并做 APP1 段级写入（exiftool 12.76 对
  MotionPhoto/Container 标签只读不写，写不了）。初版曾双写 MicroVideo 以兼容
  旧版相册，随后按项目决策（2026-10-19）移除，只写现行 Motion Photo 1.0 形态。
- `src/i2a/video.rs`：新增 `RemuxRequest`，所有视频统一流拷贝重封装为
  faststart mp42；音频转码路径同样带 faststart。
- 实测确认：MPF MP Entry 偏移以 MPF 段内 TIFF 头为基准，XMP 段大小变化时
  无需修正（段与增益图同步平移）。

## 验证

- 手工修补样本 `debug/IMG_0489-motionphoto.jpg`（双写形态）：小米相册识别为
  动态照片、微信发送不降级（用户实机实测，2026-10-19）。
- 新管线产物 `debug/IMG_0001-motionphoto-v1.1.jpg`（仅 Motion Photo 1.0，
  不含 MicroVideo）：小米相册识别为动态照片、微信发送不降级（用户实机实测，
  2026-10-19）——仅新格式形态实锤可用。
- 集成测试 `tests/merge.rs` 钉住：MotionPhoto 标记、Container 目录、
  不写 MicroVideo、ftyp mp42、moov 前置。

## 边界

单文件 Android 动态照片无法成为 iOS 系统级 Live Photo（iOS 要求 HEIC+MOV
双文件配对并注册进相册）；微信内的跨端播放由微信自己处理，发送端格式合规即可。
