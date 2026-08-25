//! Role-based cursor palette.
//! Pointer names carry meaning across algorithms — `i` opens a range, `slow`/`fast` race —
//! so they get STABLE role colours, and a reader learns the vocabulary once. The WIRE carries
//! these canonical hexes (parity-pinned); the client maps them to theme-aware CSS tokens.

use std::collections::HashMap;

// Deep blue — entry point / opens a range / primary loop
const DEEP_BLUE: &str = "#3a5a8c";
// Indigo — active position / cursor of activity (the brand accent)
const INDIGO: &str = "#4f5bd5";
// Mulberry — trailing pointer / inner loop / paired query
const MULBERRY: &str = "#8a4f7d";
// Moss green — saved-aside / one step forward
const MOSS: &str = "#5a8a5a";
// Bordeaux — explicit end / contentious second slot
const BORDEAUX: &str = "#a13e3e";

/// Pointer name → role colour. Names in the same band share a colour by conceptual role.
pub fn canon(name: &str) -> Option<&'static str> {
    match name {
        "head" | "root" | "i" | "left" | "low" | "slow" | "front" | "read" | "base" => Some(DEEP_BLUE),
        "current" | "mid" | "top" | "write" | "found" | "ptr" | "end" => Some(INDIGO),
        "j" | "previous" | "p" | "start" | "q" => Some(MULBERRY),
        "next" | "successor" | "predecessor" | "kth" => Some(MOSS),
        "tail" | "dummy" | "parent" | "last" | "fast" | "right" | "high" | "swap" | "back" => Some(BORDEAUX),
        _ => None,
    }
}

/// Real-code variable name → canonical name (so `cur`/`curr`/`node` all read as `current`).
pub fn alias(name: &str) -> Option<&'static str> {
    match name {
        "cur" | "curr" | "node" | "cnode" | "runner" | "walk" => Some("current"),
        "prev" | "pre" => Some("previous"),
        "nxt" | "tmp" | "temp" => Some("next"),
        "lo" => Some("low"),
        "hi" => Some("high"),
        "l" => Some("left"),
        "r" => Some("right"),
        "succ" => Some("successor"),
        "pred" => Some("predecessor"),
        "tree" => Some("root"),
        "h" | "first" => Some("head"),
        _ => None,
    }
}

/// Fallback hues (in order) for names with no known role — assigned by first appearance.
pub const FALLBACK: [&str; 7] = [
    "#3a5a8c", "#4f5bd5", "#a13e3e", "#8a4f7d", "#5a8a5a", "#c5a572", "#91b5c2",
];

/// Step-event colours (diff cues + graph roles) — not pointer names.
pub fn marker(name: &str) -> Option<&'static str> {
    match name {
        "changed" | "visited" => Some("#6a9656"),
        "removed" | "exception" => Some("#a13e3e"),
        "returned" | "frontier" => Some("#4f5bd5"),
        "returnedDarker" => Some("#4a6a3c"),
        "returnedLighter" => Some("#9bbf86"),
        "ref" => Some("#3a5a8c"),
        _ => None,
    }
}

/// The role colour for a pointer name, resolving aliases; `None` for unknown names.
pub fn role_color(name: &str) -> Option<&'static str> {
    canon(name).or_else(|| alias(name).and_then(canon))
}

/// Assign a stable colour to each DISTINCT name: its role colour if known, else the next
/// fallback hue by first appearance. Stable for the whole trace, so a pointer keeps its
/// colour across every step.
pub fn assign_colors(names: &[String]) -> HashMap<String, String> {
    let mut assigned = HashMap::new();
    let mut unknown = 0usize;
    for name in names {
        if assigned.contains_key(name) {
            continue;
        }
        let color = role_color(name).map_or_else(
            || {
                let c = FALLBACK[unknown % FALLBACK.len()];
                unknown += 1;
                c
            },
            |c| c,
        );
        assigned.insert(name.clone(), color.to_owned());
    }
    assigned
}

#[cfg(test)]
mod tests;
