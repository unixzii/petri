use std::fmt::{self, Display, Formatter};
use std::time::Duration;

/// Helper macro to generate a function that formats time values with
/// the specified carry rules.
macro_rules! define_formatter {
    (@
        // Internal state like identifiers of arguments, intermediate
        // local variables, etc.
        { $f: ident, $input:ident, $last:expr }

        // Expanded statements that will be pasted as-is.
        { $($expanded:tt)* } $(,)?
    ) => {
        |$f: &mut std::fmt::Formatter<'_>, $input: u64| {
            $($expanded)*
        }
    };

    // ===== Normalize =====

    (@
        { $f: ident, $input:ident, $last:expr }
        { $($expanded:tt)* }

        // Define a unit that has next units.
        , $unit:ident($singular:literal, $round:expr) $($rest:tt)*
    ) => {
        define_formatter!(@ { $f, $input, $unit / $round } {
            $($expanded)*
            let $unit = $last;
            if $unit == 1 {
                return $f.write_fmt(format_args!("1 {}", $singular));
            }
            if $unit < $round {
                return $f.write_fmt(format_args!("{} {}", $unit, stringify!($unit)));
            }
        } $($rest)*)
    };

    (@
        { $f: ident, $input:ident, $last:expr }
        { $($expanded:tt)* }

        // Define a final unit that doesn't carry to next units.
        , $unit:ident($singular:literal) $($rest:tt)*
    ) => {
        define_formatter!(@ { $f, $input, 0 } {
            $($expanded)*
            let $unit = $last;
            if $unit == 1 {
                return $f.write_fmt(format_args!("1 {}", $singular));
            }
            return $f.write_fmt(format_args!("{} {}", $unit, stringify!($unit)));
        } $($rest)*)
    };

    // ==== Entry Point ====

    ($unit:ident $($arg:tt)*) => {
        define_formatter!(@ { f, input, input } { } , $unit $($arg)*)
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
        let formatter = define_formatter! {
            seconds("second", 60),
            minutes("minute", 60),
            hours("hour", 24),
            days("day"),
        };
        formatter(f, self.0.as_secs())
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
