//! Low-frequency resource sampling; no subprocesses or system-wide process scans.
use std::time::{Duration, Instant};

const INTERVAL: Duration = Duration::from_secs(2);

#[derive(Clone, Copy)]
struct Sample {
    cpu_seconds: f64,
    memory_bytes: u64,
}

#[derive(Clone, Copy)]
enum Scope {
    Process,
    #[cfg(target_os = "linux")]
    Container,
}

pub struct Monitor {
    scope: Scope,
    previous: Option<Sample>,
    sampled_at: Instant,
    started_at: Instant,
    pub cpu_percent: Option<f64>,
    pub memory_bytes: Option<u64>,
}

impl Monitor {
    pub fn new() -> Self {
        let scope = Scope::Process;
        #[cfg(target_os = "linux")]
        let scope = if std::env::var("BROWSER_METRICS_SCOPE").as_deref() == Ok("container")
            && container_sample().is_some()
        {
            Scope::Container
        } else {
            scope
        };
        let initial = sample(scope);
        let now = Instant::now();
        Self {
            scope,
            previous: initial,
            sampled_at: now,
            started_at: now,
            cpu_percent: None,
            memory_bytes: initial.map(|s| s.memory_bytes),
        }
    }

    pub fn refresh(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.sampled_at);
        if elapsed < INTERVAL {
            return false;
        }
        let current = sample(self.scope);
        self.cpu_percent = current.zip(self.previous).and_then(|(new, old)| {
            cpu_percent(old.cpu_seconds, new.cpu_seconds, elapsed.as_secs_f64())
        });
        self.memory_bytes = current.map(|s| s.memory_bytes);
        self.previous = current;
        self.sampled_at = now;
        true
    }

    pub fn label(&self) -> String {
        let cpu = self
            .cpu_percent
            .map_or_else(|| "—".into(), |v| format!("{v:.1}%"));
        let ram = self
            .memory_bytes
            .map_or_else(|| "—".into(), |v| format!("{} MiB", v / 1_048_576));
        format!("CPU {cpu} · RAM {ram}")
    }

    pub fn description(&self) -> String {
        let scope = match self.scope {
            Scope::Process => {
                "Browser process, including all its tabs and engine threads. Separate content processes are excluded."
            }
            #[cfg(target_os = "linux")]
            Scope::Container => {
                "Whole Docker container, including the browser, display and viewer. RAM excludes inactive file cache, as in docker stats."
            }
        };
        let seconds = self.started_at.elapsed().as_secs();
        format!(
            "{scope}\nUpdated every 2 seconds. CPU: 100% = one fully used core; multiple cores can exceed 100%.\nRAM is current resident/working memory, not peak usage.\nBrowser uptime: {}m {:02}s",
            seconds / 60,
            seconds % 60
        )
    }
}

fn cpu_percent(previous: f64, current: f64, seconds: f64) -> Option<f64> {
    if current < previous || seconds <= 0.0 {
        return None;
    }
    let value = (current - previous) / seconds * 100.0;
    value.is_finite().then_some(value)
}

fn sample(scope: Scope) -> Option<Sample> {
    match scope {
        Scope::Process => process_sample(),
        #[cfg(target_os = "linux")]
        Scope::Container => container_sample(),
    }
}

#[cfg(unix)]
fn process_sample() -> Option<Sample> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    // SAFETY: getrusage initializes this correctly sized output on success.
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } != 0 {
        return None;
    }
    let usage = unsafe { usage.assume_init() };
    let cpu_seconds = usage.ru_utime.tv_sec as f64
        + usage.ru_stime.tv_sec as f64
        + (usage.ru_utime.tv_usec + usage.ru_stime.tv_usec) as f64 / 1_000_000.0;
    Some(Sample {
        cpu_seconds,
        memory_bytes: resident_memory()?,
    })
}

#[cfg(not(unix))]
fn process_sample() -> Option<Sample> {
    None
}

#[cfg(target_os = "linux")]
fn resident_memory() -> Option<u64> {
    let statm = std::fs::read_to_string("/proc/self/statm").ok()?;
    let pages: u64 = statm.split_whitespace().nth(1)?.parse().ok()?;
    // SAFETY: sysconf has no pointer arguments or side effects here.
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    (page_size > 0)
        .then(|| pages.checked_mul(page_size as u64))
        .flatten()
}

#[cfg(target_os = "macos")]
fn resident_memory() -> Option<u64> {
    let mut info = std::mem::MaybeUninit::<libc::rusage_info_v0>::uninit();
    // SAFETY: proc_pid_rusage writes the selected v0 structure into this buffer.
    let result = unsafe {
        libc::proc_pid_rusage(
            libc::getpid(),
            libc::RUSAGE_INFO_V0,
            info.as_mut_ptr().cast(),
        )
    };
    if result != 0 {
        return None;
    }
    Some(unsafe { info.assume_init() }.ri_resident_size)
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
fn resident_memory() -> Option<u64> {
    None
}

#[cfg(any(target_os = "linux", test))]
fn counter(text: &str, key: &str) -> Option<u64> {
    text.lines().find_map(|line| {
        let mut values = line.split_whitespace();
        (values.next()? == key)
            .then(|| values.next()?.parse().ok())
            .flatten()
    })
}

#[cfg(target_os = "linux")]
fn container_sample() -> Option<Sample> {
    let cpu = std::fs::read_to_string("/sys/fs/cgroup/cpu.stat").ok()?;
    let memory = std::fs::read_to_string("/sys/fs/cgroup/memory.current").ok()?;
    let stats = std::fs::read_to_string("/sys/fs/cgroup/memory.stat").ok()?;
    container_usage(&cpu, &memory, &stats)
}

#[cfg(any(target_os = "linux", test))]
fn container_usage(cpu: &str, memory: &str, stats: &str) -> Option<Sample> {
    Some(Sample {
        cpu_seconds: counter(cpu, "usage_usec")? as f64 / 1_000_000.0,
        memory_bytes: memory
            .trim()
            .parse::<u64>()
            .ok()?
            .saturating_sub(counter(stats, "inactive_file")?),
    })
}

#[cfg(test)]
#[path = "resources_tests.rs"]
mod tests;
