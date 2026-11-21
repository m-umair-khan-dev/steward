//! Local servers (P7-T05; DESIGN.md §4.2): ports listening on this machine
//! that belong to the user's own processes, read from `/proc`. Stop sends a
//! normal terminate signal (the app asks first). Other users' processes are
//! never listed or signalled.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Server {
    pub port: u16,
    pub pid: u32,
    /// The program's name ("python3", "node").
    pub program: String,
    /// Its command line, for the tooltip.
    pub command: String,
    /// The folder it runs in.
    pub cwd: Option<PathBuf>,
    /// When it started (seconds since the epoch).
    pub started: Option<i64>,
}

/// Listening sockets in `/proc/net/tcp` or `tcp6`: (port, inode).
/// Only loopback and any-address listeners count as local servers.
pub fn parse_net_tcp(text: &str) -> Vec<(u16, u64)> {
    text.lines()
        .skip(1)
        .filter_map(|line| {
            let f: Vec<&str> = line.split_whitespace().collect();
            let (local, state, inode) = (f.get(1)?, f.get(3)?, f.get(9)?);
            if *state != "0A" {
                return None; // not LISTEN
            }
            let (addr, port) = local.split_once(':')?;
            let local_only = addr.chars().all(|c| c == '0')
                || addr == "0100007F"
                || addr == "00000000000000000000000001000000";
            if !local_only {
                return None;
            }
            Some((u16::from_str_radix(port, 16).ok()?, inode.parse().ok()?))
        })
        .collect()
}

/// The start time of a process from `/proc/<pid>/stat` (field 22, in clock
/// ticks after boot), given the boot time and ticks per second.
pub fn start_time(stat: &str, boot: i64, ticks: i64) -> Option<i64> {
    // The command name (field 2) is in parentheses and may contain spaces.
    let after = &stat[stat.rfind(')')? + 2..];
    let field22: i64 = after.split_whitespace().nth(19)?.parse().ok()?;
    Some(boot + field22 / ticks.max(1))
}

fn boot_time() -> Option<i64> {
    std::fs::read_to_string("/proc/stat")
        .ok()?
        .lines()
        .find_map(|l| l.strip_prefix("btime ")?.trim().parse().ok())
}

/// The user's processes that listen on a local port, from `proc` (normally
/// "/proc"; tests pass another folder).
pub fn listening(proc: &Path) -> Vec<Server> {
    let mut sockets: HashMap<u64, u16> = HashMap::new();
    for table in ["net/tcp", "net/tcp6"] {
        if let Ok(text) = std::fs::read_to_string(proc.join(table)) {
            for (port, inode) in parse_net_tcp(&text) {
                sockets.entry(inode).or_insert(port);
            }
        }
    }
    if sockets.is_empty() {
        return Vec::new();
    }
    let uid = rustix::process::getuid().as_raw();
    let boot = boot_time().unwrap_or(0);
    let ticks = 100; // USER_HZ on Linux
    let mut out: Vec<Server> = Vec::new();
    let Ok(entries) = std::fs::read_dir(proc) else { return out };
    for entry in entries.flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else { continue };
        let dir = entry.path();
        // Only the user's own processes.
        let owned = std::os::unix::fs::MetadataExt::uid(&match dir.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        }) == uid;
        if !owned {
            continue;
        }
        let Ok(fds) = std::fs::read_dir(dir.join("fd")) else { continue };
        let mut ports: Vec<u16> = fds
            .flatten()
            .filter_map(|fd| std::fs::read_link(fd.path()).ok())
            .filter_map(|target| {
                let t = target.to_string_lossy().into_owned();
                let inode: u64 = t.strip_prefix("socket:[")?.strip_suffix(']')?.parse().ok()?;
                sockets.get(&inode).copied()
            })
            .collect();
        ports.sort_unstable();
        ports.dedup();
        if ports.is_empty() {
            continue;
        }
        let cmdline = std::fs::read(dir.join("cmdline")).unwrap_or_default();
        let words: Vec<String> = cmdline
            .split(|b| *b == 0)
            .filter(|w| !w.is_empty())
            .map(|w| String::from_utf8_lossy(w).into_owned())
            .collect();
        let program = std::fs::read_to_string(dir.join("comm"))
            .map(|c| c.trim().to_string())
            .unwrap_or_default();
        let started = std::fs::read_to_string(dir.join("stat"))
            .ok()
            .and_then(|s| start_time(&s, boot, ticks));
        for port in ports {
            out.push(Server {
                port,
                pid,
                program: program.clone(),
                command: words.join(" "),
                cwd: std::fs::read_link(dir.join("cwd")).ok(),
                started,
            });
        }
    }
    out.sort_by_key(|s| (s.port, s.pid));
    out.dedup_by_key(|s| s.port);
    out
}

/// Asks a server to stop (SIGTERM). Only the user's own processes can be
/// signalled; the kernel refuses anything else.
pub fn stop(pid: u32) -> std::io::Result<()> {
    let pid = rustix::process::Pid::from_raw(pid as i32)
        .ok_or_else(|| std::io::Error::other("no such process"))?;
    rustix::process::kill_process(pid, rustix::process::Signal::TERM).map_err(std::io::Error::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listening_sockets_on_local_addresses() {
        let text = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n\
           0: 0100007F:0BB8 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 41234 1 0\n\
           1: 00000000:1F40 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 41235 1 0\n\
           2: 0100007F:0BB8 0100007F:D431 01 00000000:00000000 00:00000000 00000000  1000        0 41236 1 0\n\
           3: C0A80002:0016 00000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 41237 1 0\n";
        assert_eq!(parse_net_tcp(text), vec![(3000, 41234), (8000, 41235)],
                   "listening on loopback or any address; not connections, not a LAN address only");
    }

    #[test]
    fn start_time_from_stat() {
        let stat = "4242 (my server) S 1 4242 4242 0 -1 4194560 1 0 0 0 5 2 0 0 20 0 1 0 12345 1000 100";
        assert_eq!(start_time(stat, 1_000_000, 100), Some(1_000_000 + 123));
    }

    #[test]
    fn this_process_is_found_when_it_listens() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let found = listening(Path::new("/proc"));
        let me = found.iter().find(|s| s.port == port).expect("our own listener");
        assert_eq!(me.pid, std::process::id());
    }
}
