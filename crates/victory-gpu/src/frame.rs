//! Frame pacing: the title sets a frame interval and waits for each frame
//! to begin. Ours is a clock started by the first interval set, ticking
//! once per interval; the token is the frame number. Each event kind
//! (origin, vblank, flip) happens once per tick, so two waits for the same
//! kind never end on the same tick: Grand Theft Auto VI polls the origin
//! with "token on time-out" (starting its next frame at once) and is paced
//! by a blocking wait for the flip before each present.
//!
//! shortcut: event offsets are ignored (every event is at the tick); tie
//! the clock to presentation if a title paces by them.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::object::{HRESULT, S_FALSE, S_OK, This};

/// What a wait returns when its time is up first.
const TIMEOUT: HRESULT = S_FALSE;
/// The frame-origin event: the one that starts frames and makes tokens.
const ORIGIN: u32 = 0;
/// Make the frame's token even if the wait times out (the title starts the
/// frame early).
const TOKEN_ON_TIMEOUT: u32 = 2;
const SIXTIETH: Duration = Duration::from_micros(16_667);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameRate {
    Fps30,
    Fps60,
}

impl std::str::FromStr for FrameRate {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "30" => Ok(Self::Fps30),
            "60" => Ok(Self::Fps60),
            _ => Err("expected 30 or 60"),
        }
    }
}

static RATE_OVERRIDE: Mutex<Option<FrameRate>> = Mutex::new(None);

/// Select the cadence for standard 30/60 Hz frame requests before the title
/// starts. Other frame intervals retain their title-defined timing.
pub fn set_frame_rate_override(rate: Option<FrameRate>) {
    *RATE_OVERRIDE.lock().unwrap_or_else(|e| e.into_inner()) = rate;
}

fn effective_interval(requested_us: u32, rate: Option<FrameRate>) -> Duration {
    // The console interval is in whole microseconds. Recognise both
    // roundings of 30/60 Hz and their 1000/1001 broadcast-rate variants.
    match (requested_us, rate) {
        (0 | 16_666..=16_667 | 16_683..=16_684 | 33_333..=33_334 | 33_366..=33_367, Some(rate)) => {
            match rate {
                FrameRate::Fps30 => Duration::from_micros(33_333),
                FrameRate::Fps60 => SIXTIETH,
            }
        }
        (0, None) => SIXTIETH,
        (us, _) => Duration::from_micros(u64::from(us)),
    }
}

fn frame_interval(requested_us: u32) -> Duration {
    let rate = *RATE_OVERRIDE.lock().unwrap_or_else(|e| e.into_inner());
    effective_interval(requested_us, rate)
}

struct Clock {
    start: Instant,
    interval: Duration,
    /// The tick number at `start` (ticks go on across interval changes).
    first: u64,
}

static CLOCK: Mutex<Option<Clock>> = Mutex::new(None);
static FRAME: AtomicU64 = AtomicU64::new(0);

fn clock() -> std::sync::MutexGuard<'static, Option<Clock>> {
    CLOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Device method: a frame every `length` microseconds. The period is how
/// many intervals a frame may take to reach the display (pipeline depth),
/// not part of the rate.
pub(crate) extern "system" fn set_interval(
    _: This,
    _output: This,
    length: u32,
    _period: u32,
    _flags: u32,
) -> HRESULT {
    let interval = frame_interval(length);
    tracing::info!(
        ?interval,
        requested_us = length,
        effective_us = interval.as_micros() as u64,
        "frame interval"
    );
    // A new interval starts from now, after the last tick handed out.
    let first = LAST
        .iter()
        .map(|t| t.load(Ordering::Relaxed))
        .max()
        .unwrap_or(0);
    *clock() = Some(Clock {
        start: Instant::now(),
        interval,
        first,
    });
    S_OK
}

/// Device method: events are scheduled at every tick.
pub(crate) extern "system" fn schedule(
    _: This,
    _kind: u32,
    _offset: u32,
    _list: *const u8,
    _flags: u32,
) -> HRESULT {
    S_OK
}

/// Frames begun so far (frame waits that ended).
pub(crate) fn current() -> u64 {
    FRAME.load(Ordering::Relaxed)
}

/// Per event kind (origin, vblank, flip and later kinds): the last tick a
/// wait for it ended on.
static LAST: [AtomicU64; 3] = [const { AtomicU64::new(0) }; 3];

fn last(kind: u32) -> &'static AtomicU64 {
    &LAST[(kind as usize).min(LAST.len() - 1)]
}

/// The next tick after now and after the last one a wait for `kind` ended
/// on (an event happens once per tick: a wait woken a little early must not
/// let the next one end on the same tick), and how long until it.
fn next_tick(kind: u32) -> (u64, Duration) {
    let mut clock = clock();
    let c = clock.get_or_insert_with(|| Clock {
        start: Instant::now(),
        interval: frame_interval(0),
        first: 0,
    });
    let elapsed = c.start.elapsed().as_nanos();
    let interval = c.interval.as_nanos().max(1);
    let tick =
        (c.first + (elapsed / interval + 1) as u64).max(last(kind).load(Ordering::Relaxed) + 1);
    let at = u128::from(tick - c.first) * interval;
    (
        tick,
        Duration::from_nanos(at.saturating_sub(elapsed) as u64),
    )
}

