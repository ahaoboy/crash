// Internationalization support

use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]

pub enum Language {
    #[default]
    Chinese,
    English,
}

impl FromStr for Language {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "en" | "english" => Ok(Language::English),
            "zh" | "chinese" | "中文" => Ok(Language::Chinese),
            _ => Err(()),
        }
    }
}
impl Language {
    pub fn code(&self) -> &'static str {
        match self {
            Language::English => "en",
            Language::Chinese => "zh",
        }
    }
}

static CURRENT_LANGUAGE: Lazy<RwLock<Language>> = Lazy::new(|| RwLock::new(Language::Chinese));

pub fn set_language(lang: Language) {
    if let Ok(mut current) = CURRENT_LANGUAGE.write() {
        *current = lang;
    }
}

pub fn get_language() -> Language {
    CURRENT_LANGUAGE
        .read()
        .map(|l| *l)
        .unwrap_or(Language::English)
}

// Translation function
pub fn t(key: &str) -> String {
    let lang = get_language();
    TRANSLATIONS
        .get(&(lang, key))
        .map(|s| s.to_string())
        .unwrap_or_else(|| key.to_string())
}

// Macro for easy translation
#[macro_export]
macro_rules! t {
    ($key:expr) => {
        $crate::common::i18n::t($key)
    };
}

