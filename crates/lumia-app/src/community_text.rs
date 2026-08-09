//! Community-plugin UI strings. Kept separate from [`crate::i18n::tr`] so the
//! i18n module stays under the project's 500-line limit now that the community
//! browser adds a block of new strings.

use lumia_core::Language;

#[allow(dead_code)]
#[derive(Clone, Copy)]
pub(crate) enum CommunityTextKey {
    CommunityPluginsDescription,
    Community,
    Installed,
    CommunitySearchPlaceholder,
    RefreshPlugins,
    CommunityLoading,
    CommunityLoadFailed,
    CommunityOffline,
    NoCommunityResults,
    CommunityInstall,
    CommunityUpdate,
    CommunityInstalled,
    CommunityIncompatible,
    CommunityDownloading,
    CommunityAuthor,
    CommunityRequiresLumia,
}

pub(crate) fn tr_community(language: Language, key: CommunityTextKey) -> &'static str {
    match language {
        Language::English => match key {
            CommunityTextKey::CommunityPluginsDescription => {
                "Browse and install community plugins signed by the official Lumia key."
            }
            CommunityTextKey::Community => "Community",
            CommunityTextKey::Installed => "Installed",
            CommunityTextKey::CommunitySearchPlaceholder => "Search plugins…",
            CommunityTextKey::RefreshPlugins => "Refresh",
            CommunityTextKey::CommunityLoading => "Loading plugin index…",
            CommunityTextKey::CommunityLoadFailed => "Could not load the plugin index.",
            CommunityTextKey::CommunityOffline => "Showing the cached plugin list from",
            CommunityTextKey::NoCommunityResults => "No plugins match your search.",
            CommunityTextKey::CommunityInstall => "Install",
            CommunityTextKey::CommunityUpdate => "Update",
            CommunityTextKey::CommunityInstalled => "Installed",
            CommunityTextKey::CommunityIncompatible => {
                "Not compatible with this version of Lumia"
            }
            CommunityTextKey::CommunityDownloading => "Downloading",
            CommunityTextKey::CommunityAuthor => "by",
            CommunityTextKey::CommunityRequiresLumia => "Requires Lumia",
        },
        Language::Chinese => match key {
            CommunityTextKey::CommunityPluginsDescription => {
                "浏览并安装由官方 Lumia 密钥签名的社区插件。"
            }
            CommunityTextKey::Community => "社区",
            CommunityTextKey::Installed => "已安装",
            CommunityTextKey::CommunitySearchPlaceholder => "搜索插件…",
            CommunityTextKey::RefreshPlugins => "刷新",
            CommunityTextKey::CommunityLoading => "正在加载插件索引…",
            CommunityTextKey::CommunityLoadFailed => "无法加载插件索引。",
            CommunityTextKey::CommunityOffline => "显示缓存的插件列表（来自",
            CommunityTextKey::NoCommunityResults => "没有与搜索匹配的插件。",
            CommunityTextKey::CommunityInstall => "安装",
            CommunityTextKey::CommunityUpdate => "更新",
            CommunityTextKey::CommunityInstalled => "已安装",
            CommunityTextKey::CommunityIncompatible => "与此版本的 Lumia 不兼容",
            CommunityTextKey::CommunityDownloading => "正在下载",
            CommunityTextKey::CommunityAuthor => "作者",
            CommunityTextKey::CommunityRequiresLumia => "需要 Lumia",
        },
    }
}
