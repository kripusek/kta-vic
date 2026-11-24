//! Which title memory the title wrote since a host copy took it (decision
//! 0007, "Copies of title memory"). The runtime reserves title GPU memory
//! with write watching (`victory-hle` xmem), so the kernel keeps a dirty bit
//! per 4 KiB page; asking for a range's written pages also clears them. Host
//! copies of the same memory can overlap (nested buffers, textures in
//! buffers), so what one asks is kept here for the others: a journal of
//! each page's last write, numbered by the asking.
//!
//! Physical-page views (xmem's sections) cannot be write-watched. There a
//! copy leaves each 64 KiB block it took read-only, and our vectored handler
//! takes the first write to it: it notes the block as written and gives the
//! write access back. The runtime tells us when it maps or unmaps views
//! ([`title_memory_remapped`]), which resets their protection.
//!
//! shortcut: writes into physical pages through another view (xmem's
//! `write_physical`, DirectStorage's page destinations) are not seen at
//! the guarded addresses; they go to the texture pool, whose textures are
//! hashed, not guarded. A system call writing into a guarded block fails
//! (none is known to).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;
use windows_sys::Win32::System::Memory::{MEMORY_BASIC_INFORMATION, VirtualQuery};

/// The unit tracked and copied: 64 KiB, the console's page (the kernel's
/// 4 KiB pages fold into it). Finer units made the checks of the world's
/// big buffers cost more than the copies they saved.
pub(crate) const PAGE: usize = 64 << 10;
/// The kernel's page, the unit of its answers.
const KERNEL_PAGE: usize = 4096;

struct Journal {
    /// Numbers the askings; 0 is "never".
    asked: u64,
    /// Page number → the asking that last found it written.
    written: BTreeMap<usize, u64>,
    /// Room for the kernel's answer.
    found: Vec<usize>,
}

static JOURNAL: Mutex<Journal> = Mutex::new(Journal {
    asked: 0,
    written: BTreeMap::new(),
    found: Vec::new(),
});

/// The pages of `[start, end)` (whole pages) as last written, for pages
/// written since tracking began: `(page number, asking)` in page order, and
/// this asking's number, which is higher than any in the list but those the
/// kernel just reported. `None` if the memory is not write-watched (nor,
/// with `guarded`, view memory, which is then guarded).
pub(crate) fn writes(start: usize, end: usize, guarded: bool) -> Option<(u64, Vec<(usize, u64)>)> {
    let (first, last) = (start / PAGE, end.div_ceil(PAGE));
    let mut j = JOURNAL.lock().unwrap_or_else(|e| e.into_inner());
    j.asked += 1;
    let now = j.asked;
    #[cfg(feature = "diagnostics")]
    let started = std::time::Instant::now();
    let (from, to) = (first * PAGE, last * PAGE);
    if !j.ask(from, to) && !j.by_allocation(from, to) {
        if !guarded {
            return None;
        }
        for page in guard(from, to)? {
            j.written.insert(page, now);
        }
    }
    #[cfg(feature = "diagnostics")]
    {
        use crate::copystat::{C, add};
        add(C::Watches, 1);
        add(C::WatchUs, started.elapsed().as_micros() as u64);
    }
    Some((
        now,
        j.written
            .range(first..last)
            .map(|(&p, &w)| (p, w))
            .collect(),
    ))
}

fn query(at: usize) -> Option<MEMORY_BASIC_INFORMATION> {
    let mut info = MEMORY_BASIC_INFORMATION::default();
    // SAFETY: the address is only queried and the output is correctly sized.
    let n = unsafe { VirtualQuery(at as *const _, &mut info, size_of_val(&info)) };
    (n != 0).then_some(info)
}

fn region_end(info: &MEMORY_BASIC_INFORMATION) -> usize {
    (info.BaseAddress as usize).saturating_add(info.RegionSize)
}

