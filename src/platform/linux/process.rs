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
        Ok(_) => true,
        Err(err) => err != Errno::ESRCH,
    }
}

fn signal(pid: u32, sig: Signal, context: &str) -> Result<bool> {
    match kill(Pid::from_raw(pid as i32), sig) {
        Ok(_) => Ok(true),
        Err(Errno::ESRCH) => Ok(false),
        Err(err) => bail!("{context}: {err}"),
    }
}
