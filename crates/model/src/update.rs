//! Telling the owner when a new version of MailCraft is out: what a version is, and what is
//! kept of a release that was found. Plain data: the window decides when to ask, a host with
//! sockets (`mailcraft-net`) does the asking, and neither installs anything.
//!
//! A version is read strictly: `MAJOR.MINOR.PATCH` in digits, a `v` before it allowed, and a
//! pre-release's suffix after a `-` (`0.5.0-beta.1`). Anything else is not a version at all.

use std::cmp::Ordering;

/// Most digits in one number of a version.
const MAX_DIGITS: usize = 6;
/// Longest pre-release suffix taken.
const MAX_SUFFIX: usize = 40;

/// A version of MailCraft, as the build or a release's tag names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    /// What follows the `-` of a pre-release (`beta.1`); empty for a release.
    pub suffix: String,
}

impl Version {
    /// `v?MAJOR.MINOR.PATCH`, with `-suffix` for a pre-release (letters, digits, dots and dashes).
    /// `None` for anything else: no spaces, no `+build`, no leading zeros, nothing left over.
    pub fn parse(text: &str) -> Option<Version> {
        let text = text.strip_prefix('v').unwrap_or(text);
        let (numbers, suffix) = match text.split_once('-') {
            Some((numbers, suffix)) => (numbers, Some(suffix)),
            None => (text, None),
        };
        let mut parts = numbers.split('.');
        let mut number = || -> Option<u32> {
            let digits = parts.next()?;
            let fine = !digits.is_empty()
                && digits.len() <= MAX_DIGITS
                && digits.bytes().all(|b| b.is_ascii_digit())
                && (digits == "0" || !digits.starts_with('0'));
            if fine { digits.parse().ok() } else { None }
        };
        let (major, minor, patch) = (number()?, number()?, number()?);
        if parts.next().is_some() {
            return None;
        }
        let suffix = match suffix {
            None => String::new(),
            Some(s) if !s.is_empty() && s.len() <= MAX_SUFFIX && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-') => {
                s.to_string()
            }
            Some(_) => return None,
        };
        Some(Version { major, minor, patch, suffix })
    }

    /// Whether this is a pre-release (it has a suffix).
    pub fn is_pre_release(&self) -> bool {
        !self.suffix.is_empty()
    }

    /// Whether this version came after `other`: a higher number, or the release of what `other`
    /// is a pre-release of.
    pub fn is_newer_than(&self, other: &Version) -> bool {
        self.cmp_to(other) == Ordering::Greater
    }

    fn cmp_to(&self, other: &Version) -> Ordering {
        (self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch)).then_with(|| {
            match (self.suffix.is_empty(), other.suffix.is_empty()) {
                (true, true) => Ordering::Equal,
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                (false, false) => self.suffix.cmp(&other.suffix),
            }
        })
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if !self.suffix.is_empty() {
            write!(f, "-{}", self.suffix)?;
        }
        Ok(())
    }
}

/// The newest release, as far as the window needs to know of it. Nothing of the release's own
/// text is in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: Version,
    /// The address of its page on the project's site, made from the tag or checked to be that.
    pub page: String,
    /// Its notes name a published security advisory (`GHSA-…` or `RUSTSEC-…`).
    pub security: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    #[test]
    fn versions_are_read_strictly() {
        assert_eq!(v("0.4.2"), Version { major: 0, minor: 4, patch: 2, suffix: String::new() });
        assert_eq!(v("v10.0.300"), Version { major: 10, minor: 0, patch: 300, suffix: String::new() });
        assert_eq!(v("1.2.3-beta.1").suffix, "beta.1");
        assert!(v("1.2.3-rc-2").is_pre_release() && !v("1.2.3").is_pre_release());
        assert_eq!(v("v1.2.3-rc.1").to_string(), "1.2.3-rc.1");
        for bad in [
            "",
            "v",
            "1",
            "1.2",
            "1.2.3.4",
            "V1.2.3",
            "vv1.2.3",
            " 1.2.3",
            "1.2.3 ",
            "1.2.3+build",
            "1.2.3-",
            "1.2.3-é",
            "1.2.3-a b",
            "01.2.3",
            "1.02.3",
            "-1.2.3",
            "1.-2.3",
            "1..3",
            "1.2.x",
            "1234567.0.0",
            "１.2.3",
            "1.2.3\n",
            "latest",
            "<script>",
        ] {
            assert_eq!(Version::parse(bad), None, "{bad:?}");
        }
        assert_eq!(Version::parse(&format!("1.2.3-{}", "a".repeat(MAX_SUFFIX + 1))), None);
        assert_eq!(v("999999.999999.999999").major, 999_999);
    }

    #[test]
    fn which_is_newer() {
        for (newer, older) in [
            ("0.4.3", "0.4.2"),
            ("0.5.0", "0.4.9"),
            ("1.0.0", "0.99.99"),
            ("0.4.3", "0.4.3-beta.1"),
            ("0.4.3-rc.1", "0.4.3-beta.1"),
            ("0.10.0", "0.9.0"),
        ] {
            assert!(v(newer).is_newer_than(&v(older)), "{newer} > {older}");
            assert!(!v(older).is_newer_than(&v(newer)), "{older} < {newer}");
        }
        assert!(!v("0.4.2").is_newer_than(&v("v0.4.2")));
    }
}
