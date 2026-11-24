use super::*;

fn uuid(seed: u8) -> Uuid {
    Uuid::from_random([seed; 16])
}

/// Notes 1 et 2 connues, toute autre inconnue.
fn known(id: i64) -> Option<Uuid> {
    match id {
        1 => Some(uuid(1)),
        2 => Some(uuid(2)),
        _ => None,
    }
}

fn rewrite(content: &str) -> String {
    rewrite_note_links(content, known).unwrap_or_else(|| content.to_owned())
}

#[test]
fn rewrites_every_link_to_a_known_note() {
    let content = "voir [A](note://1) et note://2, puis [B](note://1 \"titre\")";
    let expected = format!(
        "voir [A](note://{}) et note://{}, puis [B](note://{} \"titre\")",
        uuid(1),
        uuid(2),
        uuid(1)
    );
    assert_eq!(rewrite(content), expected);
}

#[test]
fn is_idempotent() {
    let once = rewrite("[A](note://1)\n```\nnote://2\n```\n[X](note://99)");
    assert_eq!(rewrite_note_links(&once, known), None);
}

#[test]
fn keeps_links_to_unknown_notes_and_already_rewritten_ones() {
    assert_eq!(rewrite_note_links("[X](note://99) note://", known), None);
    let already = format!("[A](note://{})", uuid(1));
    assert_eq!(rewrite_note_links(&already, known), None);
}

#[test]
fn does_not_take_a_uuid_made_of_digits_for_an_id() {
    let digits = Uuid::parse("12345678-1234-4234-8234-123456789012").unwrap();
    let content = format!("note://{digits}");
    assert_eq!(rewrite_note_links(&content, |_| Some(uuid(9))), None);
}

#[test]
fn leaves_fenced_code_blocks_alone() {
    let content =
        "note://1\n```md\n[A](note://1)\n```\nnote://2\n~~~~\nnote://2\n~~~\n~~~~\nnote://1";
    let expected = format!(
        "note://{}\n```md\n[A](note://1)\n```\nnote://{}\n~~~~\nnote://2\n~~~\n~~~~\nnote://{}",
        uuid(1),
        uuid(2),
        uuid(1)
    );
    assert_eq!(rewrite(content), expected);
}

#[test]
fn an_unclosed_fence_runs_to_the_end() {
    assert_eq!(rewrite_note_links("```\nnote://1\nnote://2", known), None);
}

#[test]
fn leaves_inline_code_alone() {
    let content = "`note://1` note://1 ``a ` note://2`` `unclosed note://2";
    let expected = format!(
        "`note://1` note://{} ``a ` note://2`` `unclosed note://{}",
        uuid(1),
        uuid(2)
    );
    assert_eq!(rewrite(content), expected);
}

#[test]
fn an_indented_fence_of_four_spaces_is_not_a_fence() {
    let content = "    ```\nnote://1";
    assert_eq!(rewrite(content), format!("    ```\nnote://{}", uuid(1)));
}

#[test]
fn keeps_line_endings_and_non_ascii_text() {
    let content = "é note://1\r\n😀 note://2";
    let expected = format!("é note://{}\r\n😀 note://{}", uuid(1), uuid(2));
    assert_eq!(rewrite(content), expected);
}

#[test]
fn retargets_uuid_links_to_copied_notes() {
    let content = format!("[A](note://{}) [B](note://{}) note://1", uuid(1), uuid(2));
    let moved = |old: Uuid| (old == uuid(1)).then_some(uuid(5));
    let expected = format!("[A](note://{}) [B](note://{}) note://1", uuid(5), uuid(2));

    assert_eq!(retarget_note_links(&content, moved), Some(expected));
    assert_eq!(retarget_note_links("note://1", moved), None);
}
