//! 一些常用的玩意

use std::{io::Write, path::Path};

/// 安全写入文件数据，写入完成后会等待文件缓冲区完全写入才关闭文件
pub fn write_file_safe(p: impl AsRef<Path>, data: &[u8]) -> Result<(), std::io::Error> {
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(p)?;
    f.write_all(data)?;
    f.flush()?;
    f.sync_all()?;
    Ok(())
}
