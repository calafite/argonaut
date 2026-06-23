#[cfg(target_os = "linux")]
pub fn apply_sandbox() -> Result<(), String> {
    use libseccomp::{ScmpAction, ScmpFilterContext, ScmpSyscall};

    let mut filter = ScmpFilterContext::new(ScmpAction::Allow)
        .map_err(|e| format!("Failed to initialize BPF context: {e}"))?;

    let net_denylist = [
        "socket", "connect", "accept", "accept4", "bind", "listen", "sendto", "recvfrom",
        "sendmsg", "recvmsg",
    ];
    for sc in net_denylist {
        if let Ok(syscall) = ScmpSyscall::from_name(sc) {
            let _ = filter.add_rule(ScmpAction::KillProcess, syscall);
        }
    }

    let proc_denylist = [
        "fork", "vfork", "clone", "clone3", "ptrace", "unshare", "setns",
    ];
    for sc in proc_denylist {
        if let Ok(syscall) = ScmpSyscall::from_name(sc) {
            let _ = filter.add_rule(ScmpAction::KillProcess, syscall);
        }
    }

    let fs_denylist = [
        "unlink", "unlinkat", "rmdir", "rename", "renameat", "chmod", "fchmod",
    ];
    for sc in fs_denylist {
        if let Ok(syscall) = ScmpSyscall::from_name(sc) {
            let _ = filter.add_rule(ScmpAction::Errno(libc::EPERM), syscall);
        }
    }

    filter
        .load()
        .map_err(|e| format!("Failed to commit seccomp filter: {e}"))?;
    Ok(())
}
