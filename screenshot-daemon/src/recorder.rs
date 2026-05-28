use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

use ffmpeg_next as ffmpeg;

pub struct Recorder {
    handle: Option<std::thread::JoinHandle<()>>,
    stop_flag: Arc<AtomicBool>,
    pause_flag: Arc<AtomicBool>,
    output_path: PathBuf,
    display_server: crate::detect::DisplayServer,
    control_pid: Option<u32>,
    paused: bool,
    finished: Arc<AtomicBool>,
}

impl Recorder {
    pub fn new(display_server: &crate::detect::DisplayServer) -> Self {
        let save_dir = dirs::video_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("screencasts");
        let _ = std::fs::create_dir_all(&save_dir);
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let filename = format!("recording_{}.webm", timestamp);
        let output_path = save_dir.join(&filename);

        Self {
            handle: None,
            stop_flag: Arc::new(AtomicBool::new(false)),
            pause_flag: Arc::new(AtomicBool::new(false)),
            output_path,
            display_server: display_server.clone(),
            control_pid: None,
            paused: false,
            finished: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn output_path(&self) -> &Path {
        &self.output_path
    }

    pub fn is_recording(&self) -> bool {
        self.handle.is_some() && !self.finished.load(Ordering::SeqCst)
    }

    pub fn set_control_pid(&mut self, pid: u32) {
        self.control_pid = Some(pid);
    }

    #[allow(dead_code)]
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    pub async fn start(&mut self) -> anyhow::Result<()> {
        if self.is_recording() {
            anyhow::bail!("already recording");
        }

        ffmpeg::init()?;

        self.stop_flag.store(false, Ordering::SeqCst);
        self.pause_flag.store(false, Ordering::SeqCst);
        self.finished.store(false, Ordering::SeqCst);

        let output_path = self.output_path.clone();
        let display_server = self.display_server.clone();
        let stop_flag = self.stop_flag.clone();
        let pause_flag = self.pause_flag.clone();
        let finished = self.finished.clone();

        let handle = std::thread::Builder::new()
            .name("screen-recorder".into())
            .spawn(move || {
                if let Err(e) = run_recording_loop(
                    &output_path,
                    &display_server,
                    &stop_flag,
                    &pause_flag,
                ) {
                    log::error!("recording loop error: {e}");
                }
                finished.store(true, Ordering::SeqCst);
            })?;

        self.handle = Some(handle);
        self.paused = false;
        log::info!("recording started: {}", self.output_path.display());
        Ok(())
    }

    pub async fn stop(&mut self) -> anyhow::Result<PathBuf> {
        if let Some(pid) = self.control_pid.take() {
            log::info!("stopping control bar (pid {})", pid);
            let _ = unsafe { libc::kill(pid as i32, libc::SIGTERM) };
        }

        self.stop_flag.store(true, Ordering::SeqCst);

        if let Some(handle) = self.handle.take() {
            tokio::task::spawn_blocking(move || handle.join())
                .await
                .map_err(|e| anyhow::anyhow!("join task failed: {e}"))?
                .map_err(|_| anyhow::anyhow!("recorder thread panicked"))?;
        }

        self.paused = false;
        log::info!("recording stopped: {}", self.output_path.display());
        Ok(self.output_path.clone())
    }

    pub async fn toggle_pause(&mut self) -> anyhow::Result<bool> {
        if !self.is_recording() {
            anyhow::bail!("not recording");
        }

        if self.paused {
            self.pause_flag.store(false, Ordering::SeqCst);
            self.paused = false;
            log::info!("recording resumed");
        } else {
            self.pause_flag.store(true, Ordering::SeqCst);
            self.paused = true;
            log::info!("recording paused");
        }
        Ok(self.paused)
    }
}

fn run_recording_loop(
    output_path: &Path,
    display_server: &crate::detect::DisplayServer,
    stop_flag: &AtomicBool,
    pause_flag: &AtomicBool,
) -> anyhow::Result<()> {
    let (width, height) = detect_screen_size(display_server)?;
    log::info!("recording at {}x{}", width, height);

    let fps = 30;
    let mut output = ffmpeg::format::output(output_path)
        .map_err(|e| anyhow::anyhow!("open output failed: {e}"))?;

    let codec = ffmpeg::encoder::find(ffmpeg::codec::Id::VP9)
        .ok_or_else(|| anyhow::anyhow!("VP9 encoder not found"))?;

    let global_header = output.format().flags().contains(ffmpeg::format::Flags::GLOBAL_HEADER);

    let encoder_ctx = ffmpeg::codec::context::Context::new_with_codec(codec);
    let mut encoder = encoder_ctx
        .encoder()
        .video()
        .map_err(|e| anyhow::anyhow!("create encoder failed: {e}"))?;

    encoder.set_width(width);
    encoder.set_height(height);
    encoder.set_format(ffmpeg::util::format::pixel::Pixel::YUV420P);
    encoder.set_time_base((1, fps as i32));
    encoder.set_frame_rate(Some((fps as i32, 1)));

    if global_header {
        encoder.set_flags(ffmpeg::codec::Flags::GLOBAL_HEADER);
    }

    let mut encoder_options = ffmpeg::Dictionary::new();
    encoder_options.set("crf", "30");
    encoder_options.set("b:v", "0");
    encoder_options.set("deadline", "realtime");
    encoder_options.set("cpu-used", "8");

    let opened_encoder = encoder
        .open_as_with(codec, encoder_options)
        .map_err(|e| anyhow::anyhow!("open encoder failed: {e}"))?;

    let stream_index = {
        let mut stream = output
            .add_stream(codec)
            .map_err(|e| anyhow::anyhow!("add stream failed: {e}"))?;
        stream.set_time_base((1, fps as i32));
        stream.set_parameters(&opened_encoder);
        stream.index()
    };

    output
        .write_header()
        .map_err(|e| anyhow::anyhow!("write header failed: {e}"))?;

    let stream_time_base = output
        .stream(stream_index)
        .ok_or_else(|| anyhow::anyhow!("missing output stream"))?
        .time_base();

    let mut scaler = ffmpeg::software::scaling::context::Context::get(
        ffmpeg::util::format::pixel::Pixel::RGB24,
        width,
        height,
        ffmpeg::util::format::pixel::Pixel::YUV420P,
        width,
        height,
        ffmpeg::software::scaling::flag::Flags::BILINEAR,
    )
    .map_err(|e| anyhow::anyhow!("create scaler failed: {e}"))?;

    let mut yuv_frame = ffmpeg::util::frame::video::Video::new(
        ffmpeg::util::format::pixel::Pixel::YUV420P,
        width,
        height,
    );

    let mut frame_index: i64 = 0;
    let frame_duration = std::time::Duration::from_micros(1_000_000 / fps as u64);

    let mut opened_encoder = opened_encoder;

    while !stop_flag.load(Ordering::SeqCst) {
        if pause_flag.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(50));
            continue;
        }

        let capture_start = std::time::Instant::now();

        let rgb_data = capture_screen_frame(display_server, width, height)?;

        let mut rgb_frame = ffmpeg::util::frame::video::Video::new(
            ffmpeg::util::format::pixel::Pixel::RGB24,
            width,
            height,
        );
        {
            let stride = rgb_frame.stride(0);
            let row_len = width as usize * 3;
            let plane = rgb_frame.data_mut(0);
            for row in 0..height as usize {
                let src_start = row * row_len;
                let src_end = src_start + row_len;
                let dst_start = row * stride;
                let dst_end = dst_start + row_len;
                if src_end <= rgb_data.len() && dst_end <= plane.len() {
                    plane[dst_start..dst_end].copy_from_slice(&rgb_data[src_start..src_end]);
                }
            }
        }

        scaler
            .run(&rgb_frame, &mut yuv_frame)
            .map_err(|e| anyhow::anyhow!("scale frame failed: {e}"))?;

        yuv_frame.set_pts(Some(frame_index));
        opened_encoder
            .send_frame(&yuv_frame)
            .map_err(|e| anyhow::anyhow!("send frame failed: {e}"))?;

        drain_encoded_packets(&mut opened_encoder, &mut output, stream_index, stream_time_base)?;

        frame_index += 1;

        let elapsed = capture_start.elapsed();
        if elapsed < frame_duration {
            std::thread::sleep(frame_duration - elapsed);
        }
    }

