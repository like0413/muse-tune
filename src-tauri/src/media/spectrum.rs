//! 使用 Windows 按进程回环捕获生成当前播放器的真实音频频谱。

use std::{
    collections::VecDeque,
    ops::{Range, RangeInclusive},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use rustfft::{FftPlanner, num_complex::Complex};
use tauri::{AppHandle, Emitter, Runtime};
use wasapi::{AudioClient, Direction, SampleType, StreamMode, WaveFormat, initialize_mta};

use crate::error::Error;

pub(super) const MEDIA_SPECTRUM_CHANGED_EVENT: &str = "media://spectrum-changed";

const SAMPLE_RATE: u32 = 48_000;
const CHANNEL_COUNT: usize = 2;
const BYTES_PER_SAMPLE: usize = size_of::<f32>();
const FFT_SIZE: usize = 2_048;
const OUTPUT_BAND_COUNT: usize = 64;
const MIN_FREQUENCY_HZ: f32 = 45.0;
const MAX_FREQUENCY_HZ: f32 = 16_000.0;
const DEFAULT_FRAME_RATE: u16 = 20;
/// 频谱帧率的合法区间；越界属于参数非法，命令层据此判定不可重试。
pub(super) const SPECTRUM_FRAME_RATE_RANGE: RangeInclusive<u16> = 15..=30;
const STOP_CHECK_INTERVAL_MS: u32 = 100;
/// dB 换算成 0..1 归一化值的下界：低于它的频带直接落到 0。
const NOISE_FLOOR_DB: f32 = -72.0;
/// 归一化值的上界：达到它的频带输出满值。它与 [`NOISE_FLOOR_DB`] 一起决定映射区间，
/// 因此只影响画面动态范围而不改变采集本身：区间收窄会让更多频带顶到满格、画面更容易跳动，
/// 区间放宽则会让整幅频谱整体偏暗。
const PEAK_DB: f32 = -8.0;

/// 管理频谱开关、当前捕获进程以及唯一的捕获线程。
pub(super) struct AudioSpectrumController<R: Runtime> {
    app: AppHandle<R>,
    enabled: bool,
    process_id: Option<u32>,
    frame_rate: u16,
    worker: Option<SpectrumWorker>,
}

impl<R: Runtime> AudioSpectrumController<R> {
    /// 创建默认关闭、尚未绑定播放器的频谱控制器。
    pub(super) fn new(app: AppHandle<R>) -> Self {
        Self {
            app,
            enabled: false,
            process_id: None,
            frame_rate: DEFAULT_FRAME_RATE,
            worker: None,
        }
    }

    /// 切换频谱采集；关闭时立即释放 WASAPI 流并清空画面。
    pub(super) fn set_enabled(&mut self, enabled: bool, frame_rate: u16) -> Result<(), Error> {
        if !SPECTRUM_FRAME_RATE_RANGE.contains(&frame_rate) {
            return Err(Error::InvalidInput(format!(
                "频谱帧率必须在 {} 到 {} 之间",
                SPECTRUM_FRAME_RATE_RANGE.start(),
                SPECTRUM_FRAME_RATE_RANGE.end()
            )));
        }
        if self.enabled == enabled && self.frame_rate == frame_rate {
            return Ok(());
        }
        self.enabled = enabled;
        self.frame_rate = frame_rate;
        self.restart()
    }

    /// 绑定当前实际发声进程；播放器未创建音频会话时保持空闲。
    pub(super) fn bind(&mut self, process_id: Option<u32>) {
        if self.process_id == process_id {
            return;
        }
        self.process_id = process_id;
        if let Err(error) = self.restart() {
            log::warn!("切换播放器频谱捕获目标失败: {error}");
        }
    }

    /// 返回配置开关与捕获线程是否实际运行。
    pub(super) fn diagnostics(&self) -> (bool, bool) {
        (self.enabled, self.worker.is_some())
    }

    /// 停止旧目标并在需要时启动新目标，保证同时只有一个捕获流。
    fn restart(&mut self) -> Result<(), Error> {
        self.worker.take();
        emit_spectrum(&self.app, &zero_frame());

        let Some(process_id) = self.process_id.filter(|_| self.enabled) else {
            return Ok(());
        };
        self.worker = Some(SpectrumWorker::spawn(
            self.app.clone(),
            process_id,
            Duration::from_millis(1000 / self.frame_rate as u64),
        )?);
        Ok(())
    }
}

/// 捕获线程句柄；释放时通过原子信号结束事件等待并回收线程。
struct SpectrumWorker {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl SpectrumWorker {
    /// 为单个播放器进程创建按事件驱动的回环捕获线程。
    fn spawn<R: Runtime>(
        app: AppHandle<R>,
        process_id: u32,
        frame_interval: Duration,
    ) -> Result<Self, Error> {
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let thread = thread::Builder::new()
            .name("media-spectrum-capture".to_owned())
            .spawn(move || {
                if let Err(error) = capture_spectrum(&app, process_id, &worker_stop, frame_interval)
                {
                    log::warn!("捕获播放器音频频谱失败: {error}");
                    emit_spectrum(&app, &zero_frame());
                }
            })?;
        Ok(Self {
            stop,
            thread: Some(thread),
        })
    }
}

impl Drop for SpectrumWorker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            log::warn!("播放器频谱捕获线程异常退出");
        }
    }
}

