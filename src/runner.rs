use crate::metrics::{MetricSnapshot, PercentStats, Sampler};
use std::io;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct RunResult {
    pub wait_status: i32,
    pub elapsed_secs: f64,
    pub gpu: PercentStats,
    pub cpu: PercentStats,
    pub memory: PercentStats,
}

pub fn run_command(cmd: &[&str], interval: Duration) -> io::Result<RunResult> {
    if cmd.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "missing command",
        ));
    }

    let start = Instant::now();
    let mut sampler = Sampler::new();
    let mut gpu = PercentStats::default();
    let mut cpu = PercentStats::default();
    let mut memory = PercentStats::default();

    let wait_status = unsafe {
        let interrupt = libc::signal(libc::SIGINT, libc::SIG_IGN);
        let quit = libc::signal(libc::SIGQUIT, libc::SIG_IGN);

        let pid = libc::fork();
        if pid < 0 {
            libc::signal(libc::SIGINT, interrupt);
            libc::signal(libc::SIGQUIT, quit);
            return Err(io::Error::last_os_error());
        }

        if pid == 0 {
            let c_strings: Vec<std::ffi::CString> = cmd
                .iter()
                .map(|arg| std::ffi::CString::new(*arg))
                .collect::<Result<_, _>>()
                .unwrap_or_else(|_| libc::_exit(126));

            let mut argv: Vec<*const libc::c_char> =
                c_strings.iter().map(|s| s.as_ptr()).collect();
            argv.push(std::ptr::null());

            libc::execvp(c_strings[0].as_ptr(), argv.as_ptr());
            let code = if io::Error::last_os_error().kind() == io::ErrorKind::NotFound {
                127
            } else {
                126
            };
            libc::_exit(code);
        }

        let mut status = 0;
        loop {
            let snapshot = sampler.sample();
            record_snapshot(&mut gpu, &mut cpu, &mut memory, &snapshot);

            let waited = libc::waitpid(pid, &mut status, libc::WNOHANG);
            if waited < 0 {
                let err = io::Error::last_os_error();
                libc::signal(libc::SIGINT, interrupt);
                libc::signal(libc::SIGQUIT, quit);
                return Err(err);
            }
            if waited == pid {
                break;
            }

            std::thread::sleep(interval);
        }

        libc::signal(libc::SIGINT, interrupt);
        libc::signal(libc::SIGQUIT, quit);
        status
    };

    Ok(RunResult {
        wait_status,
        elapsed_secs: start.elapsed().as_secs_f64(),
        gpu,
        cpu,
        memory,
    })
}

fn record_snapshot(
    gpu: &mut PercentStats,
    cpu: &mut PercentStats,
    memory: &mut PercentStats,
    snapshot: &MetricSnapshot,
) {
    gpu.record(snapshot.gpu);
    cpu.record(snapshot.cpu);
    memory.record(snapshot.memory);
}

pub fn wait_status_to_exit_code(status: i32) -> i32 {
    #[cfg(unix)]
    unsafe {
        if libc::WIFSTOPPED(status) {
            return libc::WSTOPSIG(status) + 128;
        }
        if libc::WIFSIGNALED(status) {
            return libc::WTERMSIG(status) + 128;
        }
        if libc::WIFEXITED(status) {
            return libc::WEXITSTATUS(status);
        }
        return 1;
    }
    #[cfg(not(unix))]
    {
        status
    }
}