impl Journal {
    fn ask(&mut self, start: usize, end: usize) -> bool {
        use windows_sys::Win32::System::Memory::GetWriteWatch;
        const RESET: u32 = 1;
        let mut count = (end - start).div_ceil(KERNEL_PAGE);
        self.found.resize(count.max(self.found.len()), 0);
        let mut granularity = 0;
        // SAFETY: the answer buffer has room for the reported pages.
        let failed = unsafe {
            GetWriteWatch(
                RESET,
                start as *const _,
                end - start,
                self.found.as_mut_ptr().cast(),
                &mut count,
                &mut granularity,
            )
        } != 0;
        if failed {
            return false;
        }
        // The kernel has consumed these dirty bits. Publish them even if a
        // later allocation fails and this caller falls back to a whole copy.
        for &at in &self.found[..count] {
            self.written.insert(at / PAGE, self.asked);
        }
        true
    }

    fn note_uncommitted(&mut self, from: usize, to: usize) {
        // The page may contain previously committed bytes. Invalidate it
        // conservatively even if GetWriteWatch reports no dirty bits.
        for page in from / PAGE..to.div_ceil(PAGE) {
            self.written.insert(page, self.asked);
        }
    }

    /// The kernel refuses a query spanning several allocations. Retry at
    /// allocation boundaries. Reserved parts of a partially committed
    /// allocation are included in that allocation's write-watch query.
    fn by_allocation(&mut self, start: usize, end: usize) -> bool {
        use windows_sys::Win32::System::Memory::{MEM_COMMIT, MEM_FREE, MEM_MAPPED};
        let mut at = start;
        let mut next = query(at);
        while at < end {
            let Some(info) = next else { return false };
            if info.Type == MEM_MAPPED {
                return false;
            }
            let mut committed = info.State == MEM_COMMIT;
            let mut to = region_end(&info).min(end);
            if to <= at || (at == start && to == end) {
                return false;
            }
            if !committed {
                self.note_uncommitted(at, to);
            }
            while to < end {
                next = query(to);
                let Some(after) = next else { return false };
                if info.State == MEM_FREE
                    || after.State == MEM_FREE
                    || info.AllocationBase != after.AllocationBase
                {
                    break;
                }
                committed |= after.State == MEM_COMMIT;
                let next_end = region_end(&after).min(end);
                if next_end <= to {
                    return false;
                }
                if after.State != MEM_COMMIT {
                    self.note_uncommitted(to, next_end);
                }
                to = next_end;
            }
            if to <= at || (at == start && to == end) {
                return false;
            }
            if committed && !self.ask(at, to) {
                return false;
            }
            at = to;
        }
        true
    }
}

/// Guarded blocks of view memory.
const BLOCK: usize = PAGE;

struct Guards {
    /// Blocks we made read-only.
    protected: BTreeSet<usize>,
    /// Of those, the ones not written since a copy took them.
    clean: BTreeSet<usize>,
    /// Blocks seen wholly uncommitted. Revalidated on each guard pass.
    absent: BTreeSet<usize>,
}

static GUARDS: Mutex<Guards> = Mutex::new(Guards {
    protected: BTreeSet::new(),
    clean: BTreeSet::new(),
    absent: BTreeSet::new(),
});

fn guards() -> std::sync::MutexGuard<'static, Guards> {
    GUARDS.lock().unwrap_or_else(|e| e.into_inner())
}

