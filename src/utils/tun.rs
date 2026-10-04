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
// The core config's `tun` section is inspected with `serde-saphyr` (YAML) and
// `serde_json` (JSON) so the check is a real parse rather than string matching.
//
// Every failure is only logged, never propagated: a missing TUN device must
// not abort a start that might otherwise succeed (e.g. a non-root user with a
// non-TUN config, or a platform where TUN is handled differently).

use serde::Deserialize;

/// Typed view of the Mihomo/Clash `tun` section.
///
/// Parsing is type-driven (`serde-saphyr` uses the Rust types as the schema),
/// so only the single field we care about needs declaring; every other key in
/// the real config is ignored.
#[derive(Deserialize)]
struct MihomoConfig {
    #[serde(default)]
    tun: Option<MihomoTun>,
}

#[derive(Deserialize)]
struct MihomoTun {
    #[serde(default)]
    enable: bool,
}

/// Typed view of a sing-box config's `inbounds` entries.
#[derive(Deserialize)]
struct SingboxConfig {
    #[serde(default)]
    inbounds: Vec<SingboxInbound>,
}

#[derive(Deserialize)]
struct SingboxInbound {
    #[serde(rename = "type", default)]
    kind: Option<String>,
    /// sing-box treats an omitted `enabled` as enabled.
    #[serde(default = "default_enabled")]
    enabled: bool,
}

fn default_enabled() -> bool {
    true
}

/// Parse a Mihomo/Clash config, logging and swallowing any parse error.
///
/// A config that cannot be parsed yields `None`, so callers treat malformed
/// input as "no TUN information" rather than failing hard.
fn parse_mihomo(config: &str) -> Option<MihomoConfig> {
    match serde_saphyr::from_str::<MihomoConfig>(config) {
        Ok(parsed) => Some(parsed),
        Err(e) => {
            crate::log_debug!("Failed to parse YAML config: {}", e);
            None
        }
    }
}

/// Whether a Mihomo/Clash YAML config defines a `tun` section at all,
/// regardless of whether it is enabled.
///
/// Used by the config patcher to decide whether the default TUN block still
/// needs to be injected.
pub fn has_tun_yaml(config: &str) -> bool {
    parse_mihomo(config).is_some_and(|parsed| parsed.tun.is_some())
}

/// Whether a Mihomo/Clash YAML config enables TUN (`tun.enable: true`).
///
/// Uses `serde-saphyr` rather than a hand-rolled line scan, so comments,
/// quoting, anchors/merge keys, indentation and block structure are all handled
/// by a real parser. A config that cannot be parsed is treated as
/// TUN-disabled, so malformed input can never force device setup on.
pub fn tun_enabled_yaml(config: &str) -> bool {
    parse_mihomo(config)
        .and_then(|parsed| parsed.tun)
        .is_some_and(|tun| tun.enable)
}

/// Whether a sing-box JSON config declares an enabled `tun` inbound.
pub fn tun_enabled_json(config: &str) -> bool {
    match serde_json::from_str::<SingboxConfig>(config) {
        Ok(parsed) => parsed
            .inbounds
            .iter()
            .any(|inbound| inbound.kind.as_deref() == Some("tun") && inbound.enabled),
        Err(e) => {
            crate::log_debug!("Failed to parse JSON config for TUN check: {}", e);
            false
        }
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
    fn yaml_has_tun_reports_presence() {
        // Presence is independent of `enable`.
        assert!(has_tun_yaml("tun:\n  enable: false\n"));
        assert!(has_tun_yaml("tun:\n  stack: gVisor\n"));
        assert!(has_tun_yaml("port: 7890\ntun: {}\n"));

        assert!(!has_tun_yaml("port: 7890\n"));
        // A shared prefix is not a `tun` section.
        assert!(!has_tun_yaml("tun-proxy: 7890\n"));
        // Nested under another key is not the core's TUN section.
        assert!(!has_tun_yaml("proxy:\n  tun:\n    enable: true\n"));
        // Unparseable input reports no presence.
        assert!(!has_tun_yaml("tun: [unclosed\n"));
    }

    #[test]
    fn yaml_handles_comments_and_spacing() {
        assert!(tun_enabled_yaml(
            "tun: # enable tun\n  enable: true # yes\n"
        ));
        // A `tun` nested under another key is not the core's TUN section.
        assert!(!tun_enabled_yaml("proxy:\n  tun:\n    enable: true\n"));
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
