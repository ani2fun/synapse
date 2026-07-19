//! The `platform` bounded context — cross-cutting concerns (health today; static routes, media,
//! proxies, rate limiting, security headers as their steps land). Thin and flat per ADR-S007:
//! no `domain/` (results are shared DTOs) and no ports yet — the full hexagonal layering debuts
//! in `catalog`.

pub mod client_ip;
pub mod content_cache_control;
pub mod health;
pub mod http;
pub mod likec4_proxy;
pub mod limits;
pub mod media_routes;
pub mod rate_limiter;
pub mod readiness;
pub mod security_headers;
pub mod static_routes;
pub mod telemetry;