/// 初始化按进程 WASAPI 流，并在音频事件到达时生成频谱帧。
fn capture_spectrum<R: Runtime>(
    app: &AppHandle<R>,
    process_id: u32,
    stop: &AtomicBool,
    frame_interval: Duration,
) -> Result<(), Error> {
    // COM 初始化返回的 HRESULT 来自 wasapi 内部的 windows 版本，只能保留其文案。
    initialize_mta()
        .ok()
        .map_err(|error| Error::Message(error.to_string()))?;

    let format = WaveFormat::new(
        32,
        32,
        &SampleType::Float,
        SAMPLE_RATE as usize,
        CHANNEL_COUNT,
        None,
    );
    // 第二个参数 true 表示把捕获范围扩到该进程的整棵子进程树；改成 false 时，音频由子进程
    // （渲染进程、独立音频宿主等）输出的播放器只会采到静默帧。
    let mut audio_client = AudioClient::new_application_loopback_client(process_id, true)?;
    audio_client.initialize_client(
        &format,
        &Direction::Capture,
        &StreamMode::EventsShared {
            autoconvert: true,
            buffer_duration_hns: 0,
        },
    )?;
    let audio_event = audio_client.set_get_eventhandle()?;
    let capture_client = audio_client.get_audiocaptureclient()?;
    audio_client.start_stream()?;

    let mut samples = VecDeque::with_capacity(FFT_SIZE);
    let mut packet = Vec::new();
    let mut analyzer = SpectrumAnalyzer::new();
    let mut last_frame_at = Instant::now() - frame_interval;

    while !stop.load(Ordering::Acquire) {
        // 本轮是否读到新音频包；没有新包时不再重复发射同一份陈旧频谱。
        let mut has_new_samples = false;
        while let Some(frame_count) = capture_client
            .get_next_packet_size()?
            .filter(|count| *count > 0)
        {
            let packet_size = frame_count as usize * CHANNEL_COUNT * BYTES_PER_SAMPLE;
            packet.resize(packet_size, 0);
            let (read_frames, info) = capture_client.read_from_device(&mut packet)?;
            append_mono_samples(
                &mut samples,
                &packet[..read_frames as usize * CHANNEL_COUNT * BYTES_PER_SAMPLE],
                info.flags.silent,
            );
            has_new_samples = true;
        }

        // 暂停或无音频输出期间 samples 仍保留旧数据，继续做 FFT 并发射只会让前端反复重绘同一帧。
        if has_new_samples && samples.len() == FFT_SIZE && last_frame_at.elapsed() >= frame_interval
        {
            emit_spectrum(app, analyzer.analyze(&samples));
            last_frame_at = Instant::now();
        }

        // WASAPI 音频到达事件驱动采集；短超时只用于响应线程停止，不查询媒体状态。
        let _ = audio_event.wait_for_event(STOP_CHECK_INTERVAL_MS);
    }

    let _ = audio_client.stop_stream();
    Ok(())
}

/// 将交错双声道 float32 数据折叠为单声道，并仅保留最新 FFT 窗口。
fn append_mono_samples(samples: &mut VecDeque<f32>, packet: &[u8], silent: bool) {
    let (frames, _) = packet.as_chunks::<{ CHANNEL_COUNT * BYTES_PER_SAMPLE }>();
    for frame in frames {
        let sample = if silent {
            0.0
        } else {
            let left = f32::from_ne_bytes(frame[..4].try_into().unwrap_or_default());
            let right = f32::from_ne_bytes(frame[4..8].try_into().unwrap_or_default());
            (left + right) * 0.5
        };
        if samples.len() == FFT_SIZE {
            samples.pop_front();
        }
        samples.push_back(sample);
    }
}

