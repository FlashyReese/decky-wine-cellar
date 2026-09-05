use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

const SPEED_WINDOW: Duration = Duration::from_secs(5);
const SAMPLE_INTERVAL: Duration = Duration::from_millis(250);
const MAX_SAMPLES: usize = 32;

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct DownloadProgress {
    pub bytes_downloaded: u64,
    pub total_bytes: Option<u64>,
    pub bytes_per_second: Option<u64>,
    pub eta_seconds: Option<u64>,
    pub elapsed_seconds: u64,
}

impl DownloadProgress {
    pub fn percentage(&self) -> u8 {
        match self.total_bytes.filter(|total| *total > 0) {
            Some(total) => {
                ((u128::from(self.bytes_downloaded) * 100 / u128::from(total)).min(100)) as u8
            }
            None => 0,
        }
    }
}

#[derive(Clone, Copy)]
struct Sample {
    at: Instant,
    bytes: u64,
}

pub struct DownloadProgressTracker {
    total_bytes: Option<u64>,
    started_at: Instant,
    samples: VecDeque<Sample>,
}

impl DownloadProgressTracker {
    pub fn new(total_bytes: Option<u64>, now: Instant) -> Self {
        let mut samples = VecDeque::with_capacity(MAX_SAMPLES);
        samples.push_back(Sample { at: now, bytes: 0 });
        Self {
            total_bytes: total_bytes.filter(|total| *total > 0),
            started_at: now,
            samples,
        }
    }

