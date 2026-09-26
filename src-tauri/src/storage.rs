//! 统一解析安装版与便携版的应用数据位置。

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use tauri::{Manager, Runtime};

use crate::settings_store;

/// 当前二进制是否在编译时启用了便携模式。
pub(crate) const IS_PORTABLE: bool = cfg!(feature = "portable");

/// 应用运行期使用的全部自有存储路径。
#[derive(Clone, Debug)]
pub(crate) struct StoragePaths {
    config_directory: PathBuf,
    cache_directory: PathBuf,
    log_directory: PathBuf,
    webview_directory: Option<PathBuf>,
}

impl StoragePaths {
    /// 从编译模式和 Tauri 官方路径解析器生成路径，并提前创建所需目录。
    pub(crate) fn resolve<R: Runtime>(app: &tauri::App<R>) -> io::Result<Self> {
        let paths = if IS_PORTABLE {
            let root = portable_data_root()?;
            Self {
                config_directory: root.join("config"),
                cache_directory: root.join("cache"),
                log_directory: root.join("logs"),
                webview_directory: Some(root.join("webview")),
            }
        } else {
            Self {
                config_directory: app.path().app_data_dir().map_err(io::Error::other)?,
                cache_directory: app.path().app_cache_dir().map_err(io::Error::other)?,
                log_directory: app.path().app_log_dir().map_err(io::Error::other)?,
                // 安装版继续交给 Tauri/WebView2 使用原有默认目录。
                webview_directory: None,
            }
        };
        paths.ensure_directories()?;
        Ok(paths)
    }

    /// 配置目录；便携版为 `data/config`。
    pub(crate) fn config_directory(&self) -> &Path {
        &self.config_directory
    }

    /// 缓存目录；便携版为 `data/cache`。
    pub(crate) fn cache_directory(&self) -> &Path {
        &self.cache_directory
    }

    /// 日志目录；便携版为 `data/logs`。
    pub(crate) fn log_directory(&self) -> &Path {
        &self.log_directory
    }

    /// 便携版专用 WebView2 数据目录；安装版返回 `None` 保持原行为。
    pub(crate) fn webview_directory(&self) -> Option<&Path> {
        self.webview_directory.as_deref()
    }

    /// Tauri Store 使用的设置文件绝对路径。
    pub(crate) fn settings_file(&self) -> PathBuf {
        self.config_directory.join(settings_store::PATH)
    }

    /// 创建运行期会写入的目录；失败时直接阻止启动，避免静默写回系统目录。
    fn ensure_directories(&self) -> io::Result<()> {
        for directory in [
            Some(self.config_directory()),
            Some(self.cache_directory()),
            Some(self.log_directory()),
            self.webview_directory(),
        ]
        .into_iter()
        .flatten()
        {
            fs::create_dir_all(directory)?;
        }
        Ok(())
    }
}

/// 日志插件在 `setup` 之前初始化，因此便携日志目录需要提前单独解析。
pub(crate) fn portable_log_directory() -> io::Result<Option<PathBuf>> {
    if IS_PORTABLE {
        Ok(Some(portable_data_root()?.join("logs")))
    } else {
        Ok(None)
    }
}

/// 返回可执行文件相邻的 `data` 根目录。
fn portable_data_root() -> io::Result<PathBuf> {
    let executable = std::env::current_exe()?;
    let directory = executable
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "无法定位 MuseTune.exe 所在目录"))?;
    Ok(directory.join("data"))
}

/// 暴露给前端的只读运行环境。
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeEnvironment {
    pub(crate) portable: bool,
    pub(crate) settings_store_path: PathBuf,
}

impl From<&StoragePaths> for RuntimeEnvironment {
    fn from(paths: &StoragePaths) -> Self {
        Self {
            portable: IS_PORTABLE,
            settings_store_path: paths.settings_file(),
        }
    }
}
