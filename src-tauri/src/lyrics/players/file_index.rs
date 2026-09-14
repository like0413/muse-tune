use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
    sync::Mutex,
    time::SystemTime,
};

struct CachedDirectory<T> {
    modified: Option<SystemTime>,
    entries: Vec<T>,
}

/// 按目录修改时间缓存文件名元数据，正文仍在最终命中后即时读取。
pub struct DirectoryFileIndex<T> {
    directories: Mutex<HashMap<PathBuf, CachedDirectory<T>>>,
}

impl<T: Clone> DirectoryFileIndex<T> {
    pub fn new() -> Self {
        Self {
            directories: Mutex::new(HashMap::new()),
        }
    }

    /// 目录内容未变化时复用解析后的文件名，避免每首冷歌曲重复遍历大目录。
    pub fn load(
        &self,
        directory: &Path,
        scan: impl FnOnce() -> io::Result<Vec<T>>,
    ) -> io::Result<Vec<T>> {
        let modified = fs::metadata(directory)?.modified().ok();
        if let Ok(directories) = self.directories.lock()
            && let Some(cached) = directories.get(directory)
            && cached.modified == modified
        {
            return Ok(cached.entries.clone());
        }
        let entries = scan()?;
        if let Ok(mut directories) = self.directories.lock() {
            directories.insert(
                directory.to_path_buf(),
                CachedDirectory {
                    modified,
                    entries: entries.clone(),
                },
            );
        }
        Ok(entries)
    }

    /// 文件监听已确认目录内容变化时，立即淘汰该目录的旧索引。
    pub fn invalidate(&self, directory: &Path) {
        if let Ok(mut directories) = self.directories.lock() {
            directories.remove(directory);
        }
    }
}
