use std::io;
use std::path::Path;
use std::process::Command;

#[cfg(target_os = "windows")]
pub fn stop_process(name_or_path: &str) -> io::Result<()> {
    let process_name = Path::new(name_or_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(name_or_path);

    let output = Command::new("cmd")
        .args(&["/c", &format!("taskkill /IM {process_name} /F")])
        .output()?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(io::Error::new(io::ErrorKind::Other, stderr.to_string()))
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub fn stop_process(name_or_path: &str) -> io::Result<()> {
    let process_name = Path::new(name_or_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(name_or_path);

    if let Ok(output) = Command::new("pkill").args(&["-f", process_name]).output() {
        if output.status.success() {
            return Ok(());
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(io::Error::new(io::ErrorKind::Other, stderr.to_string()));
        }
    }

    if let Ok(output) = Command::new("killall").arg(process_name).output() {
        if output.status.success() {
            return Ok(());
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(io::Error::new(io::ErrorKind::Other, stderr.to_string()));
        }
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "Neither 'pkill' nor 'killall' commands are available on this system",
    ))
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
pub fn stop_process(_name_or_path: &str) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Other,
        "Unsupported operating system",
    ))
}
