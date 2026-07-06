const NET_DENYLIST: [&str; 10] = [
    "socket",   //
    "connect",  //
    "accept",   //
    "accept4",  //
    "bind",     //
    "listen",   //
    "sendto",   //
    "recvfrom", //
    "sendmsg",  //
    "recvmsg",  //
];

const PROC_DENYLIST: [&str; 7] = [
    "fork",    //
    "vfork",   //
    "clone",   //
    "clone3",  //
    "ptrace",  //
    "unshare", //
    "setns",   //
];

const FS_DENYLIST: [&str; 7] = [
    "unlink",   //
    "unlinkat", //
    "rmdir",    //
    "rename",   //
    "renameat", //
    "chmod",    //
    "fchmod",   //
];

#[cfg(target_os = "linux")]
pub fn apply_sandbox() -> Result<(), String> {
    use libseccomp::{ScmpAction, ScmpFilterContext, error::SeccompError};
    let closure = |error: SeccompError| format!("Failed to intialise BPF context: {error}");
    let mut filter = ScmpFilterContext::new(ScmpAction::Allow).map_err(closure)?;
    register_rules(&mut filter, &NET_DENYLIST, ScmpAction::KillProcess)?;
    register_rules(&mut filter, &PROC_DENYLIST, ScmpAction::KillProcess)?;
    register_rules(&mut filter, &FS_DENYLIST, ScmpAction::Errno(libc::EPERM))?;
    let closure = |error: SeccompError| format!("Failed to commit seccomp filter: {error}");
    filter.load().map_err(closure)?;
    Ok(())
}

fn register_rules(
    filter: &mut libseccomp::ScmpFilterContext,
    syscalls: &[&str],
    action: libseccomp::ScmpAction,
) -> Result<(), String> {
    for &syscall in syscalls {
        if let Ok(syscall) = libseccomp::ScmpSyscall::from_name(syscall) {
            let closure = |error: libseccomp::error::SeccompError| {
                format!("Failed to add rule for '{syscall}: {error}")
            };
            filter.add_rule(action, syscall).map_err(closure);
        }
    }
    Ok(())
}
