use todex::extractor::Extractor;

fn matched(tokens: &[&str], ignore_default: bool, line: &str) -> Option<String> {
    let tokens = tokens.iter().map(|t| (*t).to_string()).collect();
    Extractor::prepare_token_pattern(tokens, ignore_default)
        .expect("prepare_token_pattern")
        .find(line)
        .expect("token pattern find returned an error")
        .map(|m| m.as_str().to_string())
}

#[test]
fn empty_with_ignore_default_raises() {
    let err = Extractor::prepare_token_pattern(Vec::new(), true).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert!(err.to_string().contains("no tokens"));
}

#[test]
fn prepare_token_pattern_cases() {
    let cases: &[(&[&str], bool, &str, Option<&str>)] = &[
        (&[], false, "# TODO: x", Some("TODO")),
        (&[], false, "# FIXME: x", Some("FIXME")),
        (&["WARN"], false, "# WARN: x", Some("WARN")),
        (&["WARN"], false, "# TODO: x", Some("TODO")),
        (&["WARN"], true, "# WARN: x", Some("WARN")),
        (&["WARN"], true, "# TODO: x", None),
        (&["WARN"], true, "# FIXME: x", None),
        (&["WARN", "REVISIT"], true, "WARN here", Some("WARN")),
        (&["WARN", "REVISIT"], true, "REVISIT later", Some("REVISIT")),
        (&["TODO", "TODO", "FIXME"], false, "TODO", Some("TODO")),
        (&["TODO", "TODO", "FIXME"], false, "FIXME", Some("FIXME")),
    ];
    for (tokens, ignore_default, line, expected) in cases {
        assert_eq!(
            matched(tokens, *ignore_default, line).as_deref(),
            *expected,
            "tokens={tokens:?} ignore_default={ignore_default} line={line:?}"
        );
    }
}

#[test]
fn word_boundaries_and_case() {
    let cases = [
        ("todo: lower", Some("todo")),
        ("Todo: mixed", Some("Todo")),
        ("fIxMe: weird", Some("fIxMe")),
        ("TODO", Some("TODO")),
        ("#TODO: x", Some("TODO")),
        ("//FIXME x", Some("FIXME")),
        ("(TODO)", Some("TODO")),
        ("[FIXME]", Some("FIXME")),
        ("please TODO this soon", Some("TODO")),
        ("TODOS remaining", None),
        ("MYTODO done", None),
        ("XXTODOYY", None),
    ];
    for (line, expected) in cases {
        assert_eq!(
            matched(&[], false, line).as_deref(),
            expected,
            "line={line:?}"
        );
    }
}

#[test]
fn prefers_longer_token() {
    assert_eq!(
        matched(&["FIX", "FIXME"], true, "FIXME: later").as_deref(),
        Some("FIXME")
    );
    assert_eq!(
        matched(&["FIX", "FIXME"], true, "FIX: now").as_deref(),
        Some("FIX")
    );
}

#[test]
fn overlap_leftmost_longest() {
    assert_eq!(
        matched(&["AB", "ABC", "BC"], true, " ABC ").as_deref(),
        Some("ABC")
    );
}

#[test]
fn non_ascii_token_boundary() {
    assert_eq!(
        matched(&["中文标记"], true, "中文标记: alone").as_deref(),
        Some("中文标记")
    );
    assert_eq!(
        matched(&["中文标记"], true, "前中文标记后").as_deref(),
        None
    );
}

#[test]
fn special_characters_are_literal() {
    let cases: &[(&[&str], &str, Option<&str>)] = &[
        (&["C++"], "use C++ here", Some("C++")),
        (&["file.txt"], "see file.txt now", Some("file.txt")),
        (&["file.txt"], "fileXtxt", None),
        (&["a.b*"], "a.b* value", Some("a.b*")),
        (&["A+"], "A+", Some("A+")),
        (&["A+"], "AAA", None),
        (&["@FIXME"], "# @FIXME: x", Some("@FIXME")),
        (&["@FIXME"], "# FIXME: bare", None),
        (
            &["Do we need this!"],
            "# Do we need this!",
            Some("Do we need this!"),
        ),
        (&["!!!URGENT!!!"], "# !!!URGENT!!!", Some("!!!URGENT!!!")),
    ];
    for (tokens, line, expected) in cases {
        assert_eq!(
            matched(tokens, true, line).as_deref(),
            *expected,
            "tokens={tokens:?} line={line:?}"
        );
    }
}
