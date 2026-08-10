//! One place to open an ONNX session, so every model gets the same treatment.
//!
//! Until now each model built its own session and every one of them ran on the
//! CPU — no execution provider was ever registered, so the GPU sat idle through
//! indexing, captioning and everything else.
//!
//! On Windows this asks DirectML first. DirectML was chosen over CUDA because it
//! is part of the operating system: it works on any DX12 GPU, integrated ones
//! included, and needs no toolkit installed and no gigabyte of vendor runtime
//! shipped alongside the app. It is somewhat slower than CUDA on an NVIDIA card,
//! and that is the trade — a feature everyone can run beats a faster one that
//! only NVIDIA owners reach.
//!
//! Registration is best-effort by design. A machine with no usable GPU, an
//! outdated driver, or a model using an operator DirectML has no kernel for,
//! falls back to the CPU and still produces the right answer. That fallback is
//! why the provider list is not `error_on_failure`.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use ort::session::{builder::GraphOptimizationLevel, Session};

/// Threads for the CPU path. Half the logical cores leaves the interface
/// responsive while a backfill grinds through a library in the background.
fn intra_threads() -> usize {
    std::thread::available_parallelism()
        .map(|count| (count.get() / 2).max(2))
        .unwrap_or(2)
}

/// Opens `path` as an inference session, on the GPU where one can be had.
pub fn open(path: &Path) -> anyhow::Result<Session> {
    open_with_threads(path, intra_threads())
}

/// Opens a model that has to stay on the processor whatever the setting says.
///
/// Not every graph runs on a graphics card. LaMa is the case here: its Fourier
/// convolutions — the very thing that lets it carry a wall across a hole —
/// contain a MatMul DirectML has no kernel for, and it answers `E_INVALIDARG`
/// rather than falling back on its own.
///
/// Twenty-five seconds on the processor is a fine price for the one tool that
/// does not need the speed. Generation, which does, runs on the card.
pub fn open_on_cpu(path: &Path) -> anyhow::Result<Session> {
    Session::builder()
        .map_err(to_anyhow)?
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(to_anyhow)?
        .with_intra_threads(intra_threads())
        .map_err(to_anyhow)?
        .commit_from_file(path)
        .map_err(to_anyhow)
}

/// Same, with the thread count pinned — for models run in an already-parallel
/// loop, where letting each session spawn its own pool would oversubscribe.
pub fn open_with_threads(path: &Path, threads: usize) -> anyhow::Result<Session> {
    let mut builder = Session::builder()
        .map_err(to_anyhow)?
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(to_anyhow)?
        .with_intra_threads(threads)
        .map_err(to_anyhow)?;

    #[cfg(windows)]
    if gpu_allowed() {
        // The two option enums live in the provider's own module; only the
        // provider itself is re-exported from `ep`.
        use ort::ep::directml::{DeviceFilter, PerformancePreference};
        use ort::ep::DirectML;

        // Two requirements ONNX Runtime documents for DirectML and then does not
        // enforce: no memory-pattern optimiser, and sequential execution.
        // Leaving either on is not rejected — the session opens, runs, and later
        // faults inside the driver.
        builder = builder.with_memory_pattern(false).map_err(to_anyhow)?;
        builder = builder.with_parallel_execution(false).map_err(to_anyhow)?;

        // And the one that actually mattered. DirectML's default preference is
        // power saving, which on a laptop means the *integrated* chip — half a
        // gigabyte of shared memory beside an idle 4 GB discrete card. Loading a
        // 1.7 GB model there faults the driver and takes the process with it,
        // intermittently, with `STATUS_ACCESS_VIOLATION` and nothing in the log.
        //
        // Three earlier attempts blamed a race, then the missing session
        // options, then DirectML as a whole; the real bug was asking for the
        // wrong adapter all along. Machines with one GPU never see it, which is
        // why it reads as flaky rather than wrong.
        builder = builder
            .with_execution_providers([DirectML::default()
                .with_performance_preference(PerformancePreference::HighPerformance)
                .with_device_filter(DeviceFilter::Gpu)
                .build()])
            .map_err(to_anyhow)?;
    }

    builder.commit_from_file(path).map_err(to_anyhow)
}

