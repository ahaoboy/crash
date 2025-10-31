use std::{ffi::OsStr, process::Stdio};

pub fn exec<S: AsRef<OsStr>>(cmd: S, args: Vec<&str>) -> anyhow::Result<String> {
    let cmd = cmd.as_ref();
    let output = std::process::Command::new(cmd)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .output()?;
    let s = String::from_utf8_lossy(&output.stdout);
    Ok(s.to_string())
}
