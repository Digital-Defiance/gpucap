# gpucap

Run a command like `time(1)` and report **GPU**, **CPU**, and **unified memory** utilization on Apple Silicon Macs.

```bash
cargo install gpucap
gpucap sleep 1
```

## Requirements

- macOS on Apple Silicon (M1/M2/M3/M4 and later)
- arm64 (`aarch64-apple-darwin`)

Intel Macs and non-macOS platforms are not supported.

## Usage

Wrap any command:

```bash
gpucap ffmpeg -i in.mp4 out.mp4
gpucap --color=bright --color-scheme=bright make -j8
```

Example output (stderr):

```
gpu      avg 12.3%  peak 45.6%
cpu      avg  8.1%  peak 23.4%
memory   avg 52.0%  peak 55.1%
real     1.234567 s
```

### GPU exerciser

Generate sustained GPU load for testing:

```bash
gpucap gpuexercise --percent 50 --seconds 10
```

### Color output

Follows BrightDate / bright-iputils conventions:

- `--color[=WHEN]` — `auto`, `always`, `never`, `plain`, `ansi`, `truecolor`
- `--no-color`
- `--color-scheme=SCHEME` — `default` or `bright`
- Environment: `GPUCAP_COLOR`, `GPUCAP_COLOR_SCHEME`, plus standard `NO_COLOR` / `CLICOLOR`

## Metrics

| Metric | Source |
|--------|--------|
| GPU | IOKit `Device Utilization %` (IOReport fallback) |
| CPU | Mach `host_processor_info` (system-wide) |
| Memory | Unified RAM via `host_statistics64` + `hw.memsize` |

## License

MIT — see [LICENSE](LICENSE).