/// 复用 FFT 计划和工作缓冲区，避免每帧重新分配昂贵对象。
struct SpectrumAnalyzer {
    fft: Arc<dyn rustfft::Fft<f32>>,
    window: Vec<f32>,
    buffer: Vec<Complex<f32>>,
    band_ranges: Vec<Range<usize>>,
    smoothed: Vec<f32>,
}

impl SpectrumAnalyzer {
    /// 创建 Hann 窗与固定大小 FFT 计划。
    fn new() -> Self {
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);
        let window = (0..FFT_SIZE)
            .map(|index| {
                0.5 - 0.5
                    * (2.0 * std::f32::consts::PI * index as f32 / (FFT_SIZE - 1) as f32).cos()
            })
            .collect();
        let frequency_ratio = MAX_FREQUENCY_HZ / MIN_FREQUENCY_HZ;
        let band_ranges = (0..OUTPUT_BAND_COUNT)
            .map(|band| {
                let low =
                    MIN_FREQUENCY_HZ * frequency_ratio.powf(band as f32 / OUTPUT_BAND_COUNT as f32);
                let high = MIN_FREQUENCY_HZ
                    * frequency_ratio.powf((band + 1) as f32 / OUTPUT_BAND_COUNT as f32);
                let first_bin = frequency_to_bin(low);
                first_bin..frequency_to_bin(high).max(first_bin + 1).min(FFT_SIZE / 2)
            })
            .collect();
        Self {
            fft,
            window,
            buffer: vec![Complex::default(); FFT_SIZE],
            band_ranges,
            smoothed: zero_frame(),
        }
    }

    /// 计算对数频带，并用更快的上升、更缓的回落减少视觉抖动。
    fn analyze(&mut self, samples: &VecDeque<f32>) -> &[f32] {
        for ((output, sample), window) in self.buffer.iter_mut().zip(samples).zip(&self.window) {
            *output = Complex::new(sample * window, 0.0);
        }
        self.fft.process(&mut self.buffer);

        for (band, range) in self.band_ranges.iter().enumerate() {
            let power = self.buffer[range.clone()]
                .iter()
                // 频带最大能量只需要平方值，避免对每个 bin 做一次平方根。
                .map(|value| value.norm_sqr())
                .fold(0.0_f32, f32::max)
                / (FFT_SIZE as f32 * 0.5).powi(2);
            let decibels = 10.0 * power.max(1.0e-12).log10();
            let normalized = ((decibels - NOISE_FLOOR_DB) / (PEAK_DB - NOISE_FLOOR_DB))
                .clamp(0.0, 1.0)
                .sqrt();
            // 上升用大系数、回落用小系数：能量增加时快速跟上、减少时缓慢回落，因此画面不会逐帧抖动。
            // 两者必须留在 0..=1：超过 1 会过冲来回振荡，等于 1 则完全不留缓动。
            let smoothing = if normalized > self.smoothed[band] {
                0.68
            } else {
                0.2
            };
            self.smoothed[band] += (normalized - self.smoothed[band]) * smoothing;
        }
        &self.smoothed
    }
}

/// 将频率映射到 FFT 下标，并跳过没有视觉意义的直流分量。
fn frequency_to_bin(frequency: f32) -> usize {
    ((frequency * FFT_SIZE as f32 / SAMPLE_RATE as f32).floor() as usize).max(1)
}

fn zero_frame() -> Vec<f32> {
    vec![0.0; OUTPUT_BAND_COUNT]
}

/// 将紧凑频谱帧定向发送到任务栏 WebView。
fn emit_spectrum<R: Runtime>(app: &AppHandle<R>, frame: &[f32]) {
    let payload = std::array::from_fn::<_, OUTPUT_BAND_COUNT, _>(|index| {
        (frame[index].clamp(0.0, 1.0) * u8::MAX as f32).round() as u8
    });
    if let Err(error) = app.emit_to("taskbar", MEDIA_SPECTRUM_CHANGED_EVENT, payload.as_slice()) {
        log::warn!("向任务栏广播播放器频谱失败: {error}");
    }
}
