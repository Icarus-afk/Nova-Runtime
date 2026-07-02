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
