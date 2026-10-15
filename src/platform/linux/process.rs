use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use nix::errno::Errno;
use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;

pub fn suspend(pid: u32) -> Result<bool> {
    signal(pid, Signal::SIGSTOP, "暂停录屏失败")
}

pub fn resume(pid: u32) -> Result<bool> {
    signal(pid, Signal::SIGCONT, "恢复录屏失败")
}

pub fn interrupt(pid: u32) -> Result<bool> {
    signal(pid, Signal::SIGINT, "发送停止信号失败")
}

pub fn is_running(pid: u32) -> bool {
    match kill(Pid::from_raw(pid as i32), None) {
        Ok(_) => !is_zombie(pid),
        Err(err) => err != Errno::ESRCH,
    }
}

pub fn wait_for_exit(pid: u32, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while is_running(pid) {
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    true
}

fn is_zombie(pid: u32) -> bool {
    let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
        return false;
    };
    let Some((_, fields)) = stat.rsplit_once(") ") else {
        return false;
    };
    fields.starts_with('Z')
}

fn signal(pid: u32, sig: Signal, context: &str) -> Result<bool> {
    match kill(Pid::from_raw(pid as i32), sig) {
        Ok(_) => Ok(true),
        Err(Errno::ESRCH) => Ok(false),
        Err(err) => bail!("{context}: {err}"),
    }
}