/// Device method: blocks until the next tick (or the time-out); an origin
/// wait then hands out the new frame's token. Other events keep the token
/// the title passes. Ticks are absolute (start plus whole intervals), so
/// lateness never accumulates.
pub(crate) extern "system" fn wait(
    _: This,
    kind: u32,
    timeout_ms: u32,
    _list: *const u8,
    flags: u32,
    token: *mut u64,
) -> HRESULT {
    let origin = kind == ORIGIN;
    let (tick, wait) = next_tick(kind);
    let limit = Duration::from_millis(u64::from(timeout_ms));
    let is_unlocked = std::env::var_os("VICTORY_UNLOCK_FPS").is_some();
    let hr = if is_unlocked {
        last(kind).fetch_max(tick, Ordering::Relaxed);
        S_OK
    } else if wait > limit {
        std::thread::sleep(limit);
        TIMEOUT
    } else {
        std::thread::sleep(wait);
        last(kind).fetch_max(tick, Ordering::Relaxed);
        S_OK
    };
    // A frame begun on a time-out takes no tick: its flip still does.
    if origin && (hr == S_OK || flags & TOKEN_ON_TIMEOUT != 0) {
        // Tokens count frames (consecutive), not ticks.
        let frame = FRAME.fetch_add(1, Ordering::Relaxed) + 1;
        if !token.is_null() {
            // SAFETY: checked non-null; the title passes its token.
            unsafe { token.write(frame) };
        }
    }
    #[cfg(feature = "diagnostics")]
    tracing::debug!(
        kind,
        timeout_ms,
        flags,
        hr,
        tick,
        wait_us = wait.as_micros() as u64,
        frame = current(),
        "frame wait"
    );
    hr
}

/// Device method: no statistics are kept, so none are returned.
pub(crate) extern "system" fn statistics(
    _: This,
    _token: u64,
    _kinds: u32,
    count: *mut u32,
    _out: *mut u8,
) -> HRESULT {
    if !count.is_null() {
        // SAFETY: checked non-null.
        unsafe { count.write(0) };
    }
    S_OK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_only_changes_standard_frame_rates() {
        for requested in [
            0, 16_666, 16_667, 16_683, 16_684, 33_333, 33_334, 33_366, 33_367,
        ] {
            let native = if requested == 0 { 16_667 } else { requested };
            assert_eq!(
                effective_interval(requested, None),
                Duration::from_micros(u64::from(native))
            );
            assert_eq!(
                effective_interval(requested, Some(FrameRate::Fps60)),
                SIXTIETH
            );
            assert_eq!(
                effective_interval(requested, Some(FrameRate::Fps30)),
                Duration::from_micros(33_333)
            );
        }
        for requested in [8_333, 20_000, 40_000, 41_667, 50_000, 100_000] {
            let native = Duration::from_micros(u64::from(requested));
            for rate in [None, Some(FrameRate::Fps30), Some(FrameRate::Fps60)] {
                assert_eq!(effective_interval(requested, rate), native);
            }
        }
    }

    #[test]
    fn waits_end_on_ticks_and_count_frames() {
        let null = std::ptr::null();
        assert_eq!(set_interval(null, null, 2_000, 1, 0), S_OK);
        let mut first = 0;
        assert_eq!(wait(null, 0, 1_000, std::ptr::null(), 0, &mut first), S_OK);
        let before = Instant::now();
        let mut second = 0;
        assert_eq!(wait(null, 0, 1_000, std::ptr::null(), 0, &mut second), S_OK);
        assert!(second > first);
        assert!(before.elapsed() <= Duration::from_millis(50));
        assert_eq!(wait(null, 0, 0, std::ptr::null(), 0, &mut second), TIMEOUT);
        // Tokens count frames, a frame started early on a time-out too.
        let mut early = 0;
        let flags = TOKEN_ON_TIMEOUT;
        assert_eq!(
            wait(null, 0, 0, std::ptr::null(), flags, &mut early),
            TIMEOUT
        );
        let mut next = 0;
        assert_eq!(wait(null, 0, 1_000, std::ptr::null(), 0, &mut next), S_OK);
        assert!(early == second + 1 && next == early + 1);
        assert!(LAST[0].load(Ordering::Relaxed) >= 2);

        // The title's loop (the clock is global, so in the same test): an
        // origin poll that times out (and starts the frame), then a
        // blocking flip wait. Each flip takes its own tick.
        const FLIP: u32 = 2;
        assert_eq!(set_interval(null, null, 5_000, 3, 0), S_OK);
        let mut token = 0;
        let flips: Vec<u64> = (0..4)
            .map(|_| {
                let _ = wait(null, 0, 0, std::ptr::null(), TOKEN_ON_TIMEOUT, &mut token);
                assert_eq!(
                    wait(null, FLIP, u32::MAX, std::ptr::null(), 0, &mut token),
                    S_OK
                );
                LAST[2].load(Ordering::Relaxed)
            })
            .collect();
        // The host can deschedule this test across several 5 ms ticks.
        // Each wait must consume a later tick even then.
        assert!(flips.windows(2).all(|w| w[1] > w[0]), "{flips:?}");
        // Even when a flip wait begins on the tick another just ended on.
        let (tick, _) = next_tick(FLIP);
        assert!(tick > flips[3]);

        // Changing the display cadence must not reset a guest frame token.
        let previous = token;
        set_frame_rate_override(Some(FrameRate::Fps60));
        assert_eq!(set_interval(null, null, 33_333, 3, 0), S_OK);
        assert_eq!(clock().as_ref().unwrap().interval, SIXTIETH);
        assert_eq!(
            wait(null, ORIGIN, 1_000, std::ptr::null(), 0, &mut token),
            S_OK
        );
        assert_eq!(token, previous + 1);
        set_frame_rate_override(None);
    }
}
