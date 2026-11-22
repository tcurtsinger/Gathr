use super::*;

fn runs(program: &str, args: &[&str], variables: &[(&str, &str)]) -> Runs {
    Runs {
        program: program.into(),
        args: args.iter().map(|a| a.to_string()).collect(),
        folder: None,
        variables: variables
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    }
}

/// A wrong read runs the wrong program: each shape tools' instructions use reads as the same tool.
#[test]
fn what_is_pasted_reads_as_the_program_it_names() {
    let notes = runs(
        "npx",
        &["-y", "@example/notes-mcp", r"D:\Notes"],
        &[("NOTES_KEY", "made-up-1")],
    );
    let entry = r#"{ "command": "npx", "args": ["-y", "@example/notes-mcp", "D:\\Notes"],
        "env": { "NOTES_KEY": "made-up-1" } }"#;
    for (text, name) in [
        (
            format!(r#"{{ "mcpServers": {{ "notes": {entry} }} }}"#),
            "notes",
        ),
        (
            format!(r#"{{ "servers": {{ "notes": {entry} }} }}"#),
            "notes",
        ),
        (format!(r#"{{ "notes": {entry} }}"#), "notes"),
        (entry.to_string(), "npx"),
    ] {
        let read = read(&text).unwrap();
        assert_eq!((read.name.as_str(), &read.runs), (name, &notes), "{text}");
    }

    let line = read(r#"node "C:\My Tools\server.js" --flag 'two words' D:\Notes"#).unwrap();
    let args = [r"C:\My Tools\server.js", "--flag", "two words", r"D:\Notes"];
    assert_eq!(
        (line.name.as_str(), line.runs),
        ("node", runs("node", &args, &[]))
    );
    let cwd = read(r#"{ "command": "C:\\Tools\\files.exe", "cwd": "D:\\Notes" }"#).unwrap();
    assert_eq!(
        (cwd.name.as_str(), cwd.runs.folder.as_deref()),
        ("files", Some(r"D:\Notes"))
    );

    let several = format!(r#"{{ "mcpServers": {{ "a": {entry}, "b": {entry}, "c": {entry} }} }}"#);
    let refused = read(&several).unwrap_err().to_string();
    assert_eq!(refused, "This has 3 tools. Paste one at a time.");
    for text in [
        "",
        "{ not json",
        r#"node "unclosed"#,
        r#"{ "notes": { "url": "x" } }"#,
    ] {
        let refused = read(text).unwrap_err().to_string();
        assert_eq!(refused, UNREADABLE, "{text}");
    }

    // A change: a variable left empty keeps its value, one no longer named goes.
    let was = runs("npx", &[], &[("KEEP", "old-1"), ("DROP", "old-2")]);
    let now = runs("npx", &[], &[("KEEP", ""), ("NEW", "new-1")]);
    let kept = kept(now, &was);
    assert_eq!(
        kept.variables,
        runs("x", &[], &[("KEEP", "old-1"), ("NEW", "new-1")]).variables
    );
}
