# gpucap

Run a command like `time(1)` and report **GPU**, **CPU**, and **unified memory** utilization on Apple Silicon Macs.

**Documentation:** [gpucap docs](https://digital-defiance.github.io/gpucap/) · [crates.io](https://crates.io/crates/gpucap)

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
gpucap --interval 50 cargo build
```

Example output (stderr, colored on TTY):

```
gpu      avg 12.3%  peak 45.6%
cpu      avg  8.1%  peak 23.4%
memory   avg 52.0%  peak 55.1%
real     1.234567 s
```

### Custom format (`-f` / `--format`)

Machine-readable output using [BrightDate FORMAT-SPEC](https://github.com/Digital-Defiance/brightdate-rust/blob/main/FORMAT-SPEC.md) conventions where applicable. Format output is always plain text (no color), even with `--color=always`.

```bash
gpucap -f '%gA,%gP,%uA,%uP,%hA,%hP,%e,%Ws,%Wt' sleep 1
gpucap gpuexercise -f 'target=%tG gpu=%gA/%gP elapsed=%e' --percent 50 --seconds 5
```

Environment: `GPUCAP_FORMAT` (same as `-f`).

| Specifier | Meaning |
|-----------|---------|
| `%gA` / `%gP` | GPU average / peak % |
| `%uA` / `%uP` | CPU average / peak % |
| `%hA` / `%hP` | Memory average / peak % |
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

### GPU exerciser

Generate sustained GPU load for testing (`gpuexercise` is a subcommand, not a separate binary):

```bash
gpucap gpuexercise --percent 50 --seconds 10
gpucap gpuexercise -p 75 -s 5 -f 'target=%tG gpu=%gA/%gP'
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

Samples are taken every `--interval` ms (default 100) while the child runs.

## Install

### Cargo

```sh
cargo install gpucap
```

### Homebrew

```sh
brew tap digital-defiance/tap
brew install digital-defiance/tap/gpucap
```

## License

MIT — see [LICENSE](LICENSE).
