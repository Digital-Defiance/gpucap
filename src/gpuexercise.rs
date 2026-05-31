use crate::format::{summarize_line, FormatContext};
use crate::gpu_proc::GpuProcessTracker;
use crate::metrics::{SampleTier, Sampler};
use crate::output::print_exercise_report;
use crate::platform::ChipProfile;
use crate::runner::{RunResult, RunStats};
use brightdate::BrightDate;
use clap::{Arg, ArgAction, Command};
use metal::{CompileOptions, Device, MTLResourceOptions};
use std::mem;
use std::time::{Duration, Instant};

const BUFFER_FLOATS: usize = 1 << 20;
const THREADGROUP_WIDTH: u64 = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExerciseMode {
    BestEffort,
    Load,
    Sample,
}

impl ExerciseMode {
    fn parse(value: &str) -> Result<Self, String> {
        match value.to_ascii_lowercase().as_str() {
            "best-effort" | "best_effort" | "besteffort" => Ok(Self::BestEffort),
            "load" => Ok(Self::Load),
            "sample" | "ambient" => Ok(Self::Sample),
            other => Err(format!(
                "unknown exercise mode '{other}' (expected best-effort, load, or sample)"
            )),
        }
    }
}

struct MetalLoader {
    pipeline: metal::ComputePipelineState,
    buffer: metal::Buffer,
    queue: metal::CommandQueue,
}

impl MetalLoader {
    fn new(device: &Device) -> Result<Self, String> {
        let source = include_str!("exercise.metal");
        let library = device
            .new_library_with_source(source, &CompileOptions::new())
            .map_err(|e| format!("Metal shader compile failed: {e}"))?;
        let function = library
            .get_function("gpu_load", None)
            .map_err(|e| format!("Metal kernel missing: {e}"))?;
        let pipeline = device
            .new_compute_pipeline_state_with_function(&function)
            .map_err(|e| format!("Metal pipeline failed: {e}"))?;
        let buffer = device.new_buffer(
            (BUFFER_FLOATS * mem::size_of::<f32>()) as u64,
            MTLResourceOptions::StorageModeShared,
        );
        let queue = device.new_command_queue();

        Ok(Self {
            pipeline,
            buffer,
            queue,
        })
    }

    fn dispatch(&self, iters: u32) {
        let cmd = self.queue.new_command_buffer();
        let encoder = cmd.new_compute_command_encoder();
        encoder.set_compute_pipeline_state(&self.pipeline);
        encoder.set_buffer(0, Some(&self.buffer), 0);
        encoder.set_bytes(
            1,
            mem::size_of::<u32>() as u64,
            &iters as *const u32 as _,
        );

        let grid = metal::MTLSize {
            width: BUFFER_FLOATS as u64,
            height: 1,
            depth: 1,
        };
        let threads = metal::MTLSize {
            width: THREADGROUP_WIDTH,
            height: 1,
            depth: 1,
        };
        encoder.dispatch_threads(grid, threads);
        encoder.end_encoding();
        cmd.commit();
    }
}

