//! Pure identity domain: the verified-caller shape.

/// The verified caller's opaque subject (`sub`) — never mixed with other strings.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UserId(pub String);

/// A verified caller. `username` is CANONICAL LOWERCASE, applied once at
/// the verifier, so admin gates and the submit allowlist compare apples to apples.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedUser {
    pub id: UserId,
    pub username: String,
    pub email: Option<String>,
}
