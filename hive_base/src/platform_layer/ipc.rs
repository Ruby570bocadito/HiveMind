use std::io;

/// ArenaTrait: abstraction over shared memory arena creation and access.
/// LinuxArena uses memfd_create + mmap (no filesystem footprint).
/// WindowsArena uses CreateFileMapping + MapViewOfFile.
pub trait ArenaTrait: Send + Sync {
    fn create_or_open(arena_name: Option<&str>) -> io::Result<Self>
    where Self: Sized;
    fn as_ptr(&self) -> *mut u8;
    fn size(&self) -> usize;
    fn is_owned(&self) -> bool;
}

// ── Linux implementation (memfd_create + mmap) ────────────────────────────────

#[cfg(target_os = "linux")]
pub struct ArenaMapping {
    ptr: *mut u8,
    size: usize,
    fd: i32,
    owned: bool,
    #[allow(dead_code)]
    arena_name: String,
}

#[cfg(target_os = "linux")]
unsafe impl Send for ArenaMapping {}
#[cfg(target_os = "linux")]
unsafe impl Sync for ArenaMapping {}

#[cfg(target_os = "linux")]
impl ArenaTrait for ArenaMapping {
    fn create_or_open(arena_name: Option<&str>) -> io::Result<Self> {
        let size = crate::shared_arena::arena_size();
        use std::ffi::CString;
        use std::ptr;

        let (fd, owned, name) = if let Some(name) = arena_name {
            let cname = CString::new(name).unwrap();
            let fd = unsafe {
                libc::shm_open(cname.as_ptr(), libc::O_RDWR | libc::O_CREAT | libc::O_EXCL, 0o600)
            };
            let (fd, owned) = if fd == -1 {
                let err = io::Error::last_os_error();
                if err.raw_os_error() == Some(libc::EEXIST) {
                    let fd = unsafe { libc::shm_open(cname.as_ptr(), libc::O_RDWR, 0o600) };
                    if fd == -1 { return Err(io::Error::last_os_error()); }
                    (fd, false)
                } else { return Err(err); }
            } else { (fd, true) };
            (fd, owned, name.to_string())
        } else {
            let cname = CString::new("hive_arena").unwrap();
            let fd = unsafe { libc::memfd_create(cname.as_ptr(), libc::MFD_CLOEXEC) };
            if fd == -1 { return Err(io::Error::last_os_error()); }
            (fd, true, "(anonymous)".to_string())
        };

        if owned {
            let rc = unsafe { libc::ftruncate64(fd, size as i64) };
            if rc == -1 { let _ = unsafe { libc::close(fd) }; return Err(io::Error::last_os_error()); }
        }

        let ptr = unsafe {
            libc::mmap(ptr::null_mut(), size, libc::PROT_READ | libc::PROT_WRITE, libc::MAP_SHARED, fd, 0)
        };
        if ptr == libc::MAP_FAILED { let _ = unsafe { libc::close(fd) }; return Err(io::Error::last_os_error()); }

        unsafe { libc::mlock(ptr, size); }

        Ok(Self { ptr: ptr as *mut u8, size, fd, owned, arena_name: name })
    }

    fn as_ptr(&self) -> *mut u8 { self.ptr }
    fn size(&self) -> usize { self.size }
    fn is_owned(&self) -> bool { self.owned }
}

#[cfg(target_os = "linux")]
impl Drop for ArenaMapping {
    fn drop(&mut self) {
        unsafe {
            libc::munlock(self.ptr as *const libc::c_void, self.size);
            libc::munmap(self.ptr as *mut libc::c_void, self.size);
            libc::close(self.fd);
        }
    }
}

// ── Windows implementation (CreateFileMapping + MapViewOfFile) ──────────────

#[cfg(target_os = "windows")]
pub struct ArenaMapping {
    ptr: *mut u8,
    size: usize,
    handle: isize,
    owned: bool,
    arena_name: String,
}

#[cfg(target_os = "windows")]
unsafe impl Send for ArenaMapping {}
#[cfg(target_os = "windows")]
unsafe impl Sync for ArenaMapping {}

#[cfg(target_os = "windows")]
impl ArenaTrait for ArenaMapping {
    fn create_or_open(arena_name: Option<&str>) -> io::Result<Self> {
        let size = crate::shared_arena::arena_size();
        let name = arena_name.unwrap_or("Local\\HiveArena");
        let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();

        let handle = unsafe {
            winapi::um::memoryapi::CreateFileMappingW(
                winapi::um::handleapi::INVALID_HANDLE_VALUE,
                std::ptr::null_mut(),
                winapi::um::winnt::PAGE_READWRITE,
                (size >> 32) as u32,
                size as u32,
                wide.as_ptr(),
            )
        };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        let owned = unsafe { winapi::um::errhandlingapi::GetLastError() != winapi::shared::winerror::ERROR_ALREADY_EXISTS };

        let ptr = unsafe {
            winapi::um::memoryapi::MapViewOfFile(
                handle,
                (winapi::um::winnt::SECTION_MAP_WRITE | winapi::um::winnt::SECTION_MAP_READ | winapi::um::winnt::SECTION_MAP_EXECUTE | winapi::um::winnt::SECTION_EXTEND_SIZE),
                0, 0, size,
            )
        };
        if ptr.is_null() { return Err(io::Error::last_os_error()); }

        Ok(Self { ptr: ptr as *mut u8, size, handle: handle as isize, owned, arena_name: name.to_string() })
    }

    fn as_ptr(&self) -> *mut u8 { self.ptr }
    fn size(&self) -> usize { self.size }
    fn is_owned(&self) -> bool { self.owned }
}

#[cfg(target_os = "windows")]
impl Drop for ArenaMapping {
    fn drop(&mut self) {
        unsafe {
            winapi::um::memoryapi::UnmapViewOfFile(self.ptr as *const winapi::ctypes::c_void);
            winapi::um::handleapi::CloseHandle(self.handle as *mut winapi::ctypes::c_void);
        }
    }
}

// ── Non-Linux-non-Windows fallback (heap allocation) ─────────────────────────

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub struct ArenaMapping {
    ptr: *mut u8,
    size: usize,
    owned: bool,
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
unsafe impl Send for ArenaMapping {}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
impl ArenaTrait for ArenaMapping {
    fn create_or_open(_arena_name: Option<&str>) -> io::Result<Self> {
        let size = crate::shared_arena::arena_size();
        use std::alloc::{self, Layout};
        let layout = Layout::from_size_align(size, 4096)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        let ptr = unsafe { alloc::alloc_zeroed(layout) };
        if ptr.is_null() { return Err(io::Error::new(io::ErrorKind::OutOfMemory, "arena alloc failed")); }
        Ok(Self { ptr, size, owned: true })
    }

    fn as_ptr(&self) -> *mut u8 { self.ptr }
    fn size(&self) -> usize { self.size }
    fn is_owned(&self) -> bool { self.owned }
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
impl Drop for ArenaMapping {
    fn drop(&mut self) {
        use std::alloc::{self, Layout};
        let layout = Layout::from_size_align(self.size, 4096).unwrap();
        unsafe { alloc::dealloc(self.ptr, layout); }
    }
}