pub fn run(args: &[String]) -> i32 {
    let cmd = Command::new("gpuexercise")
        .about("Exercise the GPU at a target utilization for a duration")
        .arg(
            Arg::new("percent")
                .long("percent")
                .short('p')
                .value_name("PCT")
                .help("Target GPU utilization percentage (1–100)")
                .default_value("50"),
        )
        .arg(
            Arg::new("seconds")
                .long("seconds")
                .short('s')
                .value_name("SEC")
                .help("Duration in seconds")
                .default_value("10"),
        )
        .arg(
            Arg::new("format")
                .short('f')
                .long("format")
                .value_name("FORMAT")
                .help("Print statistics using a FORMAT string (plain text)")
        )
        .arg(
            Arg::new("color")
                .long("color")
                .value_name("WHEN")
                .num_args(0..=1)
                .default_missing_value("always")
                .help("Colorize output: auto, always, never, plain, ansi, or truecolor")
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new("no_color")
                .long("no-color")
                .help("Disable color output")
                .action(ArgAction::SetTrue)
                .conflicts_with("color"),
        )
        .arg(
            Arg::new("color_scheme")
                .long("color-scheme")
                .value_name("SCHEME")
                .help("Color palette: default or bright")
                .default_value("default"),
        )
        .arg(
            Arg::new("mode")
                .long("mode")
                .value_name("MODE")
                .help("best-effort: skip load if target below ambient (default); load: always generate GPU load; sample: measure ambient only")
                .default_value("best-effort"),
        )
        .args(crate::output_style_args())
        .args(crate::common_args());

    let argv: Vec<&str> = std::iter::once("gpuexercise")
        .chain(args.iter().skip(2).map(String::as_str))
        .collect();
    let matches = match cmd.try_get_matches_from(&argv) {
        Ok(m) => m,
        Err(e) => {
            let _ = e.print();
            return e.exit_code();
        }
    };

    let colors = match crate::resolve_color_settings(&matches) {
        Ok(c) => c,
        Err(code) => return code,
    };

    let target: f64 = match matches.get_one::<String>("percent").unwrap().parse() {
        Ok(p) if (1.0..=100.0).contains(&p) => p,
        _ => {
            eprintln!("bgpucap gpuexercise: --percent must be between 1 and 100");
            return 2;
        }
    };

    let seconds: f64 = match matches.get_one::<String>("seconds").unwrap().parse() {
        Ok(s) if s > 0.0 => s,
        _ => {
            eprintln!("bgpucap gpuexercise: --seconds must be a positive number");
            return 2;
        }
    };

    let mode = match ExerciseMode::parse(
        matches
            .get_one::<String>("mode")
            .map(|s| s.as_str())
            .unwrap_or("best-effort"),
    ) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("bgpucap gpuexercise: {e}");
            return 2;
        }
    };

    let chip = ChipProfile::detect();
    let style = match crate::resolve_report_style(&matches) {
        Ok(style) => style,
        Err(code) => return code,
    };
    let tier = crate::resolve_sample_tier(&style.metrics, None);

    match exercise_gpu(target, seconds, mode, tier) {
        Ok(result) => {
            let format = matches
                .get_one::<String>("format")
                .cloned()
                .or_else(|| {
                    std::env::var("BGPUCAP_FORMAT")
                        .or_else(|_| std::env::var("GPUCAP_FORMAT"))
                        .ok()
                });

            if let Some(fmt) = format {
                let ctx = FormatContext::from_run(&result, Some(target));
                if crate::json::is_json_format(&fmt) {
                    if let Err(e) = crate::json::write_result(
                        &mut std::io::stdout(),
                        &result,
                        &chip,
                        &style.metrics,
                        Some(target),
                    ) {
                        eprintln!("bgpucap gpuexercise: {e}");
                        return 1;
                    }
                } else if let Err(e) = summarize_line(&mut std::io::stderr(), &fmt, &ctx) {
                    eprintln!("bgpucap gpuexercise: {e}");
                    return 1;
                }
            } else {
                print_exercise_report(&colors, &chip, target, seconds, &result, &style);
            }
            0
        }
        Err(e) => {
            eprintln!("bgpucap gpuexercise: {e}");
            1
        }
    }
}