/// For view memory: the pages of `[start, end)`'s blocks written since
/// they were last guarded (all of a block's pages), guarding them again
/// before the caller copies them. `None` if the memory is not a view.
fn guard(start: usize, end: usize) -> Option<Vec<usize>> {
    if !mapped(start) {
        return None;
    }
    install();
    let mut g = guards();
    let mut written = Vec::new();
    let mut run: Option<usize> = None;
    let (first, last) = (start / BLOCK, end.div_ceil(BLOCK));
    let (mut absent_until, mut clean_until) = (first, first);
    for block in first..=last {
        let mut open = block < last;
        if open && g.clean.contains(&block) {
            // A normally writable view can have been replaced without the
            // notification. Reuse the region description only for this pass.
            if block >= clean_until {
                clean_until = read_only_view_end(block);
            }
            open = block >= clean_until;
            if open {
                g.clean.remove(&block);
                g.protected.remove(&block);
            }
        }
        if open && g.absent.contains(&block) {
            if block >= absent_until {
                absent_until = query(block * BLOCK)
                    .and_then(|info| uncommitted_end(&info))
                    .unwrap_or(block);
            }
            open = block >= absent_until;
            if open {
                g.absent.remove(&block);
            }
        }
        if open {
            written.extend(block * BLOCK / PAGE..(block + 1) * BLOCK / PAGE);
            if g.protected.contains(&block) {
                // Written through the runtime, still read-only.
                g.clean.insert(block);
            } else {
                run.get_or_insert(block);
                continue;
            }
        }
        if let Some(from) = run.take() {
            protect(&mut g, from, block);
        }
    }
    Some(written)
}

