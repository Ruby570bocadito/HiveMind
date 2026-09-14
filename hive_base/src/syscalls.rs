// Direct system calls bypassing libc hooks (EDR evasion).
// EDRs hook ntdll.dll (Windows) or libc (Linux) to intercept syscalls.
// By calling the kernel directly, we avoid these hooks entirely.
//
// Linux: inline asm with syscall instruction.
// Windows: NT syscalls via ntapi (stub generated at runtime).

// ── Linux direct syscalls ────────────────────────────────────────────────────

// NOTE: the `syscall` instruction used in this module is x86_64-only
// (aarch64 Linux uses a different register ABI with `svc`), so the module is
// additionally gated on target_arch to keep aarch64 builds compiling.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub mod linux {

    /// Execute a raw syscall with up to 6 arguments.
    /// Returns the raw syscall return value (usually i64).
    ///
    /// # Safety
    ///
    /// The caller must ensure the syscall number is valid for the current
    /// platform and that arguments are correctly typed. Incorrect syscall
    /// arguments can crash the process or cause undefined behavior.
    #[inline(always)]
    pub unsafe fn syscall0(nr: i64) -> i64 {
        let ret: i64;
        std::arch::asm!(
            "syscall",
            in("rax") nr,
            lateout("rax") ret,
            lateout("rcx") _, lateout("r11") _,
            options(nostack),
        );
        ret
    }

    /// Execute a raw syscall with 1 argument.
    ///
    /// # Safety
    ///
    /// The caller must ensure the syscall number and arguments are valid
    /// for the current platform. Incorrect arguments can crash the process.
    #[inline(always)]
    pub unsafe fn syscall1(nr: i64, a1: i64) -> i64 {
        let ret: i64;
        std::arch::asm!(
            "syscall",
            in("rax") nr,
            in("rdi") a1,
            lateout("rax") ret,
            lateout("rcx") _, lateout("r11") _,
            options(nostack),
        );
        ret
    }

    /// Execute a raw syscall with 2 arguments.
    ///
    /// # Safety
    ///
    /// The caller must ensure the syscall number and arguments are valid
    /// for the current platform. Incorrect arguments can crash the process.
    #[inline(always)]
    pub unsafe fn syscall2(nr: i64, a1: i64, a2: i64) -> i64 {
        let ret: i64;
        std::arch::asm!(
            "syscall",
            in("rax") nr,
            in("rdi") a1,
            in("rsi") a2,
            lateout("rax") ret,
            lateout("rcx") _, lateout("r11") _,
            options(nostack),
        );
        ret
    }

    /// Execute a raw syscall with 3 arguments.
    ///
    /// # Safety
    ///
    /// The caller must ensure the syscall number and arguments are valid
    /// for the current platform. Incorrect arguments can crash the process.
    #[inline(always)]
    pub unsafe fn syscall3(nr: i64, a1: i64, a2: i64, a3: i64) -> i64 {
        let ret: i64;
        std::arch::asm!(
            "syscall",
            in("rax") nr,
            in("rdi") a1, in("rsi") a2, in("rdx") a3,
            lateout("rax") ret,
            lateout("rcx") _, lateout("r11") _,
            options(nostack),
        );
        ret
    }

    /// Execute a raw syscall with 4 arguments.
    ///
    /// # Safety
    ///
    /// The caller must ensure the syscall number and arguments are valid
    /// for the current platform. Incorrect arguments can crash the process.
    #[inline(always)]
    pub unsafe fn syscall4(nr: i64, a1: i64, a2: i64, a3: i64, a4: i64) -> i64 {
        let ret: i64;
        std::arch::asm!(
            "syscall",
            in("rax") nr,
            in("rdi") a1, in("rsi") a2, in("rdx") a3, in("r10") a4,
            lateout("rax") ret,
            lateout("rcx") _, lateout("r11") _,
            options(nostack),
        );
        ret
    }

    // ── Common syscall wrappers ──────────────────────────────────────────────

    /// getpid() without libc
    pub fn raw_getpid() -> i64 {
        unsafe { syscall0(39) }
    }

    /// getppid() without libc
    pub fn raw_getppid() -> i64 {
        unsafe { syscall0(110) }
    }

    /// Direct memory allocation via mmap (bypasses hooked malloc)
    pub fn raw_mmap(addr: usize, len: usize, prot: i32, flags: i32, fd: i32, offset: i64) -> i64 {
        unsafe {
            syscall6_safe(
                9,
                addr as i64,
                len as i64,
                prot as i64,
                flags as i64,
                fd as i64,
                offset,
            )
        }
    }

    #[inline(always)]
    unsafe fn syscall6_safe(nr: i64, a1: i64, a2: i64, a3: i64, a4: i64, a5: i64, a6: i64) -> i64 {
        let ret: i64;
        std::arch::asm!(
            "syscall",
            in("rax") nr,
            in("rdi") a1, in("rsi") a2, in("rdx") a3,
            in("r10") a4, in("r8") a5, in("r9") a6,
            lateout("rax") ret,
            lateout("rcx") _, lateout("r11") _,
            options(nostack),
        );
        ret
    }

    /// Direct memory protection change (mprotect without libc)
    pub fn raw_mprotect(addr: usize, len: usize, prot: i32) -> i64 {
        unsafe { syscall3(10, addr as i64, len as i64, prot as i64) }
    }

    /// Direct write to fd (bypasses hooked write)
    pub fn raw_write(fd: i32, buf: &[u8]) -> i64 {
        unsafe { syscall3(1, fd as i64, buf.as_ptr() as i64, buf.len() as i64) }
    }

    /// Direct read from fd
    pub fn raw_read(fd: i32, buf: &mut [u8]) -> i64 {
        unsafe { syscall3(0, fd as i64, buf.as_ptr() as i64, buf.len() as i64) }
    }

    /// Direct open (bypasses hooked open)
    pub fn raw_open(path: &str, flags: i32, mode: i32) -> i64 {
        let cpath = std::ffi::CString::new(path).unwrap();
        unsafe { syscall3(2, cpath.as_ptr() as i64, flags as i64, mode as i64) }
    }

    /// Direct close
    pub fn raw_close(fd: i32) -> i64 {
        unsafe { syscall3(3, fd as i64, 0, 0) }
    }

    /// Fork without libc
    pub fn raw_fork() -> i64 {
        unsafe { syscall0(57) }
    }

    /// Check if being traced (ptrace self-check without libc)
    pub fn is_traced() -> bool {
        // prctl(PR_GET_DUMPABLE, ...) - if 0, might be traced
        // More direct: try to ptrace self and see if it fails
        let ret = unsafe { syscall4(101, 0, 0, 0, 0) }; // ptrace(PTRACE_TRACEME)
        ret != 0
    }

    /// Get TID directly
    pub fn raw_gettid() -> i64 {
        unsafe { syscall0(186) }
    }

    /// memfd_create without libc (for fileless execution)
    pub fn raw_memfd_create(name: &str, flags: u32) -> i64 {
        let cname = std::ffi::CString::new(name).unwrap();
        unsafe { syscall2(319, cname.as_ptr() as i64, flags as i64) }
    }
}

// ── Windows NT syscalls (stub - requires ntapi crate) ────────────────────────


// ── Re-exports ───────────────────────────────────────────────────────────────

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub use linux::*;