type TranslationKey = (Language, &'static str);

static TRANSLATIONS: Lazy<HashMap<TranslationKey, &'static str>> = Lazy::new(|| {
    let mut m = HashMap::new();

    // Common messages
    m.insert((Language::English, "welcome"), "Welcome to ShellCrash!");
    m.insert((Language::Chinese, "welcome"), "欢迎使用 ShellCrash！");

    m.insert((Language::English, "version"), "Version");
    m.insert((Language::Chinese, "version"), "版本");

    m.insert((Language::English, "service_running"), "Service is running");
    m.insert((Language::Chinese, "service_running"), "服务正在运行");

    m.insert((Language::English, "service_stopped"), "Service is stopped");
    m.insert((Language::Chinese, "service_stopped"), "服务已停止");

    m.insert((Language::English, "memory_usage"), "Memory usage");
    m.insert((Language::Chinese, "memory_usage"), "内存占用");

    m.insert((Language::English, "uptime"), "Uptime");
    m.insert((Language::Chinese, "uptime"), "运行时长");

    // Menu options
    m.insert(
        (Language::English, "menu_start_restart"),
        "Start/Restart service",
    );
    m.insert((Language::Chinese, "menu_start_restart"), "启动/重启服务");

    m.insert((Language::English, "menu_stop"), "Stop service");
    m.insert((Language::Chinese, "menu_stop"), "停止服务");

    m.insert((Language::English, "menu_config_ports"), "Configure ports");
    m.insert((Language::Chinese, "menu_config_ports"), "配置端口");

    m.insert((Language::English, "menu_config_dns"), "Configure DNS");
    m.insert((Language::Chinese, "menu_config_dns"), "配置DNS");

    m.insert(
        (Language::English, "menu_config_firewall"),
        "Configure firewall",
    );
    m.insert((Language::Chinese, "menu_config_firewall"), "配置防火墙");

    m.insert((Language::English, "menu_config_ipv6"), "Configure IPv6");
    m.insert((Language::Chinese, "menu_config_ipv6"), "配置IPv6");

    m.insert((Language::English, "menu_log_push"), "Log push settings");
    m.insert((Language::Chinese, "menu_log_push"), "日志推送设置");

    m.insert((Language::English, "menu_task_manager"), "Task manager");
    m.insert((Language::Chinese, "menu_task_manager"), "任务管理");

    m.insert(
        (Language::English, "menu_update_config"),
        "Update configuration",
    );
    m.insert((Language::Chinese, "menu_update_config"), "更新配置");

    m.insert((Language::English, "menu_exit"), "Exit");
    m.insert((Language::Chinese, "menu_exit"), "退出");

    // Actions
    m.insert(
        (Language::English, "starting_service"),
        "Starting service...",
    );
    m.insert((Language::Chinese, "starting_service"), "正在启动服务...");

    m.insert(
        (Language::English, "stopping_service"),
        "Stopping service...",
    );
    m.insert((Language::Chinese, "stopping_service"), "正在停止服务...");

    m.insert(
        (Language::English, "restarting_service"),
        "Restarting service...",
    );
    m.insert((Language::Chinese, "restarting_service"), "正在重启服务...");

    m.insert(
        (Language::English, "service_started"),
        "Service started successfully",
    );
    m.insert((Language::Chinese, "service_started"), "服务启动成功");

    m.insert(
        (Language::English, "service_stopped_success"),
        "Service stopped successfully",
    );
    m.insert(
        (Language::Chinese, "service_stopped_success"),
        "服务停止成功",
    );

    // Errors
    m.insert((Language::English, "error_config"), "Configuration error");
    m.insert((Language::Chinese, "error_config"), "配置错误");

    m.insert((Language::English, "error_network"), "Network error");
    m.insert((Language::Chinese, "error_network"), "网络错误");

    m.insert((Language::English, "error_io"), "IO error");
    m.insert((Language::Chinese, "error_io"), "IO错误");

    m.insert(
        (Language::English, "error_path_not_found"),
        "Path not found",
    );
    m.insert((Language::Chinese, "error_path_not_found"), "路径未找到");

    m.insert(
        (Language::English, "error_permission_denied"),
        "Permission denied",
    );
    m.insert((Language::Chinese, "error_permission_denied"), "权限被拒绝");

    m.insert(
        (Language::English, "error_service_not_running"),
        "Service not running",
    );
    m.insert(
        (Language::Chinese, "error_service_not_running"),
        "服务未运行",
    );

    // Prompts
    m.insert((Language::English, "prompt_select"), "Please select");
    m.insert((Language::Chinese, "prompt_select"), "请选择");

    m.insert((Language::English, "prompt_input"), "Please input");
    m.insert((Language::Chinese, "prompt_input"), "请输入");

    m.insert((Language::English, "prompt_confirm"), "Confirm?");
    m.insert((Language::Chinese, "prompt_confirm"), "确认？");

    // Task management
    m.insert((Language::English, "task_list"), "Task list");
    m.insert((Language::Chinese, "task_list"), "任务列表");

    m.insert((Language::English, "task_add"), "Add task");
    m.insert((Language::Chinese, "task_add"), "添加任务");

    m.insert((Language::English, "task_remove"), "Remove task");
    m.insert((Language::Chinese, "task_remove"), "删除任务");

    m.insert((Language::English, "task_execute"), "Execute task");
    m.insert((Language::Chinese, "task_execute"), "执行任务");

    m.insert((Language::English, "no_tasks"), "No tasks");
    m.insert((Language::Chinese, "no_tasks"), "没有任务");

    // Download
    m.insert((Language::English, "downloading"), "Downloading");
    m.insert((Language::Chinese, "downloading"), "正在下载");

    m.insert(
        (Language::English, "download_complete"),
        "Download complete",
    );
    m.insert((Language::Chinese, "download_complete"), "下载完成");

    m.insert((Language::English, "extracting"), "Extracting");
    m.insert((Language::Chinese, "extracting"), "正在解压");

    // Update
    m.insert((Language::English, "updating_core"), "Updating core...");
    m.insert((Language::Chinese, "updating_core"), "正在更新内核...");

    m.insert(
        (Language::English, "updating_scripts"),
        "Updating scripts...",
    );
    m.insert((Language::Chinese, "updating_scripts"), "正在更新脚本...");

    m.insert(
        (Language::English, "updating_geoip"),
        "Updating GeoIP database...",
    );
    m.insert(
        (Language::Chinese, "updating_geoip"),
        "正在更新 GeoIP 数据库...",
    );

    m.insert((Language::English, "update_complete"), "Update complete");
    m.insert((Language::Chinese, "update_complete"), "更新完成");

    // Language
    m.insert(
        (Language::English, "language_changed"),
        "Language changed to English",
    );
    m.insert((Language::Chinese, "language_changed"), "语言已切换为中文");

    m.insert((Language::English, "current_language"), "Current language");
    m.insert((Language::Chinese, "current_language"), "当前语言");

    m.insert((Language::English, "menu_language"), "Switch language");
    m.insert((Language::Chinese, "menu_language"), "切换语言");

    m.insert((Language::English, "service_status"), "Service status");
    m.insert((Language::Chinese, "service_status"), "服务状态");

    m.insert((Language::English, "error"), "Error");
    m.insert((Language::Chinese, "error"), "错误");

    m.insert(
        (Language::English, "updating_config"),
        "Updating configuration...",
    );
    m.insert((Language::Chinese, "updating_config"), "正在更新配置...");

    // Port configuration
    m.insert((Language::English, "port_config"), "Port Configuration");
    m.insert((Language::Chinese, "port_config"), "端口配置");

    m.insert(
        (Language::English, "modify_http_port"),
        "Modify Http/Sock5 port",
    );
    m.insert(
        (Language::Chinese, "modify_http_port"),
        "修改Http/Sock5端口",
    );

    m.insert(
        (Language::English, "modify_redir_port"),
        "Modify redirect port",
    );
    m.insert((Language::Chinese, "modify_redir_port"), "修改静态路由端口");

    m.insert((Language::English, "modify_dns_port"), "Modify DNS port");
    m.insert((Language::Chinese, "modify_dns_port"), "修改DNS监听端口");

    m.insert(
        (Language::English, "modify_panel_port"),
        "Modify panel port",
    );
    m.insert((Language::Chinese, "modify_panel_port"), "修改面板访问端口");

    m.insert((Language::English, "return_menu"), "Return to menu");
    m.insert((Language::Chinese, "return_menu"), "返回上级菜单");

    // DNS configuration
    m.insert((Language::English, "dns_config"), "DNS Configuration");
    m.insert((Language::Chinese, "dns_config"), "DNS配置");

    m.insert((Language::English, "current_base_dns"), "Current base DNS");
    m.insert((Language::Chinese, "current_base_dns"), "当前基础DNS");

    m.insert((Language::English, "proxy_dns"), "PROXY-DNS");
    m.insert((Language::Chinese, "proxy_dns"), "PROXY-DNS");

    m.insert((Language::English, "modify_base_dns"), "Modify base DNS");
    m.insert((Language::Chinese, "modify_base_dns"), "修改基础DNS");

    m.insert((Language::English, "modify_proxy_dns"), "Modify PROXY-DNS");
    m.insert((Language::Chinese, "modify_proxy_dns"), "修改PROXY-DNS");

    m.insert((Language::English, "reset_dns"), "Reset DNS configuration");
    m.insert((Language::Chinese, "reset_dns"), "重置默认DNS配置");

    // Firewall configuration
    m.insert(
        (Language::English, "firewall_config"),
        "Firewall Configuration",
    );
    m.insert((Language::Chinese, "firewall_config"), "防火墙配置");

    m.insert(
        (Language::English, "public_dashboard"),
        "Public access to Dashboard",
    );
    m.insert(
        (Language::Chinese, "public_dashboard"),
        "公网访问Dashboard面板",
    );

    m.insert(
        (Language::English, "public_proxy"),
        "Public access to Socks/Http proxy",
    );
    m.insert(
        (Language::Chinese, "public_proxy"),
        "公网访问Socks/Http代理",
    );

    m.insert(
        (Language::English, "custom_ipv4"),
        "Custom transparent routing ipv4 segment",
    );
    m.insert((Language::Chinese, "custom_ipv4"), "自定义透明路由ipv4网段");

    // IPv6 configuration
    m.insert((Language::English, "ipv6_config"), "IPv6 Configuration");
    m.insert((Language::Chinese, "ipv6_config"), "IPv6配置");

    m.insert(
        (Language::English, "ipv6_transparent_proxy"),
        "IPv6 transparent proxy",
    );
    m.insert(
        (Language::Chinese, "ipv6_transparent_proxy"),
        "ipv6透明代理",
    );

    // Log push configuration
    m.insert(
        (Language::English, "log_push_config"),
        "Log Push Configuration",
    );
    m.insert((Language::Chinese, "log_push_config"), "日志推送配置");

    m.insert((Language::English, "telegram_push"), "Telegram push");
    m.insert((Language::Chinese, "telegram_push"), "Telegram推送");

    m.insert((Language::English, "pushdeer_push"), "PushDeer push");
    m.insert((Language::Chinese, "pushdeer_push"), "PushDeer推送");

    m.insert((Language::English, "bark_push"), "Bark push - iOS");
    m.insert((Language::Chinese, "bark_push"), "Bark推送-IOS");

    m.insert((Language::English, "pushover_push"), "Pushover push");
    m.insert((Language::Chinese, "pushover_push"), "Pushover推送");

    m.insert((Language::English, "pushplus_push"), "PushPlus push");
    m.insert((Language::Chinese, "pushplus_push"), "PushPlus推送");

    m.insert((Language::English, "synochat_push"), "SynoChat push");
    m.insert((Language::Chinese, "synochat_push"), "SynoChat推送");

    // Update options
    m.insert(
        (Language::English, "update_subscription"),
        "Update subscription",
    );
    m.insert((Language::Chinese, "update_subscription"), "更新订阅");

    m.insert((Language::English, "update_core"), "Update core");
    m.insert((Language::Chinese, "update_core"), "更新内核");

    m.insert((Language::English, "update_scripts"), "Update scripts");
    m.insert((Language::Chinese, "update_scripts"), "更新脚本");

    m.insert(
        (Language::English, "update_geoip_db"),
        "Update GeoIP database",
    );
    m.insert((Language::Chinese, "update_geoip_db"), "更新GeoIP数据库");

    m.insert((Language::English, "error_invalid_input"), "Invalid input!");
    m.insert(
        (Language::Chinese, "error_invalid_input"),
        "请输入正确的数字！",
    );

    m.insert(
        (Language::English, "updating_subscription"),
        "Updating subscription...",
    );
    m.insert(
        (Language::Chinese, "updating_subscription"),
        "正在更新订阅...",
    );

    m
});
