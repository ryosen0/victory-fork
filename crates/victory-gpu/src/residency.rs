//! Host-memory admission for speculative texture work and reusable upload
//! storage.
//!
//! Port of the coolfox line's `ab75ac3e` and `e05d4633` (32 GB RAM /
//! Windows 11 commit exhaustion), brought into this tree: when the PC is
//! short of commit, speculative texture work is not started and upload
//! storage is not grown until the shortage passes, so the title's own
//! allocations still find room instead of the driver refusing them
//! (`E_OUTOFMEMORY`) and the run dying of it.
//!
//! Here, hand-carried: our tree had no `residency.rs` and no `pool.rs` (the
//! coolfox line's own texture-memory pool); the growth backoff of that pool
//! is applied to the heap growth this tree does have
//! (`crate::tiles::take`, `CreateHeap`).
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

const GIB: u64 = 1 << 30;
static PRESSURED: AtomicBool = AtomicBool::new(false);
static HEADROOM: AtomicU64 = AtomicU64::new(u64::MAX);

fn memory_status() -> Option<MEMORYSTATUSEX> {
    // SAFETY: the API initializes the fields of a correctly sized structure.
    let mut status: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
    status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
    (unsafe { GlobalMemoryStatusEx(&mut status) } != 0).then_some(status)
}

// The texture sweep may run for seconds inside a single submitted frame.
static NEXT_COMMIT_SAMPLE: AtomicU64 = AtomicU64::new(0);
static AVAILABLE_COMMIT: AtomicU64 = AtomicU64::new(u64::MAX);
static RETRY_UPLOAD_AT: AtomicU64 = AtomicU64::new(0);

/// A driver allocation failed: uploads wait half a second before they try
/// again (a shortage rarely passes in a frame, and every texture of a pass
/// asking once made a shortage of its own).
pub(crate) fn allocation_error(error: &windows::core::Error) {
    if error.code() == windows::Win32::Foundation::E_OUTOFMEMORY {
        RETRY_UPLOAD_AT.fetch_max(crate::host::now_ms().saturating_add(500), Ordering::Relaxed);
    }
}

/// Whether an upload that just failed is still in its half second of
/// backoff: the texture pass stops here rather than trying the next
/// texture's allocation against the same shortage.
pub(crate) fn upload_refused() -> bool {
    crate::host::now_ms() < RETRY_UPLOAD_AT.load(Ordering::Relaxed)
}

fn has_upload_reserve(commit: u64, bake: bool) -> bool {
    commit >= if bake { 2 * GIB } else { 4 * GIB }
}

