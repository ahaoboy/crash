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
// The core config's `tun` section is inspected with `serde-mihomo` for
// mihomo/clash (a full, typed parse of `config.yaml`) and `serde_json` for
// sing-box, so the check is a real parse rather than string matching.
//
// Every failure in the device preparation below is only logged, never
// propagated: a missing TUN device must not abort a start that might otherwise
// succeed (e.g. a non-root user with a non-TUN config, or a platform where TUN
// is handled differently). Parsing the config, on the other hand, is fallible:
// `mihomo_tun_enabled` / `singbox_tun_enabled` return the parser error so the
// caller can reject a broken config before starting the core.

use serde::Deserialize;

/// Typed view of a sing-box config's `inbounds` entries.
///
/// mihomo/clash documents are modelled by `serde-mihomo`; sing-box is a
/// different format, so its `tun` check stays a small local struct.
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

/// Parse a mihomo (Clash.Meta) `config.yaml` with `serde-mihomo`.
fn parse_mihomo(config: &str) -> Result<serde_mihomo::Config, serde_mihomo::Error> {
    serde_mihomo::Config::from_yaml_str(config)
}

/// Whether a mihomo (Clash.Meta) YAML config enables TUN (`tun.enable: true`).
///
/// Deserializing with `serde-mihomo` means comments, quoting, anchors/merge keys
/// and the whole document structure are handled by a real parser. The parser
/// error is returned rather than swallowed, so a caller can validate the config
/// before starting the core.
pub fn mihomo_tun_enabled(config: &str) -> Result<bool, serde_mihomo::Error> {
    Ok(parse_mihomo(config)?.tun.is_some_and(|tun| tun.enable))
}

/// Whether a mihomo (Clash.Meta) YAML config defines a `tun` section at all,
/// regardless of whether it is enabled.
///
/// Used by the config patcher to decide whether the default TUN block still
/// needs to be injected. An unparseable document reports `false`, so the
/// default block is added and the config is repaired on the next download.
pub fn has_tun_yaml(config: &str) -> bool {
    parse_mihomo(config).is_ok_and(|config| config.tun.is_some())
}

/// Whether a sing-box JSON config declares an enabled `tun` inbound.
///
/// The JSON error is returned rather than swallowed so callers can validate the
/// config before starting the core.
pub fn singbox_tun_enabled(config: &str) -> Result<bool, serde_json::Error> {
    let parsed: SingboxConfig = serde_json::from_str(config)?;

    Ok(parsed
        .inbounds
        .iter()
        .any(|inbound| inbound.kind.as_deref() == Some("tun") && inbound.enabled))
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
        assert!(mihomo_tun_enabled("tun:\n  enable: true\n  stack: gVisor\n").unwrap());
        assert!(mihomo_tun_enabled("port: 7890\ntun:\n  enable: true\n").unwrap());
    }

    #[test]
    fn yaml_disabled_or_absent() {
        assert!(!mihomo_tun_enabled("port: 7890\n").unwrap());
        assert!(!mihomo_tun_enabled("tun:\n  enable: false\n").unwrap());
        // `enable` outside the `tun` block must not count.
        assert!(!mihomo_tun_enabled("other:\n  enable: true\n").unwrap());
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
        assert!(mihomo_tun_enabled("tun: # enable tun\n  enable: true # yes\n").unwrap());
        // A `tun` nested under another key is not the core's TUN section.
        assert!(!mihomo_tun_enabled("proxy:\n  tun:\n    enable: true\n").unwrap());
    }

    #[test]
    fn yaml_surfaces_parse_errors() {
        // Not valid YAML at all.
        assert!(mihomo_tun_enabled("tun: [unclosed\n").is_err());
        // Valid YAML, but does not match the mihomo schema (proxy without `type`).
        assert!(mihomo_tun_enabled("proxies:\n  - name: broken\n").is_err());
    }

    #[test]
    fn json_enabled() {
        assert!(singbox_tun_enabled(r#"{"inbounds":[{"type":"tun","enabled":true}]}"#).unwrap());
        // `enabled` omitted defaults to true in sing-box.
        assert!(singbox_tun_enabled(r#"{"inbounds":[{"type":"tun"}]}"#).unwrap());
    }

    #[test]
    fn json_disabled_or_absent() {
        assert!(!singbox_tun_enabled(r#"{"inbounds":[{"type":"tun","enabled":false}]}"#).unwrap());
        assert!(!singbox_tun_enabled(r#"{"inbounds":[{"type":"mixed"}]}"#).unwrap());
    }

    #[test]
    fn json_surfaces_parse_errors() {
        assert!(singbox_tun_enabled("not json").is_err());
    }

    #[test]
    fn shipped_config_assets_are_valid() {
        // The default config written by `install` must satisfy the mihomo
        // schema, otherwise `start` would reject a freshly installed setup.
        assert!(!mihomo_tun_enabled(include_str!("../assets/mihomo.yaml")).unwrap());
        assert!(!has_tun_yaml(include_str!("../assets/mihomo.yaml")));

        // The default TUN block injected by the patcher must be valid too.
        let tun_block = include_str!("../assets/mihomo_tun.yaml");
        assert!(mihomo_tun_enabled(tun_block).unwrap());
        assert!(has_tun_yaml(tun_block));
    }
}
