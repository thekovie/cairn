//! Idle-editing rules, kept pure so they can be tested without waiting.
//!
//! Activity means typing, pasting, inserting a picture, or using an editor
//! control. The browser reports those explicitly; merely having the page open
//! or moving the mouse does not count.

use std::time::Duration;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum IdleState {
    Active,
    /// Show "Are you still editing?" with the seconds left before release.
    Warning {
        seconds_left: u64,
    },
    /// The lock should be (or has been) released.
    Release,
}

pub fn evaluate(idle_for: Duration, warn_after: Duration, release_after: Duration) -> IdleState {
    if idle_for >= release_after {
        IdleState::Release
    } else if idle_for >= warn_after {
        IdleState::Warning {
            seconds_left: (release_after - idle_for).as_secs().max(1),
        }
    } else {
        IdleState::Active
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const M: u64 = 60;

    #[test]
    fn default_thresholds() {
        let (w, r) = (Duration::from_secs(15 * M), Duration::from_secs(20 * M));
        assert_eq!(
            evaluate(Duration::from_secs(14 * M), w, r),
            IdleState::Active
        );
        assert_eq!(
            evaluate(Duration::from_secs(15 * M), w, r),
            IdleState::Warning {
                seconds_left: 5 * M
            }
        );
        assert_eq!(
            evaluate(Duration::from_secs(20 * M), w, r),
            IdleState::Release
        );
    }
}
