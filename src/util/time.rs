use std::fmt::{self, Display, Formatter};
use std::time::Duration;

macro_rules! format_unit {
    ($f:expr, $unit:ident, $str:literal, $plural_str:literal, $round:expr, $v:expr) => {
        let $unit = $v;
        if $unit == 1 {
            return $f.write_fmt(format_args!("1 {}", $str));
        }
        if $unit < $round {
            return $f.write_fmt(format_args!("{} {}", $unit, $plural_str));
        }
    };
}

/// A lazy uptime-style string that represents a duration.
///
/// ## Panics
///
/// Implementation of `Display::fmt` will panic if days value from the
/// duration exceeds `u64::MAX`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct FormattedUptime(Duration);

impl FormattedUptime {
    /// Constructs a new `FormattedUptime` with the specified duration.
    #[inline]
    pub fn new(duration: Duration) -> Self {
        Self(duration)
    }

    /// Returns `true` if `self` is less than one second.
    #[inline]
    pub fn is_just_now(&self) -> bool {
        self.0.as_secs() == 0
    }
}

impl From<Duration> for FormattedUptime {
    fn from(value: Duration) -> Self {
        Self::new(value)
    }
}

impl Display for FormattedUptime {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        format_unit!(f, secs, "second", "seconds", 60, self.0.as_secs());
        format_unit!(f, mins, "minute", "minutes", 60, secs / 60);
        format_unit!(f, hours, "hour", "hours", 24, mins / 60);
        format_unit!(f, days, "day", "days", u64::MAX, hours / 24);

        unreachable!("there is no next time unit after 'day'");
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::FormattedUptime;

    #[test]
    fn test_format() {
        assert!(FormattedUptime::new(Duration::from_secs(0)).is_just_now());
        assert_eq!(
            FormattedUptime::new(Duration::from_secs(0)).to_string(),
            "0 seconds"
        );

        assert!(!FormattedUptime::new(Duration::from_secs(1)).is_just_now());
        assert_eq!(
            FormattedUptime::new(Duration::from_secs(1)).to_string(),
            "1 second"
        );

        assert_eq!(
            FormattedUptime::new(Duration::from_secs(6)).to_string(),
            "6 seconds"
        );

        assert_eq!(
            FormattedUptime::new(Duration::from_secs(80)).to_string(),
            "1 minute"
        );

        assert_eq!(
            FormattedUptime::new(Duration::from_secs(10800)).to_string(),
            "3 hours"
        );

        assert_eq!(
            FormattedUptime::new(Duration::from_secs(604_800)).to_string(),
            "7 days"
        );
    }
}
