// Configuration patching per core type.
//
// `patch_config` is a free function rather than a method on `CrashConfig`
// because it only reads `core` and `web` and never needs `&self` state.
// Keeping it standalone makes it trivial to unit-test in isolation.

use super::core::Core;
use super::web::WebConfig;
use serde_json::{Value, json};

/// Default TUN block appended to Mihomo configs that don't already define one.
const MIHOMO_TUN_YAML: &str = include_str!("../assets/mihomo_tun.yaml");

/// Patch a raw downloaded configuration so it is usable by the target core.
pub fn patch_config(core: Core, web: &WebConfig, config: &str) -> String {
    match core {
        Core::Mihomo => {
            let has_tun = config.lines().any(|i| i.starts_with("tun"));
            if has_tun {
                config.to_string()
            } else {
                format!("{}\n{}", config, MIHOMO_TUN_YAML)
            }
        }
        Core::Clash => config.replace("- 'RULE-SET,", "#- 'RULE-SET,").to_string(),
        Core::Singbox => patch_singbox(web, config),
    }
}

/// Patch a Singbox JSON configuration: coerce string `server_port` values to
/// numbers and merge in the clash_api / external_ui block from the web config.
fn patch_singbox(web: &WebConfig, config: &str) -> String {
    let Ok(mut v) = serde_json::from_str::<Value>(config) else {
        return config.to_string();
    };

    // Some providers emit `server_port` as a string; sing-box requires a
    // number. Coerce any string ports to numbers.
    //   FATAL[0000] outbounds[5].server_port: json: cannot unmarshal string
    //   into Go value of type uint16
    if let Some(outbounds) = v.get_mut("outbounds").and_then(|o| o.as_array_mut()) {
        for item in outbounds {
            if let Some(port_val) = item.get_mut("server_port")
                && let Some(port_str) = port_val.as_str()
                && let Ok(port_num) = port_str.parse::<u64>()
            {
                *port_val = json!(port_num);
            }
        }
    }

    let ui = web.ui.to_string();
    let secret = web.secret.to_string();
    let patch = json!({
        "experimental": {
            "cache_file": {
                "enabled": true
            },
            "clash_api": {
                "external_controller": ":9090",
                "external_ui": ui,
                "secret": secret
            }
        }
    });
    merge_json(&mut v, &patch);

    serde_json::to_string_pretty(&v).unwrap_or_else(|_| config.to_string())
}

/// Recursively merge `src` into `dst`. For objects, matching keys are merged
/// recursively; non-matching keys are inserted. For any non-object value
/// (or when the types disagree), `src` replaces `dst`.
fn merge_json(dst: &mut Value, src: &Value) {
    if dst.is_object() && src.is_object() {
        // SAFETY: checked above.
        let dst_map = dst.as_object_mut().unwrap();
        let src_map = src.as_object().unwrap();
        for (k, v) in src_map {
            match dst_map.get_mut(k) {
                Some(dst_v) => merge_json(dst_v, v),
                None => {
                    dst_map.insert(k.clone(), v.clone());
                }
            }
        }
    } else {
        // Leaf value or type mismatch: src wins.
        *dst = src.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn web() -> WebConfig {
        WebConfig::default()
    }

    #[test]
    fn mihomo_appends_tun_when_absent() {
        let input = "port: 7890\n";
        let out = patch_config(Core::Mihomo, &web(), input);
        assert!(out.contains("tun:"));
        assert!(out.starts_with("port: 7890"));
    }

    #[test]
    fn mihomo_keeps_existing_tun() {
        let input = "tun:\n  enable: false\n";
        let out = patch_config(Core::Mihomo, &web(), input);
        // Should not append the default tun block since one already exists.
        assert!(!out.contains("device: Meta"));
    }

    #[test]
    fn clash_disables_rule_set() {
        let input = "rules:\n- 'RULE-SET,cn,/path'\n";
        let out = patch_config(Core::Clash, &web(), input);
        assert!(out.contains("#- 'RULE-SET,cn,/path'"));
    }

    #[test]
    fn singbox_coerces_string_server_port() {
        let input = r#"{"outbounds":[{"type":"socks","server_port":"1080"}]}"#;
        let out = patch_config(Core::Singbox, &web(), input);
        let v: Value = serde_json::from_str(&out).expect("output is valid json");
        assert_eq!(v["outbounds"][0]["server_port"], json!(1080));
    }

    #[test]
    fn singbox_invalid_json_returned_unchanged() {
        let input = "not json";
        let out = patch_config(Core::Singbox, &web(), input);
        assert_eq!(out, input);
    }

    #[test]
    fn merge_json_deep_merge() {
        let mut dst = json!({"a": {"b": 1, "c": 2}});
        let src = json!({"a": {"c": 3, "d": 4}});
        merge_json(&mut dst, &src);
        assert_eq!(dst, json!({"a": {"b": 1, "c": 3, "d": 4}}));
    }
}
