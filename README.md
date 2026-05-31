# gpucap (`bgpucap`)

Run a command like `time(1)` and report **GPU**, **CPU**, and **unified memory** utilization on Apple Silicon Macs.

Part of [Bright Utils](https://brightutils.digitaldefiance.org): the **`b`** in **`bgpucap`** is for **Bright** (same convention as `btime`, `bfind`, `bping`).

| | |
|---|---|
| **Command** | `bgpucap` |
| **Crate** | [`gpucap`](https://crates.io/crates/gpucap) |
| **Docs** | [digital-defiance.github.io/gpucap](https://digital-defiance.github.io/gpucap/) |
| **Man page** | `man bgpucap` (installed by Homebrew) |

```bash
cargo install gpucap    # installs executable bgpucap
bgpucap sleep 1
man bgpucap             # after Homebrew install
```

Homebrew also symlinks `gpucap` → `bgpucap` for backward compatibility.

## Requirements

- macOS on Apple Silicon (M1/M2/M3/M4 and later)
- arm64 (`aarch64-apple-darwin`)

Intel Macs and non-macOS platforms are not supported.

## Usage

Wrap any command:

```bash
bgpucap ffmpeg -i in.mp4 out.mp4
bgpucap --metrics basic sleep 1          # lightweight: gpu/cpu/memory only
bgpucap --metrics gpu,pwr,freq sleep 5
bgpucap --list-metrics                   # show names and groups
bgpucap -f json sleep 1                  # JSON output
bgpucap --pid 1234 --metrics cmd-gpu sleep 5
bgpucap --columns --separator=' / ' sleep 1
```

Example output (stderr, colored on TTY):

```
gpu        avg 12.3%  peak 45.6%
cpu        avg  8.1%  peak 23.4%
memory     avg 52.0%  peak 55.1%
gpu-mhz    avg 900 MHz  peak 1200 MHz
e-mhz      avg 638 MHz  peak 900 MHz
p-mhz      avg 1231 MHz  peak 3200 MHz
gpu-pwr    avg 23 W  peak 45 W
cpu-pwr    avg 8 W  peak 19 W
dram-pwr   avg 21 W  peak 25 W
real     1.234567 s
```

### Custom format (`-f` / `--format`)

Machine-readable output using [BrightDate FORMAT-SPEC](https://github.com/Digital-Defiance/brightdate-rust/blob/main/FORMAT-SPEC.md) conventions where applicable. Format output is always plain text (no color), even with `--color=always`.

```bash
bgpucap -f '%gA,%gP,%uA,%uP,%hA,%hP,%e,%Ws,%Wt' sleep 1
bgpucap gpuexercise -f 'target=%tG gpu=%gA/%gP elapsed=%e' --percent 50 --seconds 5
```

Environment: `BGPUCAP_FORMAT` (same as `-f`; `GPUCAP_FORMAT` still accepted).

| Specifier | Meaning |
|-----------|---------|
| `%gA` / `%gP` | GPU average / peak % |
| `%gI` / `%gJ` | GPU unified memory in use (bytes) avg / peak |
| `%gM` / `%gO` | GPU memory allocated (bytes) avg / peak |
| `%gR` / `%gS` | Renderer utilization % avg / peak |
| `%gL` / `%gY` | Tiler utilization % avg / peak |
| `%gF` / `%gV` | GPU frequency (MHz) avg / peak |
| `%gU` / `%gW` | GPU temperature (°C) avg / peak |
| `%gT` | Thermal throttle (% of samples throttled) |
| `%uA` / `%uP` | CPU average / peak % |
| `%uF` / `%uV` | CPU frequency (MHz) avg / peak |
| `%gC` / `%gD` | Command process GPU % avg / peak (IORegistry `accumulatedGPUTime`) |
| `%gN` / `%gQ` | GPU SRAM power (W) avg / peak |
| `%gB` / `%gK` | GPU power (W) avg / peak |
| `%uE` / `%uQ` | E-core CPU frequency MHz avg / peak |
| `%uH` / `%uZ` | P-core CPU frequency MHz avg / peak |
| `%uB` / `%uK` | CPU package power (W) avg / peak |
| `%uG` / `%uR` | E-core power (W) avg / peak |
| `%uI` / `%uS` | P-core power (W) avg / peak |
| `%aB` / `%aK` | ANE power (W) avg / peak |
| `%hG` / `%hJ` | DRAM power (W) avg / peak |
| `%hA` / `%hP` | Memory average / peak % |
| `%hW` / `%hX` | Wired memory (bytes) avg / peak |
| `%hC` / `%hD` | Compressed memory (bytes) avg / peak |
| `%hS` / `%hO` | Swap used (bytes) avg / peak |
| `%hK` / `%hL` | Memory pressure level avg / peak (0=normal, 1=warn, 2=critical) |
| `%tG` | Exercise target GPU % (`gpuexercise` only) |
| `%e` | Elapsed seconds (`sec.centis`) |
| `%E` | Elapsed wall time (`m:ss.cc` or `h:mm:ss`) |
| `%B` | Elapsed BrightDate days (`{:.9}`) |
| `%b` | Elapsed millidays (`{:.6}`) |
| `%dE` | Elapsed millidays with ` md` suffix |
| `%Ws` / `%N` | Start BrightDate (`{:.9}`) |
| `%Wt` / `%n` | End BrightDate (`{:.9}`) |
| `%C` | Command line |
| `%x` | Exit status |
| `%%` | Literal `%` |
| `\t`, `\n` | Tab / newline |

Default machine format: `%gA,%gP,%uA,%uP,%hA,%hP,%e,%Ws,%Wt\n`

Use `-f json` for structured JSON on **stdout** (human reports stay on stderr; respects `--metrics`). Example:

```bash
bgpucap -f json --metrics basic sleep 1 > report.json
bgpucap compare before.json after.json
```

### Human report options

- `--metrics=LIST` — comma-separated metrics or groups (`basic`, `power`, `freq`, …). Env: `BGPUCAP_METRICS`. Default: all.
- `--list-metrics` — print metric names and groups
- `--separator=TEXT` — between avg value and `peak` (default: space). Env: `BGPUCAP_SEPARATOR`
- `--columns` — align avg/peak values in columns
- `--pid=PID` — track GPU usage for a specific process (IORegistry). Default child PID when `cmd-gpu` is sampled.
- `--no-track-gpu` — skip per-process GPU tracking even when `cmd-gpu` is in the metric set (conflicts with `--pid`).

When `--metrics=basic` (or a subset without extended metrics), bgpucap skips IOReport energy/freq subscriptions for lower overhead.

### Continuous watch

Sample system metrics until Ctrl+C (or `--count N`):

```bash
bgpucap watch                          # human live lines on stderr, summary on exit
bgpucap watch -n 10 --interval 500     # 10 samples every 500 ms
bgpucap watch -f json                  # NDJSON samples + final JSON on stdout
bgpucap watch --metrics basic --pid 1234
```

Live samples go to stderr (human) or stdout as NDJSON (`-f json`). The aggregated summary uses the same format as a normal run on exit.

### Compare JSON reports

Diff average metrics between two `-f json` captures:

```bash
bgpucap -f json --metrics basic sleep 5 > before.json
# … run workload …
bgpucap -f json --metrics basic sleep 5 > after.json
bgpucap compare before.json after.json
```

Output is a plain table on stdout: left avg, right avg, and delta per metric. Only metrics present in **both** reports are compared; one-sided metrics are skipped (noted on stderr).

### GPU exerciser

Generate sustained GPU load for testing (`gpuexercise` is a subcommand, not a separate binary):

```bash
bgpucap gpuexercise --percent 50 --seconds 10
bgpucap gpuexercise -p 75 -s 5 -f json
bgpucap gpuexercise --mode sample -s 5    # ambient measurement only
bgpucap gpuexercise --mode load -p 80 -s 5  # always generate load
```

**Modes** (`--mode`):

| Mode | Behavior |
|------|----------|
| `best-effort` (default) | Skip GPU load if target ≤ ambient; suggests a reachable `--percent` |
| `load` | Always generate Metal load to chase target |
| `sample` | No load; measure ambient GPU for the duration |

Also accepts `-f`, `--metrics`, color, and output formatting options.

### Color output

Follows BrightDate / bright-iputils conventions:

- `--color[=WHEN]` — `auto`, `always`, `never`, `plain`, `ansi`, `truecolor`
- `--no-color`
- `--color-scheme=SCHEME` — `default` or `bright`
- Environment: `BGPUCAP_COLOR`, `BGPUCAP_COLOR_SCHEME` (`GPUCAP_*` still accepted), plus standard `NO_COLOR` / `CLICOLOR`

## Metrics

| Metric | Source |
|--------|--------|
| GPU % | IOKit `Device Utilization %` (IOReport GPUPH fallback) |
| GPU memory | IOKit `PerformanceStatistics` (in use / allocated bytes) |
| Renderer / tiler | IOKit `Renderer/Tiler Utilization %` |
| GPU frequency | IOReport GPUPH + pmgr `voltage-states9` DVFS table |
| GPU temperature | IOReport `Tg*a Max` sensors |
| GPU throttle | IOReport `GPU_CLTM` (CLTM-induced perf states) |
| GPU / CPU / DRAM / ANE power | IOReport Energy Model (`GPU Energy`, `CPU Energy`, `DRAM`, `ANE`/`ANE0`) |
| CPU % | Mach `host_processor_info` (system-wide) |
| CPU frequency | IOReport ECPU/PCPU + pmgr DVFS tables (per-cluster + blended) |
| Memory % | Unified RAM via `host_statistics64` + `hw.memsize` |
| Wired / compressed / swap | `host_statistics64`, `vm.swapusage` |
| Memory pressure | `vm.memory_pressure` |

Extended metrics are **validated on Apple M4 Max** only (project test hardware). Other Apple Silicon chips (M1–M4 variants) report best-effort values; a footnote appears when validation status is not `validated`. Chip family is detected from `machdep.cpu.brand_string` for JSON output (`chip.family`: `m1`/`m2`/`m3`/`m4`).

Samples are taken every `--interval` ms (default 100) while the child runs.

## Install

### Cargo

```sh
cargo install gpucap
bgpucap --version
```

### Homebrew

```sh
brew tap digital-defiance/tap
brew install digital-defiance/tap/gpucap
bgpucap sleep 1
man bgpucap
# gpucap is also available as a symlink to bgpucap
```

`cargo install gpucap` installs the `bgpucap` binary only; copy `man/bgpucap.1` from the crate source to your man path if you want the man page without Homebrew.

If Homebrew reports the formula in multiple taps, remove the local dev tap:

```sh
brew untap digital-defiance/tap-local
```

## Release

```sh
./scripts/release.sh patch          # bump, test, publish, update homebrew formula
./scripts/release.sh 0.2.0 --dry-run
```

Requires `cargo login` and a checkout of [homebrew-tap](https://github.com/Digital-Defiance/homebrew-tap) at `/Volumes/Code/homebrew-tap` (override with `HOMEBREW_TAP_FORMULA`).

## License

MIT — see [LICENSE](LICENSE).
