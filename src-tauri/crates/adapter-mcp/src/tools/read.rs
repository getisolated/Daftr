//! Lecture d'une note, entière ou en partie, et lecture groupée de plusieurs notes.

use domain::note::edit::{headings, line_count, lines_between, section_lines};
use serde_json::json;

use super::{
    GetNoteArgs, GetNotesArgs, McpServices, ToolResult, existing_note, failure, with_content,
};

/// Nombre maximal de notes lues par `get_notes`.
const MAX_BATCH: usize = 20;
/// Nombre maximal de titres cités quand une section est introuvable.
const MAX_LISTED_HEADINGS: usize = 30;

/// Un morceau de note : son texte et ses lignes (à partir de 1, bornes comprises).
struct Part {
    text: String,
    from: usize,
    to: usize,
}

/// Une note complète, ou seulement une section ou des lignes pour ne pas en payer le reste.
pub(crate) fn get_note(services: &McpServices, args: GetNoteArgs) -> ToolResult {
    let note = existing_note(services, args.id)?;
    let mut value = with_content(&note);

    if args.section.is_none() && args.from_line.is_none() && args.to_line.is_none() {
        return Ok(value);
    }
    let part = part_of(&note.content, &args)?;
    value["content"] = json!(part.text);
    value["lines"] = json!(format!("{}-{}", part.from, part.to));
    value["total_lines"] = json!(line_count(&note.content));
    Ok(value)
}

/// Plusieurs notes complètes en un appel ; les identifiants inconnus sont listés à part.
pub(crate) fn get_notes(services: &McpServices, args: GetNotesArgs) -> ToolResult {
    if args.ids.is_empty() || args.ids.len() > MAX_BATCH {
        return Err(format!("Give between 1 and {MAX_BATCH} ids."));
    }
    let mut notes = Vec::new();
    let mut missing = Vec::new();

    for id in args.ids {
        match services.notes.by_id(id).map_err(failure)? {
            Some(note) => notes.push(with_content(&note)),
            None => missing.push(id),
        }
    }
    let mut value = json!({ "notes": notes });
    if !missing.is_empty() {
        value["missing"] = json!(missing);
    }
    Ok(value)
}

fn part_of(content: &str, args: &GetNoteArgs) -> Result<Part, String> {
    let (from, to) = match args.section.as_deref() {
        Some(_) if args.from_line.is_some() || args.to_line.is_some() => {
            return Err("Use either section or from_line/to_line, not both.".to_owned());
        }
        Some(title) => {
            section_lines(content, title).ok_or_else(|| section_not_found(title, content))?
        }
        None => line_range(args, line_count(content))?,
    };

    Ok(Part {
        text: lines_between(content, from, to),
        from,
        to,
    })
}

fn line_range(args: &GetNoteArgs, total: usize) -> Result<(usize, usize), String> {
    let from = args.from_line.map_or(1, as_line).max(1);
    let to = args.to_line.map_or(total, as_line).min(total);

    if from > to {
        return Err(format!("No such lines: the note has {total} lines."));
    }
    Ok((from, to))
}

fn as_line(value: u32) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

fn section_not_found(title: &str, content: &str) -> String {
    let found: Vec<String> = headings(content)
        .iter()
        .take(MAX_LISTED_HEADINGS)
        .map(|heading| format!("{} {}", "#".repeat(heading.level), heading.text))
        .collect();

    if found.is_empty() {
        format!("Section '{title}' not found: the note has no headings.")
    } else {
        format!(
            "Section '{title}' not found. Headings: {}",
            found.join(" | ")
        )
    }
}
