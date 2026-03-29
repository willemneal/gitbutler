//! Tidal cycle clock -- the collective's temporal coordination mechanism.
//!
//! All coordination uses a fixed 6-hour cycle (a "tide"). Each cycle has four
//! 90-minute phases: flood, high, ebb, low. Decisions that cannot reach
//! consensus within one tide are deferred to the next.
//!
//! The tide prevents infinite negotiation loops. When the ebb phase begins,
//! agents must wrap up. When low tide arrives, it is maintenance time --
//! memory expiration, gossip synchronization, consensus review.

use crate::types::{TideMark, TidePhase, TidalConfig};

/// Duration of a single tide phase in seconds (90 minutes).
const PHASE_DURATION_SECONDS: u64 = 5400;

/// Duration of a full tide cycle in seconds (6 hours = 21600s).
const CYCLE_DURATION_SECONDS: u64 = 4 * PHASE_DURATION_SECONDS;

/// The tidal clock, anchored to a configurable epoch.
///
/// The epoch is the reference point from which tide cycles are counted.
/// By default this is Unix epoch (1970-01-01T00:00:00Z), but in practice
/// collectives may align their tides to local port schedules.
pub struct TidalClock {
    /// Cycle duration in seconds (default 21600 = 6 hours).
    cycle_seconds: u64,
    /// Phase duration in seconds (cycle_seconds / 4).
    phase_seconds: u64,
}

impl TidalClock {
    /// Create a tidal clock with the standard 6-hour cycle.
    pub fn new() -> Self {
        Self {
            cycle_seconds: CYCLE_DURATION_SECONDS,
            phase_seconds: PHASE_DURATION_SECONDS,
        }
    }

    /// Create a tidal clock from configuration.
    pub fn from_config(config: &TidalConfig) -> Self {
        let cycle_seconds = config.tide_cycle_hours * 3600;
        let phase_seconds = cycle_seconds / 4;
        Self {
            cycle_seconds,
            phase_seconds,
        }
    }

    /// Compute the current tide mark from a Unix timestamp (seconds since epoch).
    pub fn tide_at(&self, unix_seconds: u64) -> TideMark {
        let cycle = unix_seconds / self.cycle_seconds;
        let within_cycle = unix_seconds % self.cycle_seconds;
        let phase_index = within_cycle / self.phase_seconds;
        let phase_elapsed = within_cycle % self.phase_seconds;

        let phase = match phase_index {
            0 => TidePhase::Flood,
            1 => TidePhase::High,
            2 => TidePhase::Ebb,
            _ => TidePhase::Low,
        };

        TideMark {
            timestamp: format_unix_timestamp(unix_seconds),
            phase,
            cycle,
            phase_elapsed_seconds: phase_elapsed,
        }
    }

    /// Compute the Unix timestamp when the next phase begins.
    pub fn next_phase_start(&self, unix_seconds: u64) -> u64 {
        let within_cycle = unix_seconds % self.cycle_seconds;
        let current_phase_start = (within_cycle / self.phase_seconds) * self.phase_seconds;
        let next_phase_in_cycle = current_phase_start + self.phase_seconds;

        if next_phase_in_cycle >= self.cycle_seconds {
            // Next phase is the start of a new cycle
            unix_seconds - within_cycle + self.cycle_seconds
        } else {
            unix_seconds - within_cycle + next_phase_in_cycle
        }
    }

    /// Seconds remaining in the current phase.
    pub fn seconds_remaining_in_phase(&self, unix_seconds: u64) -> u64 {
        let within_cycle = unix_seconds % self.cycle_seconds;
        let phase_elapsed = within_cycle % self.phase_seconds;
        self.phase_seconds - phase_elapsed
    }

    /// Whether a deadline (in Unix seconds) falls within the current tide cycle.
    pub fn is_within_current_cycle(&self, now: u64, deadline: u64) -> bool {
        let current_cycle_start = (now / self.cycle_seconds) * self.cycle_seconds;
        let current_cycle_end = current_cycle_start + self.cycle_seconds;
        deadline >= current_cycle_start && deadline < current_cycle_end
    }

    /// Get the cycle duration in seconds.
    pub fn cycle_seconds(&self) -> u64 {
        self.cycle_seconds
    }
}

impl Default for TidalClock {
    fn default() -> Self {
        Self::new()
    }
}

/// Format a Unix timestamp as an ISO-8601 string.
///
/// This is a simplified formatter that does not handle timezones beyond UTC.
/// In production, use a proper datetime library.
fn format_unix_timestamp(secs: u64) -> String {
    // Simple UTC formatting: days since epoch, then hours/minutes/seconds.
    let days = secs / 86400;
    let remaining = secs % 86400;
    let hours = remaining / 3600;
    let minutes = (remaining % 3600) / 60;
    let seconds = remaining % 60;

    // Approximate year/month/day from days since epoch (simplified, no leap second handling).
    let (year, month, day) = days_to_ymd(days);

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hours, minutes, seconds
    )
}

/// Convert days since Unix epoch to (year, month, day).
/// Simplified algorithm -- handles leap years but not leap seconds.
fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    let mut year = 1970;

    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }

    let leap = is_leap_year(year);
    let month_days: [u64; 12] = if leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1;
    for &md in &month_days {
        if days < md {
            break;
        }
        days -= md;
        month += 1;
    }

    (year, month, days + 1)
}

fn is_leap_year(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Human-readable tide label (e.g. "high tide, cycle 1234").
pub fn tide_label(mark: &TideMark) -> String {
    format!("{} tide, cycle {}", mark.phase, mark.cycle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_cycle_at_epoch() {
        let clock = TidalClock::new();
        let mark = clock.tide_at(0);
        assert_eq!(mark.phase, TidePhase::Flood);
        assert_eq!(mark.cycle, 0);
        assert_eq!(mark.phase_elapsed_seconds, 0);
    }

    #[test]
    fn phase_transitions_every_90_minutes() {
        let clock = TidalClock::new();

        let flood = clock.tide_at(0);
        assert_eq!(flood.phase, TidePhase::Flood);

        let high = clock.tide_at(5400);
        assert_eq!(high.phase, TidePhase::High);

        let ebb = clock.tide_at(10800);
        assert_eq!(ebb.phase, TidePhase::Ebb);

        let low = clock.tide_at(16200);
        assert_eq!(low.phase, TidePhase::Low);

        // Next cycle starts at 21600
        let next_flood = clock.tide_at(21600);
        assert_eq!(next_flood.phase, TidePhase::Flood);
        assert_eq!(next_flood.cycle, 1);
    }

    #[test]
    fn seconds_remaining_accuracy() {
        let clock = TidalClock::new();
        // At 1000 seconds into the first phase
        let remaining = clock.seconds_remaining_in_phase(1000);
        assert_eq!(remaining, 5400 - 1000);
    }

    #[test]
    fn next_phase_start_correctness() {
        let clock = TidalClock::new();
        let next = clock.next_phase_start(1000);
        assert_eq!(next, 5400); // First phase ends at 5400
    }

    #[test]
    fn custom_cycle_from_config() {
        let config = TidalConfig {
            tide_cycle_hours: 12,
            ..Default::default()
        };
        let clock = TidalClock::from_config(&config);
        assert_eq!(clock.cycle_seconds(), 43200);
    }

    #[test]
    fn tide_label_formatting() {
        let mark = TideMark {
            timestamp: "2026-03-28T14:00:00Z".to_string(),
            phase: TidePhase::High,
            cycle: 1234,
            phase_elapsed_seconds: 0,
        };
        assert_eq!(tide_label(&mark), "high tide, cycle 1234");
    }
}
