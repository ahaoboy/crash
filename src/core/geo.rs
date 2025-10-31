// GeoIP database management utilities
// This module provides helper functions for geo database operations

use crate::config::core::Core;

/// Get the list of geo databases for a specific core type
pub fn get_database_list(core_type: Core) -> Vec<&'static str> {
    match core_type {
        Core::Mihomo => vec!["geoip.metadb", "geoip.dat", "geosite.dat"],
        Core::Clash => vec![
            "china_ip_list.txt",
            "china_ipv6_list.txt",
            "cn_mini.mmdb",
            "Country.mmdb",
            "geoip_cn.db",
            "geosite.dat",
            "geosite_cn.db",
            "mrs_geosite_cn.mrs",
            "srs_geoip_cn.srs",
            "srs_geosite_cn.srs",
        ],
        Core::Singbox => vec![],
    }
}
