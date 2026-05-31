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
bgpucap --color=bright --color-scheme=bright make -j8
bgpucap --interval 50 cargo build
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
bgpucap -f '%gA,%gP,%uA,%uP,%hA,%hP,%e,%Ws,%Wt' sleep 1
bgpucap gpuexercise -f 'target=%tG gpu=%gA/%gP elapsed=%e' --percent 50 --seconds 5
```

Environment: `BGPUCAP_FORMAT` (same as `-f`; `GPUCAP_FORMAT` still accepted).

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
bgpucap gpuexercise --percent 50 --seconds 10
bgpucap gpuexercise -p 75 -s 5 -f 'target=%tG gpu=%gA/%gP'
```

### Color output

Follows BrightDate / bright-iputils conventions:

- `--color[=WHEN]` — `auto`, `always`, `never`, `plain`, `ansi`, `truecolor`
- `--no-color`
- `--color-scheme=SCHEME` — `default` or `bright`
- Environment: `BGPUCAP_COLOR`, `BGPUCAP_COLOR_SCHEME` (`GPUCAP_*` still accepted), plus standard `NO_COLOR` / `CLICOLOR`

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
