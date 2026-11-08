use super::*;

const B: Emphasis = Emphasis::BOLD;
const I: Emphasis = Emphasis::ITALIC;
const C: Emphasis = Emphasis::CODE;

/// The styled pieces of a line, as (text, emphasis).
fn styled(line: &Line) -> Vec<(&str, Emphasis)> {
    line.runs.iter().map(|(range, emphasis)| (&line.text[range.clone()], *emphasis)).collect()
}

fn one(text: &str) -> Line {
    let mut lines = parse(text);
    assert_eq!(lines.len(), 1);
    lines.remove(0)
}

#[test]
fn plain_text_is_left_alone() {
    let line = one("Take the Decanter.");
    assert_eq!(line, Line { kind: LineKind::Plain, text: "Take the Decanter.".into(), runs: vec![] });
}

#[test]
fn bold_italics_and_code_are_styled_without_their_markers() {
    let line = one("Take **Decanter**, not *Cheese Wheel*: see `rate_options` and __this__ or _that_.");
    assert_eq!(line.text, "Take Decanter, not Cheese Wheel: see rate_options and this or that.");
    assert_eq!(styled(&line), [("Decanter", B), ("Cheese Wheel", I), ("rate_options", C), ("this", B), ("that", I)]);
}

#[test]
fn emphasis_nests() {
    let line = one("*all **of** it*");
    assert_eq!(line.text, "all of it");
    assert_eq!(styled(&line), [("all ", I), ("of", Emphasis { bold: true, italic: true, code: false }), (" it", I)]);
}

#[test]
fn code_keeps_its_markers_inside() {
    let line = one("`a *b* c` and ``x ` y``");
    assert_eq!(line.text, "a *b* c and x ` y");
    assert_eq!(styled(&line), [("a *b* c", C), ("x ` y", C)]);
}

#[test]
fn a_marker_without_its_partner_stays_as_typed() {
    // As while a reply is still streaming in.
    assert_eq!(one("Take **Deca").text, "Take **Deca");
    assert_eq!(one("see `rate_").text, "see `rate_");
    assert!(one("Take **Deca").runs.is_empty());
    // Spaces inside the markers, and a lone star, are not emphasis.
    assert_eq!(one("2 * 3 * 4").text, "2 * 3 * 4");
    assert!(one("2 * 3 * 4").runs.is_empty());
}

#[test]
fn underscores_inside_words_are_not_italics() {
    let line = one("call rate_options and follow_ups");
    assert_eq!(line.text, "call rate_options and follow_ups");
    assert!(line.runs.is_empty());
}

#[test]
fn an_escaped_marker_is_literal() {
    let line = one(r"a \*star\* here");
    assert_eq!(line.text, "a *star* here");
    assert!(line.runs.is_empty());
}

#[test]
fn bullets_and_numbers_are_list_lines() {
    let lines = parse("Options:\n- **Decanter** fits\n* Cheese Wheel\n  + nested\n1. First\n12) Twelfth\n\nDone");
    let kinds: Vec<_> = lines.iter().map(|line| line.kind.clone()).collect();
    assert_eq!(
        kinds,
        [
            LineKind::Plain,
            LineKind::Bullet { depth: 0 },
            LineKind::Bullet { depth: 0 },
            LineKind::Bullet { depth: 1 },
            LineKind::Numbered { depth: 0, number: "1".into() },
            LineKind::Numbered { depth: 0, number: "12".into() },
            LineKind::Plain,
            LineKind::Plain,
        ]
    );
    assert_eq!(lines[1].text, "Decanter fits");
    assert_eq!(styled(&lines[1]), [("Decanter", B)]);
    assert_eq!(lines[4].text, "First");
    assert_eq!(lines[6].text, "");
}

#[test]
fn anything_else_stays_plain_text() {
    let lines = parse("# Heading\n| a | b |\n> quote\n```\n**not bold** in a fence\n```\n-not a bullet\n1.not a number");
    let texts: Vec<_> = lines.iter().map(|line| line.text.as_str()).collect();
    assert_eq!(texts, ["# Heading", "| a | b |", "> quote", "```", "**not bold** in a fence", "```", "-not a bullet", "1.not a number"]);
    assert!(lines.iter().all(|line| line.kind == LineKind::Plain));
    assert!(lines[4].runs.is_empty());
}

#[test]
fn ranges_are_on_character_boundaries() {
    let line = one("**Lollipop’s** “gift” _é_");
    assert_eq!(line.text, "Lollipop’s “gift” é");
    assert_eq!(styled(&line), [("Lollipop’s", B), ("é", I)]);
}