/// Leave commit for game allocations and submissions already in flight. Direct
/// texture consumers bypass this gate. Bake prefetch gets priority over lookahead.
pub(crate) fn admit_texture_pass(bake: bool) -> bool {
    let now = crate::host::now_ms();
    if now < RETRY_UPLOAD_AT.load(Ordering::Relaxed) {
        return false;
    }
    let next = NEXT_COMMIT_SAMPLE.load(Ordering::Relaxed);
    if now >= next
        && NEXT_COMMIT_SAMPLE
            .compare_exchange(next, now.saturating_add(32), Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        && let Some(status) = memory_status()
    {
        AVAILABLE_COMMIT.store(status.ullAvailPageFile, Ordering::Relaxed);
    }
    has_upload_reserve(AVAILABLE_COMMIT.load(Ordering::Relaxed), bake)
}

/// Whether the PC has 32 GiB of physical memory or less: spare upload
/// storage is kept smaller there (the coolfox line's 32 GB setting).
pub(crate) fn limited_host_ram() -> bool {
    static LIMITED: OnceLock<bool> = OnceLock::new();
    *LIMITED.get_or_init(|| memory_status().is_some_and(|s| s.ullTotalPhys <= 32 * GIB))
}

fn next_pressure(was: bool, physical: u64, commit: u64) -> bool {
    if was {
        physical < 6 * GIB || commit < 8 * GIB
    } else {
        physical <= 4 * GIB || commit <= 6 * GIB
    }
}

/// Called by the once-per-second submission sweep. A failed sample keeps the
/// last good state.
pub(crate) fn refresh() {
    let Some(status) = memory_status() else { return };
    let (physical, commit) = (status.ullAvailPhys, status.ullAvailPageFile);
    HEADROOM.store(physical.min(commit), Ordering::Relaxed);
    let was = pressured();
    let pressure = next_pressure(was, physical, commit);
    if pressure != was {
        PRESSURED.store(pressure, Ordering::Relaxed);
        if pressure {
            // The numbers that said what was holding the memory: the copies
            // of title memory, the staging buffers, the spare chunks, the
            // host textures the title fills. (Only where those lines are
            // built in; a release build says the two that always exist.)
            #[cfg(feature = "diagnostics")]
            let (mirror_span_mib, staging_mib, spare_scratch_mib, filled_texture_mib) = {
                let shadow = crate::shadow::memory();
                let texture = crate::texture::memory();
                (
                    shadow[3] >> 20,
                    texture[1] >> 20,
                    crate::host::ALIVE[4].load(Ordering::Relaxed) >> 20,
                    crate::resource::KINDS[1].load(Ordering::Relaxed) >> 20,
                )
            };
            #[cfg(not(feature = "diagnostics"))]
            let (mirror_span_mib, staging_mib, spare_scratch_mib, filled_texture_mib) = (0u64, 0u64, 0u64, 0u64);
            tracing::warn!(
                pid = std::process::id(),
                frame = crate::frame::current(),
                physical_free_mib = physical >> 20,
                available_commit_mib = commit >> 20,
                buffer_mirror_span_mib = mirror_span_mib,
                texture_staging_mib = staging_mib,
                spare_scratch_mib,
                game_filled_gpu_texture_mib = filled_texture_mib,
                "host memory pressure: limiting texture prefetch and idle upload storage"
            );
        } else {
            tracing::info!(
                physical_free_mib = physical >> 20,
                available_commit_mib = commit >> 20,
                "host memory pressure cleared"
            );
        }
    }
}

/// The lowest of physical and commit memory free at the last sweep, if one
/// has run.
pub(crate) fn headroom() -> Option<u64> {
    let bytes = HEADROOM.load(Ordering::Relaxed);
    (bytes != u64::MAX).then_some(bytes)
}

pub(crate) fn pressured() -> bool {
    PRESSURED.load(Ordering::Relaxed)
}

/// A soft per-frame admission limit. A single texture can cross it. The
/// queue retains unserved slots so they can be retried on later frames.
pub(crate) fn prefetch_limit(bytes: u64, pressure: bool, limited: bool, staging: u64) -> u64 {
    if pressure {
        bytes.min(16 << 20)
    } else if limited && staging >= GIB {
        bytes.min(32 << 20)
    } else {
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bake_and_lookahead_leave_room_for_live_game_allocations() {
        for (commit, bake, lookahead) in [
            (277 << 20, false, false),
            (2 * GIB, true, false),
            (4 * GIB, true, true),
        ] {
            assert_eq!(has_upload_reserve(commit, true), bake);
            assert_eq!(has_upload_reserve(commit, false), lookahead);
        }
    }

    #[test]
    fn either_constraint_enters_pressure_and_both_must_recover() {
        assert!(next_pressure(false, 12 * GIB, 6 * GIB));
        assert!(next_pressure(false, 4 * GIB, 16 * GIB));
        assert!(next_pressure(true, 5 * GIB, 9 * GIB));
        assert!(next_pressure(true, 7 * GIB, 7 * GIB));
        assert!(!next_pressure(true, 6 * GIB, 8 * GIB));
        assert!(!next_pressure(false, 5 * GIB, 7 * GIB));
    }

    #[test]
    fn prefetch_respects_existing_budgets_and_resumes_after_recovery() {
        assert_eq!(prefetch_limit(512 << 20, false, true, GIB), 32 << 20);
        assert_eq!(prefetch_limit(512 << 20, false, true, GIB - 1), 512 << 20);
        assert_eq!(prefetch_limit(512 << 20, false, false, 2 * GIB), 512 << 20);
        assert_eq!(prefetch_limit(0, true, false, 0), 0);
        assert_eq!(prefetch_limit(8 << 20, true, false, 0), 8 << 20);
        assert_eq!(prefetch_limit(512 << 20, true, false, 0), 16 << 20);
        assert_eq!(prefetch_limit(192 << 20, false, false, 0), 192 << 20);
    }
}
