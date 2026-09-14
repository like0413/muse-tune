//! 应用管理目录的 Windows reparse-point 边界检查。

use std::{
    fs::{self, DirEntry, Metadata},
    io,
    os::windows::fs::MetadataExt,
    path::Path,
};

use windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;

/// 创建缺失的应用管理目录，并在使用前后都拒绝 Windows reparse point。
pub(crate) fn ensure_managed_directory(path: &Path) -> io::Result<()> {
    match metadata_if_exists(path)? {
        Some(metadata) if metadata.is_dir() => Ok(()),
        Some(_) => Err(invalid_path(path, "不是目录")),
        None => {
            fs::create_dir_all(path)?;
            ensure_directory(path)
        }
    }
}

/// 读取不跟随最终路径链接的元数据，并拒绝所有 Windows reparse point。
pub(crate) fn metadata_without_reparse(path: &Path) -> io::Result<Metadata> {
    let metadata = fs::symlink_metadata(path)?;
    reject_reparse_point(path, &metadata)?;
    Ok(metadata)
}

/// 读取可选文件元数据；文件不存在是正常状态，reparse point 仍会被拒绝。
pub(crate) fn metadata_if_exists(path: &Path) -> io::Result<Option<Metadata>> {
    match metadata_without_reparse(path) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// 确保高影响操作的根路径是实体目录，而不是链接、junction 或其他重解析点。
pub(crate) fn ensure_directory(path: &Path) -> io::Result<()> {
    let metadata = metadata_without_reparse(path)?;
    if metadata.is_dir() {
        Ok(())
    } else {
        Err(invalid_path(path, "不是目录"))
    }
}

/// 读取目录项本身的元数据，避免 `DirEntry::metadata` 跟随链接目标。
pub(crate) fn entry_metadata_without_reparse(entry: &DirEntry) -> io::Result<Metadata> {
    metadata_without_reparse(&entry.path())
}

/// Windows 的 junction、符号链接和云占位符都带有 reparse-point 属性。
fn reject_reparse_point(path: &Path, metadata: &Metadata) -> io::Result<()> {
    if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 == 0 {
        Ok(())
    } else {
        Err(invalid_path(path, "是 Windows reparse point"))
    }
}

/// 为边界拒绝生成包含实际路径的稳定 I/O 错误。
fn invalid_path(path: &Path, reason: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("拒绝访问不安全路径 {}：{reason}", path.display()),
    )
}
