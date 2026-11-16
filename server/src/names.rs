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
    // `len` counts bytes, which equal the characters here, because the set
    // below admits ASCII alone. The cap of 64 is a choice that no measurement
    // backs.
    let valid = (1..=64).contains(&name.len())
        && name
            .chars()
            // Letters, digits and this punctuation can be typed anywhere and
            // fit in a URL path as they are. `$` is outside the set, so the
            // server's own names cannot be created.
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || ".-_".contains(c));
    if valid {
        Ok(name)
    } else {
        Err(Refused(
            "the tenant name must be 1 to 64 characters of a-z, 0-9, '.', '_' and '-'",
        ))
    }
}

/// `name` lowercased, when it is an email address: 3 to 254 bytes as UTF-8,
/// one `@` with text on both sides, no whitespace, and no leading `$`.
///
/// # Errors
///
/// `Refused` when the lowercased name breaks the rule.
pub fn user_name(name: &str) -> Result<String, Refused> {
    let name = name.to_lowercase();
    // The function checks only the shape of the address, because the server
    // sends no mail that could confirm it. 254 is the longest address SMTP
    // carries (RFC 5321, 4.5.3.1.3), and SMTP counts it in octets, as `len`
    // counts bytes. 3 is the length of `a@b`, the shortest text that the `@`
    // check accepts, so the lower bound adds nothing to that check.
    let shaped = (3..=254).contains(&name.len())
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
