//! The `platform` bounded context — cross-cutting concerns (health today; static routes, media,
//! proxies, rate limiting, security headers as their steps land). Thin and flat per ADR-S007:
//! no `domain/` (results are shared DTOs) and no ports yet — the full hexagonal layering debuts
//! in `catalog`.

pub mod health;
pub mod http;
