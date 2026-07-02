use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use nova_core::{Result, RuntimeError};

pub struct MmapRegion {
    file: Option<std::fs::File>,
    path: PathBuf,
    len: u64,
}

impl MmapRegion {
    pub fn new(path: &Path, size: u64) -> Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .read(true)
            .write(true)
            .open(path)?;
        file.set_len(size)?;
        Ok(MmapRegion {
            file: Some(file),
            path: path.to_path_buf(),
            len: size,
        })
    }

    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub unsafe fn map(&self) -> Result<*mut u8> {
        use std::os::unix::io::AsRawFd;
        let fd = self.file.as_ref().unwrap().as_raw_fd();
        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                self.len as usize,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                0,
            )
        };
        if ptr == libc::MAP_FAILED {
            return Err(RuntimeError::Io("mmap failed".into()));
        }
        Ok(ptr as *mut u8)
    }

    pub unsafe fn unmap(&self, ptr: *mut u8) -> Result<()> {
        let result = unsafe { libc::munmap(ptr as *mut libc::c_void, self.len as usize) };
        if result != 0 {
            return Err(RuntimeError::Io("munmap failed".into()));
        }
        Ok(())
    }
}

impl Drop for MmapRegion {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[track_caller]
    fn test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nova_memory_test_{}", name));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn new_mmap_region() {
        let dir = test_dir("new");
        let path = dir.join("test.mmap");
        let region = MmapRegion::new(&path, 4096).unwrap();
        assert_eq!(region.len(), 4096);
        assert!(!region.is_empty());
        drop(region);
        assert!(!path.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn mmap_map_unmap_cycle() {
        let dir = test_dir("cycle");
        let path = dir.join("test_cycle.mmap");
        let region = MmapRegion::new(&path, 4096).unwrap();
        unsafe {
            let ptr = region.map().unwrap();
            assert!(!ptr.is_null());
            std::ptr::write(ptr, 0x42u8);
            assert_eq!(std::ptr::read(ptr), 0x42u8);
            region.unmap(ptr).unwrap();
        }
        drop(region);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn mmap_len_and_empty() {
        let dir = test_dir("len");
        let path = dir.join("test_len.mmap");
        let region = MmapRegion::new(&path, 0).unwrap();
        assert!(region.is_empty());
        assert_eq!(region.len(), 0);
        drop(region);
        let region2 = MmapRegion::new(&path, 8192).unwrap();
        assert_eq!(region2.len(), 8192);
        assert!(!region2.is_empty());
        drop(region2);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