    opened_encoder
        .send_eof()
        .map_err(|e| anyhow::anyhow!("send encoder eof failed: {e}"))?;
    drain_encoded_packets(&mut opened_encoder, &mut output, stream_index, stream_time_base)?;

    output
        .write_trailer()
        .map_err(|e| anyhow::anyhow!("write trailer failed: {e}"))?;

    log::info!("recording written: {} ({} frames)", output_path.display(), frame_index);
    Ok(())
}

fn drain_encoded_packets(
    encoder: &mut ffmpeg::encoder::video::Encoder,
    output: &mut ffmpeg::format::context::Output,
    stream_index: usize,
    stream_time_base: ffmpeg::Rational,
) -> anyhow::Result<()> {
    let mut packet = ffmpeg::Packet::empty();
    while encoder.receive_packet(&mut packet).is_ok() {
        packet.set_stream(stream_index);
        packet.rescale_ts(encoder.time_base(), stream_time_base);
        packet
            .write_interleaved(output)
            .map_err(|e| anyhow::anyhow!("write packet failed: {e}"))?;
    }
    Ok(())
}

fn detect_screen_size(display_server: &crate::detect::DisplayServer) -> anyhow::Result<(u32, u32)> {
    match display_server {
        crate::detect::DisplayServer::X11 => {
            use x11rb::connection::Connection;
            use x11rb::rust_connection::RustConnection;

            let (conn, screen_num) = RustConnection::connect(None)?;
            let screen = &conn.setup().roots[screen_num];
            Ok((screen.width_in_pixels as u32, screen.height_in_pixels as u32))
        }
        crate::detect::DisplayServer::Wayland | crate::detect::DisplayServer::Unknown => {
            let conn = libwayshot::WayshotConnection::new()?;
            let img = conn.screenshot_all(false)?;
            Ok((img.width() as u32, img.height() as u32))
        }
    }
}