/// Whether models may use the graphics card.
///
/// Starts false and is set from the stored preference at startup, which defaults
/// to on. `HIVE_GPU=1` forces it on for one run without touching the setting —
/// how the adapter and quantization problems above were pinned down.
static GPU_ENABLED: AtomicBool = AtomicBool::new(false);

/// Applied at startup from the stored preference, and whenever it changes.
pub fn set_gpu_enabled(enabled: bool) {
    GPU_ENABLED.store(enabled, Ordering::Relaxed);
}

fn gpu_allowed() -> bool {
    GPU_ENABLED.load(Ordering::Relaxed)
        || matches!(std::env::var("HIVE_GPU").as_deref(), Ok("1") | Ok("true"))
}

/// The GPU backend currently in use, or `None` when everything runs on the CPU.
///
/// Registration failing is silent by design, so without this there is no way to
/// tell a slow machine apart from one that quietly fell back — the symptom of
/// both is the same.
pub fn gpu_backend() -> Option<&'static str> {
    #[cfg(windows)]
    if gpu_allowed() {
        use ort::ep::{DirectML, ExecutionProvider};
        if DirectML::default().is_available().unwrap_or(false) {
            return Some("DirectML");
        }
    }
    None
}

/// Whether a graphics card is there to be switched on, regardless of whether it
/// currently is. Lets Settings offer the choice only where it means something.
pub fn gpu_available() -> bool {
    #[cfg(windows)]
    {
        use ort::ep::{DirectML, ExecutionProvider};
        return DirectML::default().is_available().unwrap_or(false);
    }
    #[cfg(not(windows))]
    false
}

fn to_anyhow(error: impl std::fmt::Display) -> anyhow::Error {
    anyhow::anyhow!("{error}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_thread_count_always_leaves_room_to_work() {
        let threads = intra_threads();
        assert!(threads >= 2, "a single thread would crawl");
        let cores = std::thread::available_parallelism().map(|c| c.get()).unwrap_or(2);
        assert!(threads <= cores, "asked for more threads than the machine has");
    }

    /// Opens a real model and reports whether the GPU was reachable. Skipped by
    /// default; run with
    /// `cargo test --lib session -- --include-ignored --nocapture`.
    ///
    /// Worth re-running after a driver update: DirectML failing to register is
    /// silent by design, and the only symptom is everything being slow again.
    #[test]
    fn the_switch_survives_being_flipped_both_ways() {
        let was = GPU_ENABLED.load(Ordering::Relaxed);
        set_gpu_enabled(true);
        assert!(GPU_ENABLED.load(Ordering::Relaxed));
        set_gpu_enabled(false);
        assert!(!GPU_ENABLED.load(Ordering::Relaxed));
        set_gpu_enabled(was);
    }

    #[test]
    #[ignore]
    fn a_real_model_opens_on_the_best_backend_available() {
        // The switch is process-wide, so it is put back: leaving it on made the
        // default-off test fail depending on which ran first.
        let was = GPU_ENABLED.load(Ordering::Relaxed);
        set_gpu_enabled(true);
        println!("gpu available: {}", gpu_available());
        println!("gpu backend: {:?}", gpu_backend());

        let Some(model) = std::env::var_os("APPDATA").map(|base| {
            Path::new(&base)
                .join("com.hive")
                .join("models")
                .join("clip")
                .join("vision_model.onnx")
        }) else {
            return;
        };
        if !model.is_file() {
            return;
        }

        let started = std::time::Instant::now();
        let session = open(&model).expect("session opens");
        println!(
            "opened in {:?}, {} inputs",
            started.elapsed(),
            session.inputs().len()
        );
        set_gpu_enabled(was);
    }
}
