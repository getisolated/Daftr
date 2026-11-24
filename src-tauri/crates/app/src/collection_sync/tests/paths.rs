use super::*;

fn refusal(text: &str) -> PathRefusal {
    SyncPath::parse(text, &["build".to_owned()]).unwrap_err()
}

#[test]
fn accepts_markdown_files_in_nested_folders() {
    for text in [
        "a.md",
        "docs/a.md",
        "docs/deep/Guide.MD",
        "é à/ü.md",
        "dir.v2/a.md",
    ] {
        assert_eq!(SyncPath::parse(text, &[]).unwrap().as_str(), text);
    }
}

#[test]
fn refuses_whatever_leaves_the_collection() {
    assert_eq!(refusal(""), PathRefusal::Empty);
    assert_eq!(refusal("a//b.md"), PathRefusal::Empty);
    assert_eq!(refusal("../a.md"), PathRefusal::Traversal);
    assert_eq!(refusal("docs/../a.md"), PathRefusal::Traversal);
    assert_eq!(refusal("./a.md"), PathRefusal::Traversal);
    assert_eq!(refusal("/etc/a.md"), PathRefusal::Absolute);
    assert_eq!(refusal("C:/a.md"), PathRefusal::ForbiddenCharacter);
    assert_eq!(refusal("C:\\a.md"), PathRefusal::Absolute);
    assert_eq!(refusal("docs\\a.md"), PathRefusal::Absolute);
    assert_eq!(refusal("a\0.md"), PathRefusal::ForbiddenCharacter);
}

#[test]
fn refuses_names_other_systems_cannot_carry() {
    for text in [
        "con.md",
        "CON.md",
        "docs/Nul.md",
        "aux/a.md",
        "com1.md",
        "LPT9.md",
    ] {
        assert_eq!(refusal(text), PathRefusal::ReservedName, "{text}");
    }
    assert!(SyncPath::parse("com10.md", &[]).is_ok());
    assert!(SyncPath::parse("console.md", &[]).is_ok());
    assert_eq!(refusal("a.md:stream"), PathRefusal::ForbiddenCharacter);
    assert_eq!(refusal("what?.md"), PathRefusal::ForbiddenCharacter);
    assert_eq!(refusal("dir./a.md"), PathRefusal::TrailingDotOrSpace);
    assert_eq!(refusal("dir /a.md"), PathRefusal::TrailingDotOrSpace);
}

#[test]
fn keeps_markdown_outside_excluded_folders_only() {
    assert_eq!(refusal("image.png"), PathRefusal::NotMarkdown);
    assert_eq!(refusal("notes.txt"), PathRefusal::NotMarkdown);
    assert_eq!(refusal("build/a.md"), PathRefusal::Excluded);
    assert_eq!(refusal("BUILD/a.md"), PathRefusal::Excluded);
    assert_eq!(refusal(".git/a.md"), PathRefusal::Excluded);
    assert_eq!(refusal(".hidden/deep/a.md"), PathRefusal::Excluded);
    assert!(SyncPath::parse("builds/a.md", &["build".to_owned()]).is_ok());
}

#[test]
fn bounds_the_length_and_the_depth() {
    assert_eq!(
        refusal(&format!("{}.md", "x".repeat(300))),
        PathRefusal::TooLong
    );
    assert_eq!(
        refusal(&format!("{}/a.md", "d/".repeat(600))),
        PathRefusal::TooLong
    );
    let deep = format!(
        "{}a.md",
        "d/".repeat(domain::collection::MAX_SCAN_DEPTH + 1)
    );
    assert_eq!(refusal(&deep), PathRefusal::Excluded);
}

#[test]
fn case_variants_share_a_fold_key() {
    assert_eq!(path("Docs/A.md").fold_key(), path("docs/a.MD").fold_key());
    assert_ne!(path("docs/a.md"), path("docs/A.md"));
}

#[test]
fn hashes_the_bytes_and_prints_them_in_hex() {
    assert_eq!(hash("a"), hash("a"));
    assert_ne!(hash("a"), hash("a\r"));
    assert_eq!(hash("").to_string().len(), 64);
    assert_eq!(
        hash("").to_string(),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}
