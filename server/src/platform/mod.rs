//! The `platform` bounded context — cross-cutting concerns: health, media, proxies, rate
//! limiting, security headers. Thin and flat: no `domain/` (results are shared DTOs) and no
//! ports beyond `ReadinessProbe` — the full hexagonal layering lives in `catalog`.

pub mod admin_gate;
pub mod astro_proxy;
pub(crate) mod blocking;
pub mod client_ip;
pub mod content_cache_control;
pub(crate) mod frontmatter;
pub mod health;
pub mod http;
pub mod likec4_proxy;
pub mod limits;
pub mod media_routes;
pub mod rate_limiter;
pub mod readiness;
pub mod security_headers;
pub mod seo_routes;
pub mod telemetry;
