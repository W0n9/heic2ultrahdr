# Evaluation 测量记录

**测量日期**：2026-10-16
**构建**：`cargo build --release`（release profile，LTO + strip）
**硬件**：36 核 Linux 工作站

本文件是 README 中 Evaluation 章节的数据来源。数字全部来自一次全量实跑，
不是抽样估计。

## 语料

一份真实的 iCloud 照片导出，760 个文件、8.6 GB。

| 类别 | 数量 |
| --- | --- |
| HEIC 输入 | 508 |
| ├ 配对 MOV（Live Photo） | 226 |
| └ 无配对（静态照片） | 282 |
| MOV 输入 | 234 |
| └ 无配对图片 | 8 |
| 非 HEIC 图片（jpg / png） | 17 |
| 压缩包 | 1 |

## 吞吐

| 指标 | 数值 |
| --- | --- |
| 转换任务 | 508 |
| 成功 | 508（100%） |
| 失败 | 0 |
| 墙钟总耗时 | 118.2 s |
| 吞吐 | 4.30 张/秒 |
| 等效串行 | 0.233 s/张 |
| 输出总计 | 3.94 GiB |
| 平均输出 | 7.93 MiB/张 |

## 输出正确性

对全部 508 个输出逐一校验（直接读产物字节，不经由 exiftool 的解读）：

| 检查项 | 结果 |
| --- | --- |
| JPEG 容器有效 | 508 / 508 |
| 含 UltraHDR 增益图（ISO/IEC 21496-1 元数据） | 505 / 508 |
| 含 GCamera `MicroVideo` 标记 | 226 —— 与配对的 Live Photo 数精确一致 |
| MPF 双图结构（主图 + 增益图） | 抽样验证通过 |

**505/508 的解释**：3 个源文件本身不含 Apple HDR gain map。它们在文件里
`hdrgainmap` 出现 0 次（正常 Apple 照片出现 1 次），且没有 Make/Model，却带着
HEVC 的 content light level 元数据 —— 像是第三方工具生成的 HEIC。程序按设计
降级为普通 JPEG 输出（`src/i2a/convert.rs:245-249`），日志中对应 3 次
`not apple HDR, skip HDR`。**这是正确行为，不是失败。**

## 耗时分布

单任务耗时取自运行日志，**包含并行排队时间**，不是纯计算时间。

| 类型 | n | 中位数 | 平均 | P95 |
| --- | --- | --- | --- | --- |
| Live Photo | 226 | 8.06 s | 11.12 s | 22.2 s |
| 静态照片 | 282 | 3.01 s | 3.39 s | 6.33 s |

Live Photo 明显更贵：多了一次音频转码（Apple 的 pcm_s16le → AAC）和一次
整段视频追加。

## 观察

- **最慢任务**：`IMG_0066` 耗时 94.2 s、输出 233 MiB。其配对视频是 241 MB 的
  4K HEVC（3840×2160），远大于典型 Live Photo 视频（约 66 MB）。耗时与体积
  都来自这个非典型输入，程序行为正确。
- **大小写冲突**：语料中有 7 个 stem 同时存在 `IMG_00xx.HEIC` 与 `IMG_00xx.JPG`。
  程序按设计选 HEIC、把 JPG 记为 `DuplicateImage` 问题（日志中可见
  `Multiple images found with the same name`）。
- **非 HEIC 输入**：17 个 jpg/png 被 `NotHeic` 跳过，无输出 —— 符合预期。

## 复现方式

```bash
# 在隔离副本上跑，避免输出混进原始语料（原目录里有 7 个同名不同大小写的 JPG）
cp -a 'iCloud 照片' /tmp/corpus
cargo build --release
./target/release/heic2ultrahdr /tmp/corpus -o keep -j -v
```
