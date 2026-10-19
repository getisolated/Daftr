//! Liens markdown entre les fichiers d'une collection.

/// Cibles des liens en ligne `[texte](cible)` d'un texte markdown, telles qu'écrites ; les
/// images (`![…](…)`) et les liens par référence sont ignorés.
///
/// Équivaut à `(?<!!)\[[^\]\n]*\]\((<[^>\n]+>|[^)\s]+)(?:\s+"[^"\n]*")?\)`.
///
/// ```
/// let targets = domain::collection::extract_link_targets("[a](./a.md \"t\") ![i](p.png)");
/// assert_eq!(targets, ["./a.md"]);
/// ```
pub fn extract_link_targets(content: &str) -> Vec<String> {
    let chars: Vec<char> = content.chars().collect();
    let mut targets = Vec::new();
    let mut index = 0;

    while index < chars.len() {
        match link_at(&chars, index) {
            Some((target, end)) => {
                targets.push(target);
                index = end;
            }
            None => index += 1,
        }
    }
    targets
}

/// Le lien commençant à `start` (sa cible et l'index qui suit sa `)`), s'il y en a un.
fn link_at(chars: &[char], start: usize) -> Option<(String, usize)> {
    if chars[start] != '[' || (start > 0 && chars[start - 1] == '!') {
        return None;
    }
    let close = start
        + 1
        + chars[start + 1..]
            .iter()
            .position(|c| *c == ']' || *c == '\n')?;
    if chars[close] != ']' || chars.get(close + 1) != Some(&'(') {
        return None;
    }
    let target_start = close + 2;

    angle_target(chars, target_start)
        .and_then(|end| link_end(chars, target_start, end))
        .or_else(|| {
            bare_target(chars, target_start).and_then(|end| link_end(chars, target_start, end))
        })
}

/// Fin (exclue) d'une cible `<…>` sans retour à la ligne.
fn angle_target(chars: &[char], start: usize) -> Option<usize> {
    if chars.get(start) != Some(&'<') {
        return None;
    }
    let length = chars[start + 1..]
        .iter()
        .position(|c| *c == '>' || *c == '\n')?;
    let close = start + 1 + length;

    (length > 0 && chars[close] == '>').then_some(close + 1)
}

/// Fin (exclue) d'une cible sans espace ni `)`.
fn bare_target(chars: &[char], start: usize) -> Option<usize> {
    let length = chars[start..]
        .iter()
        .position(|c| *c == ')' || c.is_whitespace())
        .unwrap_or(chars.len() - start);

    (length > 0).then_some(start + length)
}

/// Cible et fin du lien quand la cible est suivie de `)`, ou d'un titre `"…"` puis `)`.
fn link_end(chars: &[char], start: usize, end: usize) -> Option<(String, usize)> {
    let after = title_end(chars, end).unwrap_or(end);

    (chars.get(after) == Some(&')')).then(|| (chars[start..end].iter().collect(), after + 1))
}

/// Fin (exclue) d'un titre `\s+"[^"\n]*"` commençant à `start`.
fn title_end(chars: &[char], start: usize) -> Option<usize> {
    let spaces = chars[start..]
        .iter()
        .take_while(|c| c.is_whitespace())
        .count();
    let quote = start + spaces;
    if spaces == 0 || chars.get(quote) != Some(&'"') {
        return None;
    }
    let length = chars[quote + 1..]
        .iter()
        .position(|c| *c == '"' || *c == '\n')?;
    let close = quote + 1 + length;

    (chars[close] == '"').then_some(close + 1)
}

/// Chemin (dans la collection) désigné par un lien relatif de `from_file` ; `None` pour les
/// adresses web, les ancres seules et les chemins qui sortent de la collection. Une cible
/// sans extension est une page markdown (`.md` ajouté) ; les autres extensions sont gardées.
///
/// Reflet de `resolveRelativeLink` (`collection.util.ts`) côté renderer.
///
/// ```
/// use domain::collection::resolve_collection_link;
/// let resolved = resolve_collection_link("docs/x/intro.md", "../api.md#top");
/// assert_eq!(resolved.as_deref(), Some("docs/api.md"));
/// ```
pub fn resolve_collection_link(from_file: &str, target: &str) -> Option<String> {
    let cleaned = clean_target(target);
    if cleaned.is_empty() || has_scheme(&cleaned) {
        return None;
    }
    let segments = join_segments(from_file, &cleaned)?;
    let resolved = segments.join("/");
    let last = segments.last().map(String::as_str).unwrap_or_default();

    // Un lien vers un dossier (`../`, `./guide/`) ouvre sa page d'index, comme index.html
    if points_to_folder(&cleaned) {
        return Some(if resolved.is_empty() {
            "index.md".to_owned()
        } else {
            format!("{resolved}/index.md")
        });
    }
    Some(if has_extension(last) {
        resolved
    } else {
        format!("{resolved}.md")
    })
}

fn clean_target(target: &str) -> String {
    let trimmed = target.trim();
    let unwrapped = trimmed.strip_prefix('<').unwrap_or(trimmed);
    let unwrapped = unwrapped.strip_suffix('>').unwrap_or(unwrapped);
    let without_anchor = unwrapped.split('#').next().unwrap_or_default();

    let decoded = percent_decode(without_anchor).unwrap_or_else(|| without_anchor.to_owned());
    // `~` seul est la racine de la collection, comme `~/`
    if decoded == "~" {
        "~/".to_owned()
    } else {
        decoded
    }
}

