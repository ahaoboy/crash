// TUN device preparation for the proxy core.
//
// Mihomo/Clash's TUN + `auto-route` mode needs the `tun` kernel module and a
// usable `/dev/net/tun` character device (major 10, minor 200). Both normally
// require root, so this module detects admin rights with the `is-admin` crate
// and then makes a best-effort attempt with whatever privileges are available.
//
// This replaces the equivalent shell snippet with Rust/system APIs:
//   - module presence    -> read `/proc/modules`   (instead of `lsmod | grep tun`)
//   - loading the module -> `modprobe tun`
//   - device existence   -> `Path::exists()`       (instead of `ls -l /dev/net`)
//   - device creation    -> `libc::mknod`          (instead of `mknod`)
//   - device permissions -> `libc::chmod`          (instead of `chmod`)
//
// Every failure is only logged, never propagated: a missing TUN device must
// not abort a start that might otherwise succeed (e.g. a non-root user with a
// non-TUN config, or a platform where TUN is handled differently).

/// Whether a Mihomo/Clash YAML config enables TUN.
///
/// Looks for a top-level `tun:` mapping and, inside its (indented) block, for
/// `enable: true`. Deliberately a small hand-rolled scan — the same approach
/// `patcher::patch_config` uses to detect an existing `tun` block — so the
/// crate does not need a full YAML parser just for this single boolean.
///
/// Unreadable/absent keys are treated as disabled, so a malformed config can
/// never force device setup on.
pub fn tun_enabled_yaml(config: &str) -> bool {
    let mut in_tun = false;
    let mut enabled = false;

    for raw in config.lines() {
        // Drop a trailing inline comment, but only when the `#` starts a
        // comment (preceded by whitespace) rather than appearing inside a
        // value such as a quoted URL.
        let line = match raw.split_once('#') {
            Some((before, _)) if before.is_empty() || before.ends_with(' ') => before,
            _ => raw,
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let indent = line.len() - line.trim_start().len();

        if in_tun {
            if indent == 0 {
                // Left the `tun` block; fall through to treat this as the
                // next top-level key.
                in_tun = false;
            } else {
                if let Some(value) = trimmed.strip_prefix("enable:") {
                    enabled = parse_bool(value).unwrap_or(false);
                }
                continue;
            }
        }

        if indent == 0 && trimmed == "tun:" {
            in_tun = true;
        }
    }

    enabled
}

/// Whether a sing-box JSON config declares an enabled `tun` inbound.
pub fn tun_enabled_json(config: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(config) else {
        return false;
    };

    value
        .get("inbounds")
        .and_then(|inbounds| inbounds.as_array())
        .is_some_and(|inbounds| {
            inbounds.iter().any(|inbound| {
                inbound.get("type").and_then(|t| t.as_str()) == Some("tun")
                    && inbound
                        .get("enabled")
                        .and_then(|e| e.as_bool())
                        .unwrap_or(true)
            })
        })
}

/// Parse a YAML scalar as a boolean, accepting the usual truthy/falsey spellings.
fn parse_bool(value: &str) -> Option<bool> {
    let token = value.split_whitespace().next().unwrap_or("");
    let token = token.trim_matches(|c| c == '"' || c == '\'' || c == ',');

    match token.to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" => Some(true),
        "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// Ensure the `tun` kernel module and `/dev/net/tun` device are ready for TUN.
///
/// Best-effort and side-effect only: this never returns an error, so callers
/// can invoke it unconditionally before starting the core. On platforms other
/// than Linux it is a no-op.
pub fn ensure_tun_device() {
    #[cfg(target_os = "linux")]
    linux::ensure_tun_device();

    #[cfg(not(target_os = "linux"))]
    crate::log_info!("Skipping TUN device setup: only supported on Linux");
}

#[cfg(target_os = "linux")]
mod linux {
    use crate::utils::command::execute;
    use crate::{log_info, log_warn};
    use std::ffi::CString;
    use std::io::ErrorKind;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    const TUN_MODULE: &str = "tun";
    const TUN_DEVICE: &str = "/dev/net/tun";
    const TUN_DEVICE_DIR: &str = "/dev/net";
    /// Linux TUN device numbers: `/dev/net/tun` is character device 10:200.
    const TUN_MAJOR: libc::c_uint = 10;
    const TUN_MINOR: libc::c_uint = 200;
    /// `rw-rw-rw-` — matches the shell setup this module replaces.
    const TUN_MODE: libc::mode_t = 0o666;

    pub(super) fn ensure_tun_device() {
        log_info!(
            "Preparing TUN device (admin: {})",
            if is_admin::is_admin() { "yes" } else { "no" }
        );

        ensure_tun_module();
        ensure_tun_device_node();
    }

    /// Load the `tun` module when it is not already present.
    fn ensure_tun_module() {
        if tun_module_loaded() {
            log_info!("Kernel module '{}' already loaded", TUN_MODULE);
            return;
        }

        log_info!(
            "Kernel module '{}' not loaded, running modprobe",
            TUN_MODULE
        );
        match execute("modprobe", &[TUN_MODULE]) {
            Ok(_) => log_info!("Kernel module '{}' loaded", TUN_MODULE),
            Err(e) => log_warn!(
                "Failed to load kernel module '{}': {} (continuing)",
                TUN_MODULE,
                e
            ),
        }
    }

    /// Whether `tun` is listed in `/proc/modules` — the data source `lsmod` reads.
    ///
    /// Returns `false` when `/proc/modules` cannot be read so the caller still
    /// attempts `modprobe` (which is harmless when the module is built in or
    /// already loaded).
    fn tun_module_loaded() -> bool {
        match std::fs::read_to_string("/proc/modules") {
            Ok(modules) => modules
                .lines()
                .any(|line| line.split_whitespace().next() == Some(TUN_MODULE)),
            Err(e) => {
                log_warn!(
                    "Failed to read /proc/modules: {} (will try modprobe anyway)",
                    e
                );
                false
            }
        }
    }

    /// Create `/dev/net/tun` when it is missing, then normalise its permissions.
    fn ensure_tun_device_node() {
        let path = Path::new(TUN_DEVICE);

        if !path.exists() {
            log_info!("TUN device {} missing, creating it", TUN_DEVICE);
            match create_tun_device(path) {
                Ok(()) => log_info!("Created TUN device {}", TUN_DEVICE),
                Err(e) => {
                    // Non-root users typically lack permission to `mknod` here.
                    log_warn!(
                        "Failed to create TUN device {}: {} (continuing)",
                        TUN_DEVICE,
                        e
                    );
                    return;
                }
            }
        }

        ensure_tun_permissions(path);
    }

    /// `mknod /dev/net/tun c 10 200`, creating the parent directory if needed.
    fn create_tun_device(path: &Path) -> std::io::Result<()> {
        // `create_dir_all` is a no-op when the directory already exists.
        std::fs::create_dir_all(TUN_DEVICE_DIR)?;

        let c_path = CString::new(path.as_os_str().as_encoded_bytes())
            .map_err(|e| std::io::Error::new(ErrorKind::InvalidInput, e))?;

        // SAFETY: `c_path` is a valid NUL-terminated C string; the mode and dev
        // arguments are plain integers, so no other precondition exists.
        let rc = unsafe {
            libc::mknod(
                c_path.as_ptr(),
                libc::S_IFCHR | TUN_MODE,
                libc::makedev(TUN_MAJOR, TUN_MINOR),
            )
        };

        if rc != 0 {
            return Err(std::io::Error::last_os_error());
        }

        Ok(())
    }

    /// Apply `rw-rw-rw-` to the device when it is not already set.
    fn ensure_tun_permissions(path: &Path) {
        let Ok(metadata) = std::fs::metadata(path) else {
            log_warn!("Failed to read metadata of {} (continuing)", TUN_DEVICE);
            return;
        };

        let mode = metadata.permissions().mode() & 0o777;
        if mode == TUN_MODE {
            log_info!("TUN device {} already has mode {:o}", TUN_DEVICE, mode);
            return;
        }

        log_info!(
            "TUN device {} has mode {:o}, setting to {:o}",
            TUN_DEVICE,
            mode,
            TUN_MODE
        );

        let Ok(c_path) = CString::new(path.as_os_str().as_encoded_bytes()) else {
            log_warn!("TUN device path contains a NUL byte (continuing)");
            return;
        };

        // SAFETY: `c_path` is a valid NUL-terminated C string.
        let rc = unsafe { libc::chmod(c_path.as_ptr(), TUN_MODE) };
        if rc != 0 {
            log_warn!(
                "Failed to chmod {} to {:o}: {} (continuing)",
                TUN_DEVICE,
                TUN_MODE,
                std::io::Error::last_os_error()
            );
        } else {
            log_info!("Set TUN device {} mode to {:o}", TUN_DEVICE, TUN_MODE);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaml_enabled() {
        assert!(tun_enabled_yaml("tun:\n  enable: true\n  stack: gVisor\n"));
        assert!(tun_enabled_yaml("port: 7890\ntun:\n  enable: true\n"));
        assert!(tun_enabled_yaml("tun:\n  enable: \"true\"\n"));
    }

    #[test]
    fn yaml_disabled_or_absent() {
        assert!(!tun_enabled_yaml("port: 7890\n"));
        assert!(!tun_enabled_yaml("tun:\n  enable: false\n"));
        // `enable` outside the `tun` block must not count.
        assert!(!tun_enabled_yaml("other:\n  enable: true\n"));
        // A later top-level key ends the `tun` block.
        assert!(!tun_enabled_yaml(
            "tun:\n  stack: gVisor\nother:\n  enable: true\n"
        ));
    }

    #[test]
    fn yaml_handles_comments_and_spacing() {
        assert!(tun_enabled_yaml(
            "tun: # enable tun\n  enable: true # yes\n"
        ));
        // Not a top-level `tun:` key.
        assert!(!tun_enabled_yaml("  tun:\n    enable: true\n"));
    }

    #[test]
    fn json_enabled() {
        assert!(tun_enabled_json(
            r#"{"inbounds":[{"type":"tun","enabled":true}]}"#
        ));
        // `enabled` omitted defaults to true in sing-box.
        assert!(tun_enabled_json(r#"{"inbounds":[{"type":"tun"}]}"#));
    }

    #[test]
    fn json_disabled_or_absent() {
        assert!(!tun_enabled_json(
            r#"{"inbounds":[{"type":"tun","enabled":false}]}"#
        ));
        assert!(!tun_enabled_json(r#"{"inbounds":[{"type":"mixed"}]}"#));
        assert!(!tun_enabled_json("not json"));
    }
}