fn exercise_gpu(
    target: f64,
    seconds: f64,
    mode: ExerciseMode,
    tier: SampleTier,
) -> Result<RunResult, String> {
    let device = Device::system_default().ok_or("no Metal GPU found on this system")?;
    let loader = MetalLoader::new(&device)?;
    let mut sampler = Sampler::with_tier(tier);
    let self_pid = std::process::id() as i32;
    let mut gpu_proc = if tier == SampleTier::Full || mode != ExerciseMode::Sample {
        GpuProcessTracker::new(self_pid)
    } else {
        None
    };

    let baseline = measure_baseline(&mut sampler);
    let chase_target = match mode {
        ExerciseMode::Sample => false,
        ExerciseMode::Load => true,
        ExerciseMode::BestEffort => target > baseline + 1.0,
    };
    if mode == ExerciseMode::BestEffort && !chase_target {
        let suggested = suggest_percent(baseline);
        eprintln!(
            "bgpucap gpuexercise: note: ambient GPU usage is about {baseline:.0}%; \
             cannot hold a target at or below that (try --percent {suggested:.0}, \
             --mode load, or --mode sample)"
        );
    }
    let effective_target = if chase_target {
        target.min(100.0)
    } else {
        baseline
    };

    let start = Instant::now();
    let start_bd = BrightDate::now().value;
    let deadline = start + Duration::from_secs_f64(seconds);
    let mut stats = RunStats::default();
    let mut iters = if chase_target {
        initial_iters_for_target(effective_target)
    } else {
        100
    };
    let mut batch_count: u32 = if chase_target {
        initial_batch_for_target(effective_target)
    } else {
        1
    };
    let mut last_sample = Instant::now();

    while Instant::now() < deadline {
        if chase_target {
            for _ in 0..batch_count {
                if Instant::now() >= deadline {
                    break;
                }
                loader.dispatch(iters);
            }
            if Instant::now() >= deadline {
                break;
            }
        }

        let sample_interval = Duration::from_millis(100);
        let remaining = deadline.saturating_duration_since(Instant::now());
        std::thread::sleep(remaining.min(sample_interval));

        if Instant::now() >= deadline {
            break;
        }

        let now = Instant::now();
        let dt = now.duration_since(last_sample).as_secs_f64();
        last_sample = now;

        let mut snapshot = sampler.sample();
        if let Some(tracker) = gpu_proc.as_mut() {
            if let Some(cmd_gpu) = tracker.sample(dt) {
                snapshot.command_gpu = Some(cmd_gpu);
            }
        }
        stats.record(&snapshot);

        if chase_target {
            let util = snapshot.gpu;
            iters = adjust_iters(iters, util, effective_target);
            batch_count = adjust_batch(batch_count, util, effective_target);
        }
    }

    if !stats.has_gpu_samples() {
        return Err("could not read GPU utilization (IOKit PerformanceStatistics)".into());
    }

    let elapsed = start.elapsed().as_secs_f64();

    Ok(stats.into_run_result(
        vec![
            "gpuexercise".into(),
            "--percent".into(),
            target.to_string(),
            "--seconds".into(),
            seconds.to_string(),
        ],
        0,
        elapsed,
        start_bd,
        BrightDate::now().value,
        Some(self_pid),
    ))
}

fn suggest_percent(baseline: f64) -> f64 {
    (baseline + 5.0).min(100.0).max(1.0)
}

fn measure_baseline(sampler: &mut Sampler) -> f64 {
    let mut sum = 0.0;
    let mut n = 0u32;
    let deadline = Instant::now() + Duration::from_millis(400);
    while Instant::now() < deadline {
        sum += sampler.sample().gpu;
        n += 1;
        std::thread::sleep(Duration::from_millis(100));
    }
    if n == 0 {
        0.0
    } else {
        sum / n as f64
    }
}

fn initial_iters_for_target(target: f64) -> u32 {
    ((target * 800.0) as u32).clamp(50, 200_000)
}

fn initial_batch_for_target(target: f64) -> u32 {
    let batch = (target / 12.0).ceil() as u32;
    batch.clamp(1, 24)
}

fn adjust_iters(current: u32, util: f64, target: f64) -> u32 {
    let margin = 5.0;
    let next = if util < target - margin {
        ((current as f64) * 1.18) as u32
    } else if util > target + margin {
        ((current as f64) * 0.82) as u32
    } else {
        current
    };
    next.clamp(10, 100_000)
}

fn adjust_batch(current: u32, util: f64, target: f64) -> u32 {
    let margin = 5.0;
    let next = if util < target - margin {
        current.saturating_add(1)
    } else if util > target + margin {
        current.saturating_sub(1)
    } else {
        current
    };
    next.clamp(1, 16)
}