fn capture_screen_frame(
    display_server: &crate::detect::DisplayServer,
    width: u32,
    height: u32,
) -> anyhow::Result<Vec<u8>> {
    match display_server {
        crate::detect::DisplayServer::X11 => capture_x11_frame(width, height),
        crate::detect::DisplayServer::Wayland | crate::detect::DisplayServer::Unknown => {
            capture_wayland_frame(width, height)
        }
    }
}

fn capture_x11_frame(width: u32, height: u32) -> anyhow::Result<Vec<u8>> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::*;
    use x11rb::rust_connection::RustConnection;

    let (conn, screen_num) = RustConnection::connect(None)?;
    let screen = &conn.setup().roots[screen_num];

    let reply = get_image(
        &conn,
        ImageFormat::Z_PIXMAP,
        screen.root,
        0,
        0,
        width as u16,
        height as u16,
        u32::MAX,
    )?
    .reply()?;

    let mut rgb = Vec::with_capacity((width * height * 3) as usize);
    for chunk in reply.data.chunks_exact(4) {
        rgb.push(chunk[2]);
        rgb.push(chunk[1]);
        rgb.push(chunk[0]);
    }

    Ok(rgb)
}

fn capture_wayland_frame(width: u32, height: u32) -> anyhow::Result<Vec<u8>> {
    let conn = libwayshot::WayshotConnection::new()?;
    let img = conn.screenshot_all(false)?;
    let rgba_buf = img.as_rgba8().ok_or_else(|| anyhow::anyhow!("failed to get rgba8 buffer"))?;
    let rgba = rgba_buf.as_raw();

    let mut rgb = Vec::with_capacity((width * height * 3) as usize);
    for chunk in rgba.chunks_exact(4) {
        rgb.push(chunk[0]);
        rgb.push(chunk[1]);
        rgb.push(chunk[2]);
    }

    Ok(rgb)
}

pub type SharedRecorder = Arc<Mutex<Recorder>>;