/// Whether the current allocation at `at` is a mapped view. Asking only for
/// the allocation type avoids enumerating pages with identical protection,
/// which can span an entire large view after its blocks were guarded.
fn mapped(at: usize) -> bool {
    use windows_sys::Win32::System::Memory::{
        MEM_MAPPED, MEMORY_BASIC_INFORMATION, MemoryRegionInfo, QueryVirtualMemoryInformation,
        VirtualQuery, WIN32_MEMORY_REGION_INFORMATION,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    let mut region = WIN32_MEMORY_REGION_INFORMATION::default();
    // SAFETY: both queries describe an address without dereferencing it. The
    // output buffers match their information classes.
    let queried = unsafe {
        QueryVirtualMemoryInformation(
            GetCurrentProcess(),
            at as *const _,
            MemoryRegionInfo,
            (&raw mut region).cast(),
            size_of_val(&region),
            std::ptr::null_mut(),
        )
    };
    if queried != 0 {
        // These are the Windows SDK's named bitfields in declaration order.
        // windows-sys exposes only the union's raw bits.
        const PRIVATE: u32 = 1 << 0;
        const MAPPED_DATA_FILE: u32 = 1 << 1;
        const MAPPED_IMAGE: u32 = 1 << 2;
        const MAPPED_PAGE_FILE: u32 = 1 << 3;
        const MAPPED_PHYSICAL: u32 = 1 << 4;
        // SAFETY: this union was filled by the successful query above.
        match unsafe { region.Anonymous.Flags } {
            MAPPED_DATA_FILE | MAPPED_PAGE_FILE | MAPPED_PHYSICAL => return true,
            PRIVATE | MAPPED_IMAGE => return false,
            _ => {}
        }
    }
    // Unknown flag combinations and failed allocation queries keep the
    // VirtualQuery type check, including for a free address.
    let mut info = MEMORY_BASIC_INFORMATION::default();
    // SAFETY: the output buffer has the required layout and size.
    let n = unsafe { VirtualQuery(at as *const _, &mut info, size_of_val(&info)) };
    n != 0 && info.Type == MEM_MAPPED
}

fn read_only_view_end(block: usize) -> usize {
    use windows_sys::Win32::System::Memory::{MEM_COMMIT, MEM_MAPPED, PAGE_READONLY};
    query(block * BLOCK)
        .filter(|info| {
            info.State == MEM_COMMIT && info.Type == MEM_MAPPED && info.Protect == PAGE_READONLY
        })
        .map_or(block, |info| region_end(&info) / BLOCK)
}

/// The end of the complete uncommitted blocks in this region. A reserved
/// leading 4 KiB page does not prove the rest of its 64 KiB block is absent.
fn uncommitted_end(info: &MEMORY_BASIC_INFORMATION) -> Option<usize> {
    use windows_sys::Win32::System::Memory::{MEM_FREE, MEM_RESERVE};
    matches!(info.State, MEM_FREE | MEM_RESERVE).then(|| region_end(info) / BLOCK)
}

/// Protect only full blocks of writable mapped memory, within one queried
/// region at a time. A neighboring private allocation stays writable.
fn protect(g: &mut Guards, mut from: usize, to: usize) {
    use windows_sys::Win32::System::Memory::{
        MEM_COMMIT, MEM_MAPPED, PAGE_READONLY, PAGE_READWRITE, VirtualProtect,
    };

    while from < to {
        let Some(info) = query(from * BLOCK) else {
            break;
        };
        let end = (region_end(&info) / BLOCK).min(to);
        if end <= from {
            from += 1;
            continue;
        }
        if uncommitted_end(&info).is_some() {
            g.absent.extend(from..end);
        } else if info.State == MEM_COMMIT
            && info.Type == MEM_MAPPED
            && info.Protect == PAGE_READWRITE
        {
            let mut old = 0;
            // SAFETY: queried full blocks in one writable view region.
            let protected = unsafe {
                VirtualProtect(
                    (from * BLOCK) as *const _,
                    (end - from) * BLOCK,
                    PAGE_READONLY,
                    &mut old,
                )
            } != 0;
            if protected {
                g.protected.extend(from..end);
                g.clean.extend(from..end);
            }
        }
        from = end;
    }
}

/// Gives write access back to the guarded blocks of `[start, end)` and
/// notes them written; true if any was guarded.
fn open(g: &mut Guards, start: usize, end: usize) -> bool {
    use windows_sys::Win32::System::Memory::{PAGE_READWRITE, VirtualProtect};
    let blocks: Vec<usize> = g
        .protected
        .range(start / BLOCK..end.div_ceil(BLOCK))
        .copied()
        .collect();
    let mut opened = false;
    for block in blocks {
        g.protected.remove(&block);
        g.clean.remove(&block);
        if read_only_view_end(block) <= block {
            continue;
        }
        let mut old = 0;
        // SAFETY: a full read-only mapped block that we previously guarded.
        opened |=
            unsafe { VirtualProtect((block * BLOCK) as *const _, BLOCK, PAGE_READWRITE, &mut old) }
                != 0;
    }
    opened
}

/// Before our own write into title memory `[at, at + len)` through the
/// kernel (which does not fault into our handler): opens its guarded blocks.
/// The runtime calls it (as `kernel_will_write`) before the title's kernel
/// reads into its own memory.
pub fn writing(at: usize, len: usize) {
    open(&mut guards(), at, at + len);
}

/// The runtime mapped or unmapped title memory `[at, at + len)`: what it
/// shows is new and read-write, so it is no longer guarded.
pub fn title_memory_remapped(at: usize, len: usize) {
    let mut g = guards();
    let range = at / BLOCK..(at + len).div_ceil(BLOCK);
    let absent: Vec<_> = g.absent.range(range.clone()).copied().collect();
    for block in absent {
        g.absent.remove(&block);
    }
    let blocks: Vec<usize> = g.protected.range(range).copied().collect();
    for block in blocks {
        g.protected.remove(&block);
        g.clean.remove(&block);
    }
}

/// Registers the handler that takes writes to guarded blocks, once, ahead
/// of the crash reporter's.
fn install() {
    use windows_sys::Win32::Foundation::{EXCEPTION_ACCESS_VIOLATION, NTSTATUS};
    use windows_sys::Win32::System::Diagnostics::Debug::{
        AddVectoredExceptionHandler, EXCEPTION_CONTINUE_EXECUTION, EXCEPTION_CONTINUE_SEARCH,
        EXCEPTION_POINTERS,
    };
    unsafe extern "system" fn on_write(info: *mut EXCEPTION_POINTERS) -> i32 {
        // SAFETY: the system passes the exception being dispatched.
        let r = unsafe { &*(*info).ExceptionRecord };
        const WRITE: usize = 1;
        if r.ExceptionCode != EXCEPTION_ACCESS_VIOLATION as NTSTATUS
            || r.NumberParameters < 2
            || r.ExceptionInformation[0] != WRITE
        {
            return EXCEPTION_CONTINUE_SEARCH;
        }
        let at = r.ExceptionInformation[1];
        // Held across both checks: another thread may have opened the block
        // since this one faulted, and a copy may have guarded it again since
        // (then it is ours again, not read-only for some other reason).
        let mut g = guards();
        if open(&mut g, at, at + 1) {
            #[cfg(feature = "diagnostics")]
            crate::copystat::add(crate::copystat::C::GuardFaults, 1);
            return EXCEPTION_CONTINUE_EXECUTION;
        }
        if writable(at) {
            EXCEPTION_CONTINUE_EXECUTION
        } else {
            EXCEPTION_CONTINUE_SEARCH
        }
    }
    static ONCE: std::sync::Once = std::sync::Once::new();
    // SAFETY: the handler only touches our guard state and protections.
    ONCE.call_once(|| unsafe {
        AddVectoredExceptionHandler(1, Some(on_write));
    });
}

/// Whether `at` is committed read-write memory now.
fn writable(at: usize) -> bool {
    use windows_sys::Win32::System::Memory::{
        MEM_COMMIT, MEMORY_BASIC_INFORMATION, PAGE_READWRITE, VirtualQuery,
    };
    // SAFETY: plain data filled in by the query.
    let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
    // SAFETY: a valid out pointer of the right size.
    let n = unsafe { VirtualQuery(at as *const _, &mut info, size_of_val(&info)) };
    n != 0 && info.State == MEM_COMMIT && info.Protect == PAGE_READWRITE
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::System::Memory::{
        MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE, VirtualAlloc, VirtualFree,
    };
    const MEM_WRITE_WATCH: u32 = 0x20_0000;

    fn with_adjacent_private(second_flags: u32, test: impl FnOnce(usize)) {
        // SAFETY: the probe identifies an address for two adjacent allocations.
        let probe =
            unsafe { VirtualAlloc(std::ptr::null(), 4 * PAGE, MEM_RESERVE, PAGE_READWRITE) };
        assert!(!probe.is_null());
        // SAFETY: the probe is still reserved and owned by the fixture.
        let released = unsafe { VirtualFree(probe, 0, MEM_RELEASE) };
        assert_ne!(released, 0);
        struct Owned(usize);
        impl Drop for Owned {
            fn drop(&mut self) {
                // SAFETY: the owned allocation has no other users.
                unsafe { VirtualFree(self.0 as *mut _, 0, MEM_RELEASE) };
            }
        }
        let alloc = |at, flags| {
            // SAFETY: an exact address in the range identified by the probe.
            let got = unsafe {
                VirtualAlloc(
                    at as *const _,
                    2 * PAGE,
                    MEM_RESERVE | MEM_COMMIT | flags,
                    PAGE_READWRITE,
                )
            };
            assert_eq!(got as usize, at);
            Owned(at)
        };
        let first = alloc(probe as usize, MEM_WRITE_WATCH);
        let _second = alloc(first.0 + 2 * PAGE, second_flags);
        test(first.0);
    }

    #[test]
    fn cross_allocation_queries_keep_writes_even_when_a_later_allocation_is_unwatched() {
        for second_flags in [MEM_WRITE_WATCH, 0] {
            with_adjacent_private(second_flags, |base| {
                let mut journal = Journal {
                    asked: 1,
                    written: BTreeMap::new(),
                    found: Vec::new(),
                };
                // SAFETY: both addresses are in committed pages owned by the fixture.
                unsafe {
                    (base as *mut u8).write(1);
                    ((base + 3 * PAGE) as *mut u8).write(2);
                }
                assert!(!journal.ask(base, base + 4 * PAGE));
                assert_eq!(
                    journal.by_allocation(base, base + 4 * PAGE),
                    second_flags != 0
                );
                assert_eq!(journal.written.get(&(base / PAGE)), Some(&1));
                if second_flags != 0 {
                    assert_eq!(journal.written.get(&(base / PAGE + 3)), Some(&1));
                }
                if second_flags != 0 {
                    use windows_sys::Win32::System::Memory::MEM_DECOMMIT;
                    // SAFETY: release only the last 4 KiB of fixture allocation A.
                    let decommitted = unsafe {
                        VirtualFree(
                            (base + 2 * PAGE - KERNEL_PAGE) as *mut _,
                            KERNEL_PAGE,
                            MEM_DECOMMIT,
                        )
                    };
                    assert_ne!(decommitted, 0);
                    journal.asked += 1;
                    assert!(journal.by_allocation(base, base + 4 * PAGE));
                    assert_eq!(journal.written.get(&(base / PAGE + 1)), Some(&2));
                }
                // A later ask for just the first allocation sees the journal entry
                // even though the failed broader query consumed the kernel bit.
                journal.asked += 1;
                assert!(journal.ask(base, base + 2 * PAGE));
                assert_eq!(journal.written.get(&(base / PAGE)), Some(&1));
            });
        }
    }

    #[test]
    fn guard_rechecks_fully_reserved_blocks_and_keeps_partial_blocks_dirty() {
        use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
        use windows_sys::Win32::System::Memory::{
            CreateFileMappingW, FILE_MAP_WRITE, MEMORY_MAPPED_VIEW_ADDRESS, MapViewOfFile,
            SEC_RESERVE, UnmapViewOfFile,
        };
        const SIZE: usize = 4 * PAGE;
        struct View(MEMORY_MAPPED_VIEW_ADDRESS);
        impl Drop for View {
            fn drop(&mut self) {
                title_memory_remapped(self.0.Value as usize, SIZE);
                // SAFETY: the owned view is still mapped.
                unsafe { UnmapViewOfFile(self.0) };
            }
        }
        // SAFETY: a new pagefile section and a view owned by the fixture.
        let view = unsafe {
            let section = CreateFileMappingW(
                INVALID_HANDLE_VALUE,
                std::ptr::null(),
                PAGE_READWRITE | SEC_RESERVE,
                0,
                SIZE as u32,
                std::ptr::null(),
            );
            assert!(!section.is_null());
            let view = MapViewOfFile(section, FILE_MAP_WRITE, 0, 0, SIZE);
            CloseHandle(section);
            assert!(!view.Value.is_null());
            View(view)
        };
        let at = view.0.Value as usize;
        let commit = |offset, len| {
            // SAFETY: the range lies inside the owned reserved section.
            let committed =
                unsafe { VirtualAlloc((at + offset) as *const _, len, MEM_COMMIT, PAGE_READWRITE) };
            assert_eq!(committed as usize, at + offset);
        };
        commit(0, PAGE);
        commit(PAGE + KERNEL_PAGE, KERNEL_PAGE);
        let base = at / PAGE;
        assert_eq!(
            guard(at, at + SIZE).unwrap(),
            [base, base + 1, base + 2, base + 3]
        );
        assert_eq!(guard(at, at + SIZE).unwrap(), [base + 1]);

        // Ordinary commit without notification, then an explicitly notified
        // change. Both invalidate previously absent blocks.
        commit(2 * PAGE + KERNEL_PAGE, KERNEL_PAGE);
        assert_eq!(guard(at, at + SIZE).unwrap(), [base + 1, base + 2]);
        commit(3 * PAGE, PAGE);
        title_memory_remapped(at + 3 * PAGE, PAGE);
        assert_eq!(
            guard(at, at + SIZE).unwrap(),
            [base + 1, base + 2, base + 3]
        );
        assert_eq!(guard(at, at + SIZE).unwrap(), [base + 1, base + 2]);
    }

    #[test]
    fn guard_leaves_adjacent_private_memory_writable_and_detects_view_replacement() {
        use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
        use windows_sys::Win32::System::Memory::{
            CreateFileMappingW, FILE_MAP_WRITE, MapViewOfFileEx, UnmapViewOfFile,
        };
        // SAFETY: the probe identifies space for a mapped and a private page.
        let probe =
            unsafe { VirtualAlloc(std::ptr::null(), 2 * PAGE, MEM_RESERVE, PAGE_READWRITE) };
        assert!(!probe.is_null());
        // SAFETY: the probe is still reserved and owned by the fixture.
        let released = unsafe { VirtualFree(probe, 0, MEM_RELEASE) };
        assert_ne!(released, 0);
        let base = probe as usize;
        // SAFETY: the address is in the just-released probe range.
        let private = unsafe {
            VirtualAlloc(
                (base + PAGE) as *const _,
                PAGE,
                MEM_RESERVE | MEM_COMMIT,
                PAGE_READWRITE,
            )
        };
        assert_eq!(private as usize, base + PAGE);
        let map = || {
            // SAFETY: a new writable section mapped at the released address.
            unsafe {
                let section = CreateFileMappingW(
                    INVALID_HANDLE_VALUE,
                    std::ptr::null(),
                    PAGE_READWRITE,
                    0,
                    PAGE as u32,
                    std::ptr::null(),
                );
                assert!(!section.is_null());
                let view = MapViewOfFileEx(section, FILE_MAP_WRITE, 0, 0, PAGE, probe);
                CloseHandle(section);
                assert_eq!(view.Value as usize, base);
                view
            }
        };
        let view = map();
        let refresh = || guard(base, base + 2 * PAGE).unwrap();
        assert_eq!(refresh(), [base / PAGE, base / PAGE + 1]);
        assert!(!writable(base));
        assert!(writable(base + PAGE));
        assert_eq!(refresh(), [base / PAGE + 1]);
        // SAFETY: the first fixture view is replaced by a new writable view.
        assert_ne!(unsafe { UnmapViewOfFile(view) }, 0);
        let replacement = map();
        assert_eq!(refresh(), [base / PAGE, base / PAGE + 1]);
        title_memory_remapped(base, PAGE);
        // SAFETY: the fixture owns both allocations.
        unsafe {
            UnmapViewOfFile(replacement);
            VirtualFree((base + PAGE) as *mut _, 0, MEM_RELEASE);
        }
    }

    #[test]
    fn allocation_query_recognizes_file_views_and_rejects_nonviews() {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Memory::{
            CreateFileMappingW, FILE_MAP_READ, MapViewOfFile, PAGE_READONLY, UnmapViewOfFile,
        };
        let file = tempfile::tempfile().unwrap();
        file.set_len(PAGE as u64).unwrap();
        // SAFETY: a valid, open file and a read-only mapping of its contents.
        let section = unsafe {
            CreateFileMappingW(
                file.as_raw_handle(),
                std::ptr::null(),
                PAGE_READONLY,
                0,
                0,
                std::ptr::null(),
            )
        };
        assert!(!section.is_null());
        // SAFETY: an open section containing at least one page.
        let view = unsafe { MapViewOfFile(section, FILE_MAP_READ, 0, 0, PAGE) };
        assert!(!view.Value.is_null());
        assert!(mapped(view.Value as usize + 13));
        // SAFETY: our successful mapping and its section handle.
        unsafe {
            UnmapViewOfFile(view);
            CloseHandle(section);
        }
        assert!(!mapped((&raw const file) as usize)); // private stack allocation
        assert!(!mapped(mapped as *const () as usize)); // executable image
        assert!(!mapped(0)); // no allocation to query
    }

    #[test]
    fn written_pages_are_reported_to_every_asker() {
        // SAFETY: a fresh write-watched allocation, freed at the end.
        let p = unsafe {
            VirtualAlloc(
                std::ptr::null(),
                4 * PAGE,
                MEM_RESERVE | MEM_COMMIT | MEM_WRITE_WATCH,
                PAGE_READWRITE,
            )
        } as usize;
        assert_ne!(p, 0);
        let (start, end) = (p, p + 4 * PAGE);
        let (baseline, before) = writes(start, end, false).expect("watched");
        // An earlier allocation may have occupied these addresses. No page
        // of the new allocation has been written during this asking.
        assert!(before.iter().all(|&(_, epoch)| epoch < baseline));
        // SAFETY: committed pages of ours.
        unsafe { ((p + PAGE + 8) as *mut u8).write(1) };
        let (now, mut after) = writes(start, end, false).expect("watched");
        after.retain(|&(_, epoch)| epoch > baseline);
        assert_eq!(after, [(p / PAGE + 1, now)]);
        // Another asker after the kernel's bit was cleared still sees it.
        let (later, again) = writes(p + PAGE, p + 2 * PAGE, false).expect("watched");
        assert!(later > now);
        assert_eq!(again, [(p / PAGE + 1, now)]);
        let mut plain = vec![0u8; PAGE];
        assert!(
            writes(
                plain.as_mut_ptr() as usize,
                plain.as_ptr() as usize + PAGE,
                true
            )
            .is_none()
        );
        // SAFETY: our allocation.
        unsafe { VirtualFree(p as *mut _, 0, MEM_RELEASE) };
    }

    /// A kernel read into a guarded view block fails; opened first, it
    /// lands and the block counts as written.
    #[test]
    fn kernel_reads_into_guarded_blocks_after_opening() {
        use std::io::{Read, Seek, Write};
        use windows_sys::Win32::System::Memory::{
            CreateFileMappingW, FILE_MAP_WRITE, MapViewOfFile, UnmapViewOfFile,
        };
        // SAFETY: a pagefile section of two blocks, mapped read-write.
        let view = unsafe {
            let section = CreateFileMappingW(
                -1isize as _,
                std::ptr::null(),
                PAGE_READWRITE,
                0,
                (2 * BLOCK) as u32,
                std::ptr::null(),
            );
            assert!(!section.is_null());
            MapViewOfFile(section, FILE_MAP_WRITE, 0, 0, 2 * BLOCK).Value as usize
        };
        assert_ne!(view, 0);
        assert!(guard(view, view + 2 * BLOCK).is_some()); // now read-only
        let mut file = tempfile::tempfile().unwrap();
        file.write_all(b"from the disk").unwrap();
        // `ReadFile` inside: the kernel writes the view.
        let read = |file: &mut std::fs::File| {
            file.rewind().unwrap();
            // SAFETY: 13 bytes of our view, nothing else refers to them.
            let into = unsafe { std::slice::from_raw_parts_mut((view + 100) as *mut u8, 13) };
            file.read(into).is_ok_and(|n| n == 13)
        };
        assert!(
            !read(&mut file),
            "the kernel does not fault into our handler"
        );
        writing(view + 100, 13);
        assert!(read(&mut file));
        // SAFETY: the bytes just read.
        assert_eq!(
            unsafe { std::slice::from_raw_parts((view + 100) as *const u8, 13) },
            b"from the disk"
        );
        assert_eq!(
            guard(view, view + 2 * BLOCK),
            Some((view / PAGE..view / PAGE + 1).collect())
        );
        // SAFETY: our view.
        unsafe {
            UnmapViewOfFile(
                windows_sys::Win32::System::Memory::MEMORY_MAPPED_VIEW_ADDRESS {
                    Value: view as *mut _,
                },
            )
        };
        title_memory_remapped(view, 2 * BLOCK);
    }
}
