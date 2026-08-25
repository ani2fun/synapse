//! Pure identity domain: the verified-caller shape.

/// The verified caller's opaque subject (`sub`) — never mixed with other strings.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UserId(pub String);

/// A caller's name, canonical: trimmed and lowercased, with no other spelling representable.
///
/// The type earns its keep because THREE independent surfaces compare on this string — the
/// submit allowlist, the content-editor allowlist, and `ADMIN_USERS` — and each of them answers
/// "no" rather than failing when a spelling drifts. That makes a missed lowercase a SILENT
/// authorisation bug: the grant exists, the row matches nothing, and the person is simply
/// refused with no trace of why.
///
/// [`Username::parse`] is the only door, so canonicalisation happens once at the verifier
/// instead of once per comparison. Deliberately NOT `Borrow<str>`: that would make
/// `admin_users.contains("Ada")` compile and quietly answer `false`, which is the exact hole
/// this type closes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Username(String);

impl Username {
    /// `None` when nothing usable survives trimming. A blank name is not a caller: stored as a
    /// grant it would key a row nobody can match, and compared as one it would authorise the
    /// absence of a claim.
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        let canonical = raw.trim().to_lowercase();
        (!canonical.is_empty()).then_some(Self(canonical))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// For the wire, where the name is a plain string again.
    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl std::fmt::Display for Username {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A verified caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedUser {
    pub id: UserId,
    pub username: Username,
    pub email: Option<String>,
}

#[cfg(test)]
mod tests;