    /// Poll even when no bytes arrive, so a stalled download's speed decays to zero.
    pub fn snapshot(&mut self, bytes_downloaded: u64, now: Instant) -> DownloadProgress {
        let current = Sample {
            at: now,
            bytes: bytes_downloaded,
        };

        // Keep the observation immediately before the window for interpolation.
        while self.samples.len() > 1
            && now.saturating_duration_since(self.samples[1].at) >= SPEED_WINDOW
        {
            self.samples.pop_front();
        }

        // Fast callers still use their current observation for the rate, without
        // retaining every poll or allocating a history proportional to chunks.
        if now.saturating_duration_since(self.samples.back().unwrap().at) >= SAMPLE_INTERVAL {
            if self.samples.len() == MAX_SAMPLES {
                self.samples.pop_front();
            }
            self.samples.push_back(current);
        }

        let first = self.samples[0];
        let window_start = now
            .checked_sub(SPEED_WINDOW)
            .map(|cutoff| cutoff.max(first.at))
            .unwrap_or(first.at);
        let mut starting_bytes = first.bytes;

        if window_start > first.at {
            let next = self.samples.get(1).copied().unwrap_or(current);
            let sample_nanos = next.at.saturating_duration_since(first.at).as_nanos();
            if sample_nanos > 0 {
                let offset_nanos = window_start.saturating_duration_since(first.at).as_nanos();
                let bytes_between = u128::from(next.bytes.saturating_sub(first.bytes));
                starting_bytes = first
                    .bytes
                    .saturating_add((bytes_between * offset_nanos / sample_nanos) as u64);
            }
        }

        let elapsed_nanos = now.saturating_duration_since(window_start).as_nanos();
        let bytes_per_second = (elapsed_nanos > 0).then(|| {
            let bytes = u128::from(bytes_downloaded.saturating_sub(starting_bytes));
            (bytes * 1_000_000_000 / elapsed_nanos).min(u128::from(u64::MAX)) as u64
        });
        let eta_seconds = self.total_bytes.and_then(|total| {
            bytes_per_second.filter(|speed| *speed > 0).map(|speed| {
                let remaining = total.saturating_sub(bytes_downloaded);
                // Round up, so an unfinished download never claims zero seconds.
                remaining / speed + u64::from(remaining % speed != 0)
            })
        });

        DownloadProgress {
            bytes_downloaded,
            total_bytes: self.total_bytes,
            bytes_per_second,
            eta_seconds,
            elapsed_seconds: now.saturating_duration_since(self.started_at).as_secs(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steady_download_reports_speed_and_remaining_time() {
        let start = Instant::now();
        let mut tracker = DownloadProgressTracker::new(Some(10_001), start);
        for second in 1..=5 {
            let progress = tracker.snapshot(second * 1_000, start + Duration::from_secs(second));
            assert_eq!(progress.bytes_per_second, Some(1_000));
            assert_eq!(progress.eta_seconds, Some(11 - second));
            assert_eq!(progress.elapsed_seconds, second);
        }
    }

    #[test]
    fn startup_and_unknown_length_do_not_invent_an_eta() {
        let start = Instant::now();
        let mut tracker = DownloadProgressTracker::new(Some(1_000), start);
        let initial = tracker.snapshot(0, start);
        assert_eq!(initial.bytes_per_second, None);
        assert_eq!(initial.eta_seconds, None);
        assert_eq!(initial.percentage(), 0);
        let waiting = tracker.snapshot(0, start + Duration::from_secs(1));
        assert_eq!(waiting.bytes_per_second, Some(0));
        assert_eq!(waiting.eta_seconds, None);

        for total in [None, Some(0)] {
            let mut tracker = DownloadProgressTracker::new(total, start);
            let progress = tracker.snapshot(500, start + Duration::from_millis(500));
            assert_eq!(progress.total_bytes, None);
            assert_eq!(progress.bytes_per_second, Some(1_000));
            assert_eq!(progress.eta_seconds, None);
            assert_eq!(progress.percentage(), 0);
        }
    }

    #[test]
    fn stalled_download_speed_decays_and_eta_disappears() {
        let start = Instant::now();
        let mut tracker = DownloadProgressTracker::new(Some(10_000), start);
        for half_second in 1..=10 {
            tracker.snapshot(
                half_second * 500,
                start + Duration::from_millis(half_second * 500),
            );
        }

        let slowing = tracker.snapshot(5_000, start + Duration::from_millis(7_500));
        assert_eq!(slowing.bytes_per_second, Some(500));
        assert_eq!(slowing.eta_seconds, Some(10));
        let stalled = tracker.snapshot(5_000, start + Duration::from_secs(10));
        assert_eq!(stalled.bytes_per_second, Some(0));
        assert_eq!(stalled.eta_seconds, None);
        let still_stalled = tracker.snapshot(5_000, start + Duration::from_secs(60));
        assert_eq!(still_stalled.bytes_per_second, Some(0));
        assert_eq!(still_stalled.eta_seconds, None);
    }

    #[test]
    fn sparse_observations_use_actual_elapsed_time() {
        let start = Instant::now();
        let mut tracker = DownloadProgressTracker::new(Some(10_000), start);
        let progress = tracker.snapshot(1_000, start + Duration::from_secs(10));
        assert_eq!(progress.bytes_per_second, Some(100));
        assert_eq!(progress.eta_seconds, Some(90));
    }

    #[test]
    fn percentage_and_rate_do_not_overflow_or_exceed_their_limits() {
        let start = Instant::now();
        let mut tracker = DownloadProgressTracker::new(Some(1), start);
        let progress = tracker.snapshot(u64::MAX, start + Duration::from_nanos(1));
        assert_eq!(progress.percentage(), 100);
        assert_eq!(progress.bytes_per_second, Some(u64::MAX));
        assert_eq!(progress.eta_seconds, Some(0));

        let mut tracker = DownloadProgressTracker::new(Some(100), start);
        assert_eq!(
            tracker
                .snapshot(25, start + Duration::from_secs(1))
                .percentage(),
            25
        );
    }

    #[test]
    fn rapid_polling_keeps_a_bounded_history_and_uses_current_bytes() {
        let start = Instant::now();
        let mut tracker = DownloadProgressTracker::new(Some(30_000), start);
        for millis in 1..=20_000 {
            let progress = tracker.snapshot(millis, start + Duration::from_millis(millis));
            assert!(tracker.samples.len() <= MAX_SAMPLES);
            assert_eq!(progress.bytes_per_second, Some(1_000));
        }
        assert!(tracker.samples.len() <= 22);
    }
}
