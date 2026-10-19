use anyhow::Context;
use std::path::PathBuf;

mod convert;
mod merge;
mod utils;
pub mod video;
mod xmp;

#[derive(Debug)]
pub struct ConvertRequest {
    pub image_path: PathBuf,
    /// 同名视频。None 表示这是一张静态照片，没有动态照片视频要追加。
    pub video_path: Option<PathBuf>,
    pub output_path: PathBuf,
    // 用 exiftool 实现 exif 写入
    pub exiftool_path: Option<PathBuf>,

    pub overwrite_existing: bool,

    /// [0, 100]
    pub image_quality: i32,
    /// [0, 100]
    pub gainmap_quality: i32,
}

impl ConvertRequest {
    /// Input and output is same file
    pub fn io_same_file(&self) -> bool {
        self.image_path.as_os_str().eq_ignore_ascii_case(self.output_path.as_os_str())
    }

    fn is_input_heic(&self) -> anyhow::Result<bool> {
        let ans = self.image_extension()?.eq_ignore_ascii_case("heic");
        Ok(ans)
    }

    pub fn convert(&self) -> anyhow::Result<()> {
        let video = self
            .video_path
            .as_deref()
            .map_or_else(|| "<static photo>".to_string(), |p| p.display().to_string());
        debug!(
            "Running convert request {} + {} => {}",
            self.image_path.display(),
            video,
            self.output_path.display(),
        );

        self.check_valid().context("request arguments invalid")?;
        let t = std::time::Instant::now();

        // 1. convert image（基础图；元数据统一在末尾用一次 exiftool 写入）
        let mut guard = self.make_hdr()?;

        // 2. 重封装动态照片视频为 faststart MP4（静态照片为 None）。视频在元数据
        //    写完后才追加：两个偏移字段以文件尾为基准，字节数此时已知；且 exiftool
        //    与 XMP 段写入只作用于纯图片，不用整重写带视频尾部的大文件。
        let motion = self.prepare_motion_video()?;
        let _tmp_guard = motion.as_ref().map(|(p, _)| {
            let p = p.clone();
            utils::Guard::new(move || {
                std::fs::remove_file(&p).ok();
            })
        });

        // 3. 一次性写完全部元数据（原先 copy_meta + update_motion_photo_exif 会让
        //    exiftool 用 -overwrite_original 把整个输出再重写 1~2 遍）
        let video_size = motion.as_ref().map(|(_, size)| *size);
        self.write_output_metadata(&self.image_path, video_size)?;

        // 4. 视频追加到输出末尾
        if let Some((tmp_video, _)) = &motion {
            self.append_video(tmp_video)?;
        }
        self.sync_file_times(&self.image_path, &self.output_path)?;

        #[rustfmt::skip]
        let output_size = self.output_path.metadata().context("Output is gone")?.len() as f32 / 1024.0 / 1024.0;
        info!(
            "convert success in {:.2?}: {} + {} => {} (size={output_size:.2} MiB)",
            t.elapsed(),
            self.image_path.display(),
            video,
            self.output_path.display(),
        );

        guard.cancel();
        Ok(())
    }

    pub fn delete_original(&self) -> anyhow::Result<()> {
        if !self.io_same_file() {
            std::fs::remove_file(&self.image_path).context("delete original image failed")?;
        }
        if let Some(video_path) = self.video_path.as_deref() {
            std::fs::remove_file(video_path).context("delete original video failed")?;
        }
        Ok(())
    }

    fn exif_tool(&self) -> crate::utils::ExifTool {
        match self.exiftool_path.as_ref() {
            Some(path) => crate::utils::ExifTool::with_path(path.clone()),
            None => crate::utils::ExifTool::new(),
        }
    }

    fn make_hdr(&self) -> anyhow::Result<utils::Guard<impl FnOnce() + '_>> {
        let t = std::time::Instant::now();
        match self.is_input_heic()? {
            true => self.convert_heic_to_jpg()?,
            false => self.copy_image()?,
        }
        let guard = utils::Guard::new(|| {
            // in case rest failed, remove generated output
            std::fs::remove_file(&self.output_path).ok();
        });
        debug!("jpg ensured (with HDR effect), time={:?}", t.elapsed());
        Ok(guard)
    }

    /// 把动态照片视频重封装为 faststart MP4 临时文件，返回其路径与字节数
    /// （静态照片，或输出已是动态照片时返回 None）。视频由 convert() 在元数据
    /// 写完后追加到输出末尾；临时文件的清理由 convert() 的 guard 负责。
    ///
    /// 所有视频统一经 avformat 重封装为 faststart MP4（brand mp42）后再追加：
    /// 原样追加的 QuickTime MOV（`ftyp qt  `、moov 后置）会让按 Android
    /// Motion Photo 1.0 规范实现的读取方（微信等）判为无法预览。音频已是
    /// AAC/AC-3 或没有音轨时流拷贝，否则把音频转码为 AAC。
    #[instrument(skip_all)]
    fn prepare_motion_video(&self) -> anyhow::Result<Option<(std::path::PathBuf, u64)>> {
        // 静态照片：没有视频要追加。
        let Some(video_path) = self.video_path.as_deref() else {
            debug!("no video, treat as static photo");
            return Ok(None);
        };

        // heic 输出的基础图是刚编码出来的，不可能已经是 motion photo；
        // 只有非 heic 输入走拷贝路径时才有可能，检查留给那种情况。
        if !self.is_input_heic()? && self.output_is_motion_photo()? {
            warn!("Output is already a motion photo, skip append video");
            return Ok(None);
        }

        let audio_codec = video::VideoUtils::get_audio_codec(video_path)?;
        debug!(?audio_codec, "input video");

        let video_name = video_path.file_stem().context("parse video path filename failed")?;
        let tmp_video_name = format!("{}-remuxing.mp4", video_name.to_string_lossy());
        let tmp_video = video_path.with_file_name(tmp_video_name);
        anyhow::ensure!(!tmp_video.exists(), "tempfile exists");

        let result = match audio_codec.as_deref() {
            // 音频本就兼容或没有音轨：流拷贝重封装，不碰编码参数
            None | Some("aac") | Some("ac3") => info_span!("remux_faststart")
                .in_scope(|| {
                    video::RemuxRequest {
                        input: video_path,
                        output: &tmp_video,
                    }
                    .execute()
                })
                .context("remux video to faststart mp4 failed"),
            // Apple 用 pcm_s16le 编码实况照片音频，Android 播放器普遍不支持，转码为 AAC
            _ => info_span!("transcode_aac_audio")
                .in_scope(|| {
                    video::VideoAudioEncodeRequest {
                        input: video_path,
                        output: &tmp_video,
                        bit_rate: 128 << 10,
                        encoder: "aac",
                    }
                    .execute()
                })
                .context("convert video audio to aac failed"),
        };
        if result.is_err() {
            // 失败时临时文件可能不完整，就地清理；成功后由 convert() 的 guard 接管
            std::fs::remove_file(&tmp_video).ok();
        }
        result?;

        let size = tmp_video.metadata()?.len();
        Ok(Some((tmp_video, size)))
    }
}
