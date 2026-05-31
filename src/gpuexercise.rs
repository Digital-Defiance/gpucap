use crate::format::{summarize_line, FormatContext};
use crate::iokit::gpu_utilization_iokit;
use crate::metrics::PercentStats;
use crate::output::print_exercise_report;
use brightdate::BrightDate;
use clap::{Arg, ArgAction, Command};
use metal::{CompileOptions, Device, MTLResourceOptions};
use std::mem;
use std::time::{Duration, Instant};

const BUFFER_FLOATS: usize = 1 << 20;
const THREADGROUP_WIDTH: u64 = 256;

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
        );

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

    match exercise_gpu(target, seconds) {
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
                let ctx = FormatContext {
                    command: vec![
                        "gpuexercise".into(),
                        "--percent".into(),
                        target.to_string(),
                        "--seconds".into(),
                        seconds.to_string(),
                    ],
                    wait_status: 0,
                    elapsed_secs: result.elapsed_secs,
                    start_bd: result.start_bd,
                    end_bd: result.end_bd,
                    gpu: result.gpu,
                    cpu: PercentStats::default(),
                    memory: PercentStats::default(),
                    exercise_target: Some(target),
                };
                if let Err(e) = summarize_line(&mut std::io::stderr(), &fmt, &ctx) {
                    eprintln!("bgpucap gpuexercise: {e}");
                    return 1;
                }
            } else {
                print_exercise_report(&colors, target, seconds, &result.gpu);
            }
            0
        }
        Err(e) => {
            eprintln!("bgpucap gpuexercise: {e}");
            1
        }
    }
}

struct ExerciseResult {
    gpu: PercentStats,
    elapsed_secs: f64,
    start_bd: f64,
    end_bd: f64,
}

fn exercise_gpu(target: f64, seconds: f64) -> Result<ExerciseResult, String> {
    let device = Device::system_default().ok_or("no Metal GPU found on this system")?;
    let loader = MetalLoader::new(&device)?;

    let baseline = measure_baseline();
    if target <= baseline + 1.0 {
        eprintln!(
            "bgpucap gpuexercise: note: ambient GPU usage is about {baseline:.0}%; \
             cannot hold a target at or below that (try a higher --percent)"
        );
    }
    let effective_target = target.max(baseline + 2.0);

    let start = Instant::now();
    let start_bd = BrightDate::now().value;
    let deadline = start + Duration::from_secs_f64(seconds);
    let mut stats = PercentStats::default();
    let mut iters = initial_iters_for_target(effective_target);
    let mut batch_count: u32 = initial_batch_for_target(effective_target);

    while Instant::now() < deadline {
        for _ in 0..batch_count {
            loader.dispatch(iters);
        }

        std::thread::sleep(Duration::from_millis(100));

        let util = gpu_utilization_iokit().unwrap_or(0.0);
        stats.record(util);

        iters = adjust_iters(iters, util, effective_target);
        batch_count = adjust_batch(batch_count, util, effective_target);
    }

    if stats.samples == 0 {
        return Err("could not read GPU utilization (IOKit PerformanceStatistics)".into());
    }

    Ok(ExerciseResult {
        gpu: stats,
        elapsed_secs: start.elapsed().as_secs_f64(),
        start_bd,
        end_bd: BrightDate::now().value,
    })
}

fn measure_baseline() -> f64 {
    let mut sum = 0.0;
    let mut n = 0u32;
    let deadline = Instant::now() + Duration::from_millis(400);
    while Instant::now() < deadline {
        if let Some(util) = gpu_utilization_iokit() {
            sum += util;
            n += 1;
        }
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
    next.clamp(10, 500_000)
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
    next.clamp(1, 32)
}
