//! When a `needs` entry arrives in apr. A demo may need a verb or flag that
//! only a later, not-yet-released apr has; at today's pin that demo is refused
//! by name (`NotRun{PinUnreleased}`) instead of failing a help probe or a
//! version check, so the re-pin that unblocks it is visible in the verdict.
//!
//! The table is declared, not probed: an unreleased apr is by definition not
//! the forjar-installed one, so no probe of the host can say what it will have.
//! Evidence for each row is the help diff between the pinned apr and the
//! newer build.

use crate::pin::cmp_version;
use std::cmp::Ordering;

/// The newest apr release a demo may pin. Every version above it is
/// unreleased. Bump it when the next tag is cut.
pub const APR_LATEST_RELEASED: &str = "0.69.3";

/// `(need, first apr version that has it)`. A verb row matches a need that
/// begins with its words (`"apr ptx-debug"` matches `"apr ptx-debug analyze"`);
/// a flag row (`"--x"`) matches a need that contains that flag.
pub const APR_ARRIVALS: &[(&str, &str)] = &[
    // `apr ptx-debug --help`: unrecognized at 0.69.3, present at
    // 0.70.0 (d13f86934). The first pinnable 0.70 is rc.1 (a dev build is
    // never a pin).
    ("apr ptx-debug", "0.70.0-rc.1"),
];

/// The first apr version that has `need`, if the table declares one.
pub fn since(need: &str) -> Option<&'static str> {
    since_in(APR_ARRIVALS, need)
}

pub fn since_in(table: &[(&str, &'static str)], need: &str) -> Option<&'static str> {
    let words: Vec<&str> = need.split_whitespace().collect();
    table.iter().find_map(|(row, v)| {
        let row: Vec<&str> = row.split_whitespace().collect();
        let hit = match row.as_slice() {
            [flag] if flag.starts_with('-') => words.contains(flag),
            _ => !row.is_empty() && words.starts_with(&row),
        };
        hit.then_some(*v)
    })
}

/// `Some(since)` when `need` is absent at apr `pin` and arrives only in an
/// unreleased apr: `pin < since` and `since > latest_released`.
pub fn pin_unreleased(
    table: &[(&str, &'static str)],
    latest_released: &str,
    need: &str,
    pin: &str,
) -> Option<&'static str> {
    let since = since_in(table, need)?;
    let below = cmp_version(pin, since) == Some(Ordering::Less);
    let unreleased = cmp_version(since, latest_released) == Some(Ordering::Greater);
    (below && unreleased).then_some(since)
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: &[(&str, &str)] = &[("apr ptx-debug", "0.70.0-rc.1"), ("--new-flag", "0.70.0")];

    #[test]
    fn verb_rows_match_by_word_prefix_and_flag_rows_by_membership() {
        assert_eq!(since_in(T, "apr ptx-debug"), Some("0.70.0-rc.1"));
        assert_eq!(since_in(T, "apr ptx-debug analyze"), Some("0.70.0-rc.1"));
        assert_eq!(since_in(T, "apr ptx"), None, "not a word prefix");
        assert_eq!(since_in(T, "apr run --new-flag"), Some("0.70.0"));
        assert_eq!(since_in(T, "--new-flag"), Some("0.70.0"));
        assert_eq!(since_in(T, "apr serve run"), None);
    }

    #[test]
    fn fires_only_below_an_unreleased_since() {
        let f = |need, pin| pin_unreleased(T, "0.69.3", need, pin);
        assert_eq!(f("apr ptx-debug", "0.69.3"), Some("0.70.0-rc.1"));
        assert_eq!(f("apr ptx-debug", "0.70.0-rc.1"), None, "pin has it");
        assert_eq!(f("apr serve run", "0.69.3"), None, "not in the table");
        // Once since is released, an old pin is an ordinary Refused.
        assert_eq!(
            pin_unreleased(T, "0.70.0-rc.1", "apr ptx-debug", "0.69.3"),
            None
        );
    }

    /// The shipped table's rows are all well-formed versions.
    #[test]
    fn shipped_table_is_semver() {
        for (need, v) in APR_ARRIVALS {
            assert!(cmp_version(v, APR_LATEST_RELEASED).is_some(), "{need}: {v}");
        }
    }
}
