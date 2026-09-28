//! Time zones.
//!
//! Everything Cairn writes to disk is UTC (unix seconds or UTC stamps). Only
//! *display* is converted, into the zone each person chooses in Settings.
//! Offsets are labelled "GMT+8" style, matching what browsers show, rather
//! than tz-database abbreviations (which can be ambiguous: Manila is "PST").

use jiff::Timestamp;
use jiff::tz::TimeZone;

use crate::error::{CairnError, Result};

/// Check that `name` is a known IANA time zone (for example "Asia/Manila").
pub fn validate_zone(name: &str) -> Result<()> {
    TimeZone::get(name)
        .map(|_| ())
        .map_err(|_| CairnError::BadRequest(format!("{name:?} is not a known timezone.")))
}

/// The zone to show times in: the configured one, else this computer's,
/// else UTC.
pub fn user_zone(configured: Option<&str>) -> TimeZone {
    configured
        .and_then(|name| TimeZone::get(name).ok())
        .or_else(|| TimeZone::try_system().ok())
        .unwrap_or(TimeZone::UTC)
}

/// Name of a zone for display ("Asia/Manila"), or "UTC".
pub fn zone_name(zone: &TimeZone) -> String {
    zone.iana_name().unwrap_or("UTC").to_string()
}

/// "GMT+8", "GMT-4", "GMT+5:30", or "GMT" for an offset in seconds.
pub fn offset_label(offset_seconds: i32) -> String {
    if offset_seconds == 0 {
        return "GMT".into();
    }
    let sign = if offset_seconds < 0 { '-' } else { '+' };
    let abs = offset_seconds.unsigned_abs();
    let (hours, minutes) = (abs / 3600, (abs % 3600) / 60);
    if minutes == 0 {
        format!("GMT{sign}{hours}")
    } else {
        format!("GMT{sign}{hours}:{minutes:02}")
    }
}

/// Today's calendar date in `zone`, as "YYYY-MM-DD".
pub fn today(zone: &TimeZone) -> String {
    Timestamp::now().to_zoned(zone.clone()).date().to_string()
}

/// A moment as "Sep 28, 2026, 2:30 PM GMT+8" in `zone`.
pub fn format_moment(unix_secs: i64, zone: &TimeZone) -> String {
    let Ok(ts) = Timestamp::from_second(unix_secs) else {
        return String::new();
    };
    let zoned = ts.to_zoned(zone.clone());
    let offset = offset_label(zoned.offset().seconds());
    format!("{} {offset}", zoned.strftime("%b %-d, %Y, %-I:%M %p"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_are_labelled_like_browsers() {
        assert_eq!(offset_label(8 * 3600), "GMT+8");
        assert_eq!(offset_label(-4 * 3600), "GMT-4");
        assert_eq!(offset_label(5 * 3600 + 1800), "GMT+5:30");
        assert_eq!(offset_label(0), "GMT");
    }
}