/// Segments du chemin désigné ; `/…` et `~/…` (convention DocFX) partent de la racine.
fn join_segments(from_file: &str, cleaned: &str) -> Option<Vec<String>> {
    let directory = from_file
        .rsplit_once('/')
        .map(|(directory, _)| directory)
        .unwrap_or_default();
    let from_root = cleaned.strip_prefix("~/");
    let mut segments: Vec<String> = if cleaned.starts_with('/') || from_root.is_some() {
        Vec::new()
    } else {
        directory
            .split('/')
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect()
    };

    for segment in from_root.unwrap_or(cleaned).split('/') {
        match segment {
            ".." => {
                segments.pop()?;
            }
            "." | "" => {}
            name => segments.push(name.to_owned()),
        }
    }
    Some(segments)
}

/// La cible d'un lien désigne un dossier (`../`, `./guide/`, `~/`) plutôt qu'un fichier.
pub fn is_folder_link(target: &str) -> bool {
    let cleaned = clean_target(target);
    !cleaned.is_empty() && !has_scheme(&cleaned) && points_to_folder(&cleaned)
}

/// La cible désigne un dossier : elle finit par `/`, ou par `.` ou `..`.
fn points_to_folder(cleaned: &str) -> bool {
    let last = cleaned.rsplit('/').next().unwrap_or_default();
    cleaned.ends_with('/') || last == "." || last == ".."
}

/// `^[a-z][a-z0-9+.-]*:` sans tenir compte de la casse (`https:`, `mailto:`, `note:`…).
fn has_scheme(target: &str) -> bool {
    let Some((scheme, _)) = target.split_once(':') else {
        return false;
    };
    let mut chars = scheme.chars();
    let valid_rest = |c: char| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-');

    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && chars.all(valid_rest)
}

/// `\.[^./]+$` : le dernier segment se termine par une extension.
fn has_extension(segment: &str) -> bool {
    segment
        .rsplit_once('.')
        .is_some_and(|(_, extension)| !extension.is_empty())
}

/// Décodage `%XX` ; `None` si une séquence est invalide (le texte est alors gardé tel quel).
///
/// ```
/// assert_eq!(domain::collection::percent_decode("a%20b").as_deref(), Some("a b"));
/// ```
pub fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = std::str::from_utf8(bytes.get(index + 1..index + 3)?).ok()?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_inline_links_and_skips_images() {
        let content = "See [a](./a.md \"title\") and ![img](pic.png), [b](<with space.md>)";
        assert_eq!(extract_link_targets(content), ["./a.md", "<with space.md>"]);
    }

    #[test]
    fn ignores_broken_links() {
        assert!(extract_link_targets("[a\nb](x.md) [c]( d.md) [e](f.md \"t").is_empty());
        assert!(extract_link_targets("[ref][1] [empty]()").is_empty());
    }

    #[test]
    fn keeps_brackets_inside_the_label() {
        assert_eq!(extract_link_targets("[a [b](c.md)"), ["c.md"]);
    }

    #[test]
    fn resolves_like_the_renderer() {
        assert_eq!(
            resolve_collection_link("docs/x/intro.md", "../api.md#top").as_deref(),
            Some("docs/api.md")
        );
        assert_eq!(resolve_collection_link("intro.md", "https://x.dev"), None);
        assert_eq!(resolve_collection_link("intro.md", "../out.md"), None);
        assert_eq!(
            resolve_collection_link("docs/a.md", "/root.md").as_deref(),
            Some("root.md")
        );
    }

    #[test]
    fn adds_md_only_to_extension_less_targets() {
        assert_eq!(
            resolve_collection_link("docs/intro.md", "Other%20page").as_deref(),
            Some("docs/Other page.md")
        );
        assert_eq!(
            resolve_collection_link("README.md", "src/WMSAlign.Enum.al").as_deref(),
            Some("src/WMSAlign.Enum.al"),
        );
    }

    #[test]
    fn a_link_to_a_folder_opens_its_index_page() {
        let resolve = |from: &str, target: &str| resolve_collection_link(from, target);
        assert_eq!(
            resolve("docs/guide/a.md", "../").as_deref(),
            Some("docs/index.md")
        );
        assert_eq!(
            resolve("docs/a.md", "./api/").as_deref(),
            Some("docs/api/index.md")
        );
        assert_eq!(resolve("docs/a.md", "..").as_deref(), Some("index.md"));
        assert_eq!(resolve("a.md", "../"), None);
    }

    #[test]
    fn resolves_tilde_from_the_root_of_the_collection() {
        let resolve = |target: &str| resolve_collection_link("doc/v28/a.md", target);
        assert_eq!(
            resolve("~/doc/v27/index.md").as_deref(),
            Some("doc/v27/index.md")
        );
        assert_eq!(resolve("~/doc/guide").as_deref(), Some("doc/guide.md"));
        assert_eq!(resolve("~/").as_deref(), Some("index.md"));
        assert_eq!(resolve("~").as_deref(), Some("index.md"));
    }

    #[test]
    fn keeps_badly_encoded_targets_as_written() {
        assert_eq!(
            resolve_collection_link("a.md", "100%.md").as_deref(),
            Some("100%.md")
        );
        assert_eq!(resolve_collection_link("a.md", "#anchor"), None);
    }
}
