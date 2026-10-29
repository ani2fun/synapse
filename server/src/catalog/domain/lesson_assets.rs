//! Lesson-local images (ADR-RS013): what an `_assets/…` image reference in a lesson is, and which
//! files under `_assets/` the server publishes. One rule, read by both the route that serves the
//! files and the lint that checks a lesson's references, so the two cannot disagree.

/// The file types published from `_assets/` outside `_simulators/`.
pub const IMAGE_EXTENSIONS: [&str; 6] = ["svg", "png", "webp", "jpg", "jpeg", "gif"];

/// `file` (relative to an `_assets/` folder) is a published image: an image extension, and no
/// empty, `_`-led or `.`-led segment — which also rules out `..`.
#[must_use]
pub fn is_public_image(file: &str) -> bool {
    let visible = file
        .split('/')
        .all(|segment| !segment.is_empty() && !segment.starts_with(['_', '.']));
    let extension = file
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase());
    visible && extension.is_some_and(|extension| IMAGE_EXTENSIONS.contains(&extension.as_str()))
}

/// Every markdown image in `content` whose src is lesson-local (`_assets/…` or `./_assets/…`),
/// as the part after `_assets/`. Images inside fenced code are examples, not references, so they
/// are skipped.
#[must_use]
pub fn local_image_refs(content: &str) -> Vec<String> {
    let mut refs = Vec::new();
    let mut fence: Option<&str> = None;
    for line in content.lines() {
        let trimmed = line.trim_start();
        let marker = ["```", "~~~"].into_iter().find(|m| trimmed.starts_with(m));
        match (fence, marker) {
            (None, Some(m)) => {
                fence = Some(m);
                continue;
            }
            (Some(open), Some(m)) if open == m => {
                fence = None;
                continue;
            }
            (Some(_), _) => continue,
            (None, None) => {}
        }
        let mut rest = line;
        while let Some(start) = rest.find("![") {
            rest = &rest[start + 2..];
            let Some(open) = rest.find("](") else { break };
            let after = &rest[open + 2..];
            let Some(close) = after.find(')') else { break };
            let target = after[..close].split_whitespace().next().unwrap_or("");
            let target = target.trim_start_matches('<').trim_end_matches('>');
            let target = target.strip_prefix("./").unwrap_or(target);
            if let Some(file) = target.strip_prefix("_assets/") {
                refs.push(file.to_owned());
            }
            rest = &after[close..];
        }
    }
    refs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_image_beside_the_lesson_or_in_a_plain_subfolder_is_public() {
        for file in [
            "shot.png",
            "keycloak/login.PNG",
            "c4.svg",
            "a.webp",
            "b.jpg",
            "c.jpeg",
            "d.gif",
        ] {
            assert!(is_public_image(file), "{file}");
        }
    }

    #[test]
    fn hidden_folders_sources_and_traversal_are_not_public() {
        for file in [
            "_drafts/a.png",
            ".cache/a.png",
            "../a.png",
            "a//b.png",
            ".png",
            "c4.d2",
            "notes.md",
        ] {
            assert!(!is_public_image(file), "{file}");
        }
    }

    #[test]
    fn local_refs_are_read_from_prose_but_not_from_code() {
        let md = "![a](_assets/a.png) and ![b](./_assets/s/b.svg \"title\")\n\
                  ![c](/media/java/c.png) ![d](https://x/_assets/d.png)\n\
                  ```md\n![e](_assets/e.png)\n```\n";
        assert_eq!(local_image_refs(md), ["a.png", "s/b.svg"]);
    }
}
