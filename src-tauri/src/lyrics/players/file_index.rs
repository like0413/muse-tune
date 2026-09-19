use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::SystemTime,
};

struct CachedDirectory<T> {
    modified: Option<SystemTime>,
    entries: Arc<[T]>,
}

/// 按目录修改时间缓存文件名元数据，正文仍在最终命中后即时读取。
pub struct DirectoryFileIndex<T> {
    directories: Mutex<HashMap<PathBuf, CachedDirectory<T>>>,
}

impl<T> DirectoryFileIndex<T> {
    pub fn new() -> Self {
        Self {
            directories: Mutex::new(HashMap::new()),
        }
    }

    /// 目录内容未变化时复用解析后的文件名，避免每首冷歌曲重复遍历大目录。
    /// 返回共享索引而不是深拷贝，使上千条目的播放器缓存不再被整份克隆。
    pub fn load(
        &self,
        directory: &Path,
        scan: impl FnOnce() -> io::Result<Vec<T>>,
    ) -> io::Result<Arc<[T]>> {
        let modified = fs::metadata(directory)?.modified().ok();
        if let Ok(directories) = self.directories.lock()
            && let Some(cached) = directories.get(directory)
            && cached.modified == modified
        {
            return Ok(Arc::clone(&cached.entries));
        }
        let entries: Arc<[T]> = scan()?.into();
        if let Ok(mut directories) = self.directories.lock() {
            directories.insert(
                directory.to_path_buf(),
                CachedDirectory {
                    modified,
                    entries: Arc::clone(&entries),
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
