use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

/// URL of the default Whisper model (base, ~142 MB, multilingual).
pub const DEFAULT_MODEL_URL: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin";

pub fn model_path_for(data_dir: &Path) -> PathBuf {
    data_dir.join("models").join("ggml-base.bin")
}

/// Whether the `stt` Cargo feature was compiled in. The transcribe command
/// only actually works if this returns true.
pub const fn is_available() -> bool {
    cfg!(feature = "stt")
}

/// Download the Whisper model by shelling out to `curl` (standard on Linux).
/// Writes to `dest` and creates the parent directory if needed.
pub async fn download_model(dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let out = tokio::process::Command::new("curl")
        .arg("-L")
        .arg("--fail")
        .arg("-o")
        .arg(dest)
        .arg(DEFAULT_MODEL_URL)
        .output()
        .await
        .map_err(|e| anyhow!("failed to run curl: {}", e))?;
    if !out.status.success() {
        let _ = tokio::fs::remove_file(dest).await;
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(anyhow!("curl failed: {}", stderr));
    }
    Ok(())
}

#[cfg(feature = "stt")]
pub fn transcribe_wav(wav: &[u8], model_path: &Path) -> Result<String> {
    use hound::SampleFormat;
    use std::io::Cursor;
    use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

    if !model_path.exists() {
        return Err(anyhow!(
            "Whisper model not found at {}. Download it via the Settings panel first.",
            model_path.display()
        ));
    }

    // Debug: save the incoming WAV to /tmp so the user can play it back and
    // verify that audio capture actually works. Overwritten on every call.
    let _ = std::fs::write("/tmp/ccm-last-recording.wav", wav);

    // --- 1. Decode WAV into f32 mono samples at its native rate. ----------
    let mut reader = hound::WavReader::new(Cursor::new(wav))
        .map_err(|e| anyhow!("invalid WAV input: {}", e))?;
    let spec = reader.spec();
    let channels = spec.channels as usize;
    let src_rate = spec.sample_rate;

    let samples: Vec<f32> = match (spec.sample_format, spec.bits_per_sample) {
        (SampleFormat::Float, 32) => reader
            .samples::<f32>()
            .filter_map(|s| s.ok())
            .collect(),
        (SampleFormat::Int, bits) => {
            let max = (1i64 << (bits - 1)) as f32;
            reader
                .samples::<i32>()
                .filter_map(|s| s.ok())
                .map(|s| s as f32 / max)
                .collect()
        }
        _ => return Err(anyhow!("unsupported WAV format: {:?}", spec)),
    };

    // Downmix to mono by averaging channels.
    let mono: Vec<f32> = if channels <= 1 {
        samples
    } else {
        samples
            .chunks(channels)
            .map(|c| c.iter().sum::<f32>() / channels as f32)
            .collect()
    };

    // --- 2. Resample to 16 kHz (whisper's required input rate). ----------
    let resampled = if src_rate == 16_000 {
        mono
    } else {
        linear_resample(&mono, src_rate as usize, 16_000)
    };

    // --- 3. Run Whisper. --------------------------------------------------
    let ctx = WhisperContext::new_with_params(
        model_path
            .to_str()
            .ok_or_else(|| anyhow!("model path is not UTF-8"))?,
        WhisperContextParameters::default(),
    )
    .map_err(|e| anyhow!("failed to load whisper model: {}", e))?;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    // Auto-detect language. Without this, whisper assumes English even
    // when the audio is in another language.
    params.set_language(Some("auto"));
    // Skip the typical non-speech tokens that Whisper emits when it hears
    // background noise — we want clean text only.
    params.set_suppress_blank(true);
    params.set_suppress_nst(true);

    let mut state = ctx
        .create_state()
        .map_err(|e| anyhow!("failed to create whisper state: {}", e))?;
    state
        .full(params, &resampled)
        .map_err(|e| anyhow!("whisper transcription failed: {}", e))?;

    let num_segments = state
        .full_n_segments()
        .map_err(|e| anyhow!("segment count: {}", e))?;
    let mut text = String::new();
    for i in 0..num_segments {
        if let Ok(seg) = state.full_get_segment_text(i) {
            text.push_str(&seg);
        }
    }
    Ok(text.trim().to_string())
}

#[cfg(not(feature = "stt"))]
pub fn transcribe_wav(_wav: &[u8], _model_path: &Path) -> Result<String> {
    Err(anyhow!(
        "Speech-to-text was not compiled in. Rebuild with `--features stt` \
         (requires `cmake` and a C++ toolchain)."
    ))
}

/// Very simple linear resampler. Good enough for speech → Whisper; not a
/// high-quality DSP resampler, but adequate for voice dictation.
#[cfg(feature = "stt")]
fn linear_resample(input: &[f32], src_rate: usize, dst_rate: usize) -> Vec<f32> {
    if input.is_empty() || src_rate == dst_rate {
        return input.to_vec();
    }
    let ratio = src_rate as f64 / dst_rate as f64;
    let out_len = ((input.len() as f64) / ratio) as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let x = i as f64 * ratio;
        let idx = x as usize;
        let frac = (x - idx as f64) as f32;
        let a = input[idx];
        let b = if idx + 1 < input.len() { input[idx + 1] } else { a };
        out.push(a + (b - a) * frac);
    }
    out
}
