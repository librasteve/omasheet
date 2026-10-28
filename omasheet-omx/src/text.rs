// Copyright (c) 2026 Stephen Roe

//! The kinds of text that read as something narrower: a web address, an
//! email address, a UUID.
//!
//! Each of these is text in a dress, as a `Percent` is an exact number in
//! one: the value held is the text that was written, and the type says what
//! that text was read as. What counts as one of them is settled here and
//! nowhere else, so that a cell written into a declared column and the same
//! text put through `Email(x)` can never disagree.
//!
//! The reading is deliberately plain. `is_email` is not RFC 5322 and does
//! not try to be: a full reading of that grammar admits addresses no one
//! sends mail to and takes a page of code to get wrong, and a wrong refusal
//! here stops a file being written out at all. These turn away the text that
//! is unarguably not an address, not the text that is merely unusual.

/// Whether `s` reads as a UUID: thirty-six characters, hex digits in runs of
/// 8, 4, 4, 4 and 12 with a hyphen between the runs, in either case. The
/// braced and `urn:uuid:` spellings are not read as one.
pub fn is_uuid(s: &str) -> bool {
    if s.len() != 36 {
        return false;
    }
    s.bytes().enumerate().all(|(i, b)| match i {
        8 | 13 | 18 | 23 => b == b'-',
        _ => b.is_ascii_hexdigit(),
    })
}

/// Whether `s` reads as an email address: one `@`, a name before it, and a
/// domain after it of two or more parts with a dot between them. No spaces
/// and no control characters anywhere. Anything else about the name is left
/// alone, so `a.b+tag@example.co.uk` reads as one.
pub fn is_email(s: &str) -> bool {
    if s.chars().any(is_blank) {
        return false;
    }
    let Some((name, domain)) = s.split_once('@') else {
        return false;
    };
    if name.is_empty() || domain.contains('@') {
        return false;
    }
    // A dot, and no empty part, so a domain with no dot, a dot at either
    // end, or two dots together is not read as one.
    domain.contains('.') && domain.split('.').all(|part| !part.is_empty())
}

/// Whether `s` reads as a web address: a scheme, a `:`, and then the rest of
/// the address. No spaces and no control characters anywhere. A scheme is
/// required, so `example.com` on its own does not read as one, and where the
/// rest opens with `//` something has to follow it. Beyond that, what comes
/// after the `:` is not read into.
pub fn is_url(s: &str) -> bool {
    if s.chars().any(is_blank) {
        return false;
    }
    let Some((scheme, rest)) = s.split_once(':') else {
        return false;
    };
    let mut letters = scheme.chars();
    let Some(first) = letters.next() else {
        return false;
    };
    let names_something = match rest.strip_prefix("//") {
        Some(after) => !after.is_empty(),
        None => !rest.is_empty(),
    };
    first.is_ascii_alphabetic()
        && letters.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))
        && names_something
}

/// A space, a tab, a newline, or anything else that is not text to read.
fn is_blank(c: char) -> bool {
    c.is_whitespace() || c.is_control()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_uuid_is_hex_in_five_runs() {
        assert!(is_uuid("7b4e2c10-5f8a-4c3e-9d21-0a1b2c3d4e5f"));
        // The case of the digits is not read into.
        assert!(is_uuid("7B4E2C10-5F8A-4C3E-9D21-0A1B2C3D4E5F"));
        assert!(is_uuid("00000000-0000-0000-0000-000000000000"));
    }

    #[test]
    fn a_uuid_written_another_way_is_not_read_as_one() {
        // No hyphens, the braced form, and the urn form.
        assert!(!is_uuid("7b4e2c105f8a4c3e9d210a1b2c3d4e5f"));
        assert!(!is_uuid("{7b4e2c10-5f8a-4c3e-9d21-0a1b2c3d4e5f}"));
        assert!(!is_uuid("urn:uuid:7b4e2c10-5f8a-4c3e-9d21-0a1b2c3d4e5f"));
        // The right shape, but `g` is not a hex digit, and a run too short.
        assert!(!is_uuid("7b4e2c10-5f8a-4c3e-9d21-0a1b2c3d4e5g"));
        assert!(!is_uuid("7b4e2c1-5f8a-4c3e-9d21-0a1b2c3d4e5f"));
        assert!(!is_uuid(""));
    }

    #[test]
    fn an_email_address_is_a_name_and_a_domain() {
        assert!(is_email("name@example.com"));
        assert!(is_email("a.b+tag@example.co.uk"));
        // Unusual, and left alone: the name is not read into.
        assert!(is_email("!#$%@example.com"));
    }

    #[test]
    fn text_that_is_not_an_address_is_not_read_as_one() {
        assert!(!is_email("name"));
        assert!(!is_email("@example.com"));
        assert!(!is_email("name@"));
        assert!(!is_email("name@example"));
        assert!(!is_email("name@.com"));
        assert!(!is_email("name@example."));
        assert!(!is_email("name@example..com"));
        assert!(!is_email("one@example.com two@example.com"));
        assert!(!is_email("name@two@example.com"));
        assert!(!is_email(""));
    }

    #[test]
    fn a_web_address_has_a_scheme() {
        assert!(is_url("https://example.com"));
        assert!(is_url("http://example.com/a/b?c=d#e"));
        assert!(is_url("mailto:name@example.com"));
        // A scheme may carry digits and a few marks after its first letter.
        assert!(is_url("view-source:https://example.com"));
        assert!(is_url("s3://bucket/key"));
    }

    #[test]
    fn text_without_a_scheme_is_not_read_as_a_web_address() {
        assert!(!is_url("example.com"));
        assert!(!is_url("www.example.com"));
        assert!(!is_url("//example.com"));
        assert!(!is_url("https://"));
        assert!(!is_url("1http://example.com"));
        assert!(!is_url("https://exa mple.com"));
        assert!(!is_url(""));
    }
}
