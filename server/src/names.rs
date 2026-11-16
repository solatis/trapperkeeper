//! The rules for names people create: tenant names and user names. A name
//! that starts with `$` belongs to the server (ADR-0028).

/// Why a value of a create request was refused, in words for the response
/// body.
#[derive(Debug)]
pub struct Refused(pub &'static str);

/// `name` lowercased, when it is 1 to 64 characters of a-z, 0-9, `.`, `_`
/// and `-`.
///
/// # Errors
///
/// `Refused` when the lowercased name breaks the rule.
pub fn tenant_name(name: &str) -> Result<String, Refused> {
    let name = name.to_lowercase();
    // `len` counts bytes, which are characters in this ASCII set. 64 is a
    // choice that no measurement backs.
    let valid = (1..=64).contains(&name.len())
        && name
            .chars()
            // These can be typed anywhere and fit in a URL path as they are,
            // and `$` is outside them.
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || ".-_".contains(c));
    if valid {
        Ok(name)
    } else {
        Err(Refused(
            "the tenant name must be 1 to 64 characters of a-z, 0-9, '.', '_' and '-'",
        ))
    }
}

/// `name` lowercased, when it is an email address: 3 to 254 characters, one
/// `@` with text on both sides, no whitespace, and no leading `$`.
///
/// # Errors
///
/// `Refused` when the lowercased name breaks the rule.
pub fn user_name(name: &str) -> Result<String, Refused> {
    let name = name.to_lowercase();
    // The function checks only the shape, because the server sends no mail.
    // SMTP carries at most 254 octets (RFC 5321, 4.5.3.1.3). The rule counts
    // 254 characters, which fit MariaDB's VARCHAR(255) and the TEXT of the
    // other backends. 3 is `a@b`, the shortest that the `@` check accepts.
    let shaped = (3..=254).contains(&name.chars().count())
        && !name.chars().any(char::is_whitespace)
        && name.split_once('@').is_some_and(|(local, domain)| {
            !local.is_empty() && !domain.is_empty() && !domain.contains('@')
        });
    if !shaped {
        return Err(Refused(
            "the user name must be an email address of 3 to 254 characters",
        ));
    }
    // An address may contain `$`, so the reserved prefix is refused on its
    // own.
    if name.starts_with('$') {
        return Err(Refused("names that start with '$' belong to the server"));
    }
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::user_name;

    #[test]
    fn a_user_name_is_bounded_in_characters() {
        // Each `\u{e9}` is one character of two bytes, so the first name has
        // 254 characters in 504 bytes.
        let name = |letters: usize| format!("{}@b.c", "\u{e9}".repeat(letters));
        assert!(user_name(&name(250)).is_ok());
        assert!(user_name(&name(251)).is_err());
    }
}
