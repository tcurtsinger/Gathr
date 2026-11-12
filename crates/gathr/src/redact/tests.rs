use super::{redact, redact_json};
use serde_json::{Value, json};

/// Each input reads as its expected text, and redacting that again changes nothing.
fn reads(cases: &[(&str, &str)]) {
    for (input, expected) in cases {
        let once = redact(input);
        assert_eq!(once, *expected, "input: {input:?}");
        assert_eq!(redact(&once), once, "redacted twice, from: {input:?}");
    }
}

fn untouched(texts: &[&str]) {
    for text in texts {
        assert_eq!(redact(text), *text);
    }
}

/// Real frame shapes (Claude Code and Codex) with a secret where an agent's work can put one.
#[test]
fn secrets_inside_frame_values_are_redacted() {
    let frames = [
        json!({ "type": "user", "message": { "role": "user", "content": "sk-ant-api03-abcdefghijklmnopqrst" } }),
        json!({ "type": "assistant", "message": { "content": [{ "type": "tool_use", "name": "Bash",
            "input": { "command": "curl -H \"Authorization: Bearer abc123secret\" x" } }] } }),
        json!({ "type": "stream_event", "event": { "type": "content_block_delta",
            "delta": { "type": "input_json_delta", "partial_json": "{\"key\":\"sk-proj-0123456789abcdefgh\"}" } } }),
        json!({ "method": "item/started", "params": { "item": { "type": "mcpToolCall",
            "arguments": { "headers": { "Authorization": "Basic dXNlcjpwYXNz" }, "refresh_token": "rt-1" } } } }),
    ];
    for frame in frames {
        let line = frame.to_string();
        let body = redact_json(&frame).to_string();
        for secret in [
            "sk-ant-api03",
            "abc123secret",
            "sk-proj-0123",
            "dXNlcjpwYXNz",
            "rt-1",
        ] {
            assert!(
                !body.contains(secret),
                "{secret} survived in {body} (from {line})"
            );
        }
        assert!(body.contains("[redacted:"), "nothing was marked in {body}");
        serde_json::from_str::<Value>(&body).expect("the redacted frame is still JSON");
    }
}

#[test]
fn frame_values_without_secrets_are_untouched() {
    let frame = json!({ "type": "result", "usage": { "input_tokens": 12, "output_tokens": 3 },
        "result": "Done: wrote note.txt", "session_id": "8947397a-6c50-409f-9253-43c09abad99c" });
    assert_eq!(redact_json(&frame), frame);
}

#[test]
fn each_secret_shape_is_redacted() {
    reads(&[
        (
            "key sk-ant-api03-abcdefghijklmnop end",
            "key [redacted:key] end",
        ),
        (
            "Authorization: Bearer abc.def.ghi",
            "Authorization: Bearer [redacted:bearer]",
        ),
        (
            "got eyJhbGciOi.eyJzdWIiOiIx.c2lnbmF0dXJl\n",
            "got [redacted:jwt]\n",
        ),
        (
            "run ANTHROPIC_API_KEY=xyz claude",
            "run ANTHROPIC_API_KEY=[redacted:env] claude",
        ),
        (
            "claude --api-key 12345 -p",
            "claude --api-key [redacted:flag] -p",
        ),
        (
            "value \"sk-proj-0123456789abcdef\",",
            "value \"[redacted:key]\",",
        ),
    ]);
}

/// A secret flag's value is hidden whole, after a space or an `=`: to the next space outside
/// quotes, whatever punctuation it holds. Its quotes stay, so JSON text stays JSON.
#[test]
fn a_secret_flags_whole_value_is_redacted() {
    reads(&[
        (
            "gh --token=abc123 pr list",
            "gh --token=[redacted:flag] pr list",
        ),
        ("x --PASSWORD=hunter2 y", "x --PASSWORD=[redacted:flag] y"),
        ("x --token=\"abc123\" y", "x --token=\"[redacted:flag]\" y"),
        (
            "x --auth-token=user:s3cret y",
            "x --auth-token=[redacted:flag] y",
        ),
        ("x --token='a b' y", "x --token='[redacted:flag]' y"),
        ("x --token \"a b\" y", "x --token \"[redacted:flag]\" y"),
        ("x --api-key=abc,def y", "x --api-key=[redacted:flag] y"),
        ("x --api-key abc,def:ghi y", "x --api-key [redacted:flag] y"),
        (
            "[\"--token\", \"abc\", \"next\"]",
            "[\"--token\", \"[redacted:flag]\", \"next\"]",
        ),
        (
            "{\"args\":[\"--token\",\"abc\"],\"n\":1}",
            "{\"args\":[\"--token\",\"[redacted:flag]\"],\"n\":1}",
        ),
        (
            "{\"command\":\"deploy --token=abc\",\"timeout\":5}",
            "{\"command\":\"deploy --token=[redacted:flag]\",\"timeout\":5}",
        ),
        // A secret waited for is not looked for past the command's end.
        ("x --token | sort", "x --token | sort"),
        // A value holds what a shell would split at, as it did before: nothing of it is left.
        ("x --token a|b&c<d y", "x --token [redacted:flag] y"),
        (
            "x --token=a|b y; FOO=a&b z",
            "x --token=[redacted:flag] y; FOO=[redacted:env] z",
        ),
        // A quoted value may start like JSON goes on.
        ("x --token \" 5\" y", "x --token \"[redacted:flag]\" y"),
        ("x --token \":abc\" y", "x --token \"[redacted:flag]\" y"),
        ("x --pass=hunter2", "x --pass=[redacted:flag]"),
        ("x --pass hunter2 y", "x --pass [redacted:flag] y"),
        ("x --passwd hunter2", "x --passwd [redacted:flag]"),
        ("x --auth=abc y", "x --auth=[redacted:flag] y"),
        ("x --auth abc y", "x --auth [redacted:flag] y"),
        ("x --credential abc", "x --credential [redacted:flag]"),
        ("x --credentials=abc", "x --credentials=[redacted:flag]"),
    ]);
}

/// The flag's name is read word by word: a secret word inside a longer one is no secret.
#[test]
fn a_flag_that_only_looks_like_a_secret_is_untouched() {
    untouched(&[
        "cargo run --port=47101 --data=test-data --color=always --jobs=4 -k=v",
        "x --keyboard=us --monkey=1 --tokenizer=x --max-tokens=4096",
        "x --keyboard us --monkey 1 --tokenizer x --max-tokens 4096 --author bob",
        "x --passes 3 --pass-through y --pwd /tmp --pwd=/tmp --authors=a",
        // Named like a secret, taking none: the next word is not its value.
        "docker login --password-stdin -u bob",
        "gh auth login --with-token < token.txt",
    ]);
}

/// Hidden though they hold no secret: a file's path, a sort column. Hiding too much is the safe
/// side, and telling these from `--key <secret>` would take knowing every program.
#[test]
fn a_flag_with_a_secret_word_is_hidden_even_when_it_names_a_file_or_a_column() {
    reads(&[
        ("x --key-file=path/id.pem", "x --key-file=[redacted:flag]"),
        ("x --sort-key=name", "x --sort-key=[redacted:flag]"),
        ("sort --key=2,2 file", "sort --key=[redacted:flag] file"),
    ]);
}

/// PowerShell's parameters: one dash, words run together with capitals.
#[test]
fn a_powershell_parameter_named_like_a_secret_hides_its_value() {
    reads(&[
        (
            "Connect-Thing -Token abc -Path x",
            "Connect-Thing -Token [redacted:flag] -Path x",
        ),
        ("x -ApiKey abc", "x -ApiKey [redacted:flag]"),
        (
            "x -Password 'a b' -Name n",
            "x -Password '[redacted:flag]' -Name n",
        ),
        ("x -Secret abc", "x -Secret [redacted:flag]"),
        ("x -AccessToken abc", "x -AccessToken [redacted:flag]"),
        ("x -Token:abc y", "x -Token:[redacted:flag] y"),
    ]);
    untouched(&[
        "Get-Item -Path src -Name lib.rs -KeyboardLayout us -Tokenize x -Authorize now",
        "git commit -m fix -S; tar -xzf a.tgz; ls -la",
    ]);
}

#[test]
fn a_powershell_environment_variable_named_like_a_secret_hides_its_value() {
    reads(&[
        (
            "$env:GH_TOKEN=\"abc\"; gh pr list",
            "$env:GH_TOKEN=\"[redacted:env]\"; gh pr list",
        ),
        (
            "$env:API_KEY = 'x y'; next",
            "$env:API_KEY = '[redacted:env]'; next",
        ),
        ("$Env:DbPassword = abc", "$Env:DbPassword = [redacted:env]"),
        // An upper-case `NAME=value` is hidden whatever its name (0011), quoted or not.
        ("TOKEN=\"a b\" run", "TOKEN=\"[redacted:env]\" run"),
        (
            "$env:GATHR_DATA=\"test-data\"",
            "$env:GATHR_DATA=\"[redacted:env]\"",
        ),
    ]);
    untouched(&["$env:PATH = \"C:\\tools\"; echo $env:TOKEN; $env:Path += ';x'"]);
}

/// A login inside a URL or a connection string: the password goes, the name and host stay.
#[test]
fn a_password_in_a_url_is_hidden() {
    let key = format!("ghp_{}", "0a1B".repeat(9));
    let with_key = redact(&format!("git clone https://{key}@example.test/r.git"));
    assert_eq!(
        with_key,
        "git clone https://[redacted:url]@example.test/r.git"
    );
    reads(&[
        (
            "git clone https://bob:hunter2@example.test/repo.git",
            "git clone https://bob:[redacted:url]@example.test/repo.git",
        ),
        (
            "psql postgres://app:s3@cret@db.example.test:5432/main?x=1",
            "psql postgres://app:[redacted:url]@db.example.test:5432/main?x=1",
        ),
        (
            "\"redis://:s3cret@cache.example.test\"",
            "\"redis://:[redacted:url]@cache.example.test\"",
        ),
    ]);
    untouched(&[
        "https://example.test:8443/a@b ssh://git@example.test/repo http://127.0.0.1:47101/",
        "git@example.test:org/repo.git mailto:someone@example.test C://not/a@url",
    ]);
}

/// A header written out in a command: `Authorization` with any scheme, or a hyphened name with a
/// secret word. A lone word before a colon is prose, and stays.
#[test]
fn a_header_named_like_a_secret_hides_its_value() {
    reads(&[
        (
            "curl -H \"X-Api-Key: abc123\" x",
            "curl -H \"X-Api-Key: [redacted:header]\" x",
        ),
        (
            "curl -H 'Authorization: token abc123' x",
            "curl -H 'Authorization: token [redacted:header]' x",
        ),
        (
            "Authorization: Basic dXNlcjpwYXNz",
            "Authorization: Basic [redacted:header]",
        ),
        ("Authorization: abc123", "Authorization: [redacted:header]"),
        (
            "Proxy-Authorization: Basic abc x",
            "Proxy-Authorization: Basic [redacted:header] x",
        ),
        (
            "-H 'X-Auth-Token: abc'",
            "-H 'X-Auth-Token: [redacted:header]'",
        ),
        ("-H \"apikey: abc\"", "-H \"apikey: [redacted:header]\""),
    ]);
    untouched(&[
        "Content-Type: application/json",
        "Accept-Encoding: gzip, Cache-Control: no-cache",
        "primary key: id, error: token expired, secret: none, auth: off",
        "{\"X-Api-Key\": 3, \"key\": \"value\"}",
    ]);
}

/// Keys known by how they start. Built here, so no made-up key sits in the source for a
/// scanner to take for a real one.
#[test]
fn a_providers_key_is_hidden_by_its_prefix() {
    let tail = "0a1B".repeat(6);
    for prefix in [
        "sk-",
        "sk_live_",
        "sk_test_",
        "ghp_",
        "gho_",
        "ghu_",
        "ghs_",
        "github_pat_",
        "xoxb-",
        "xoxp-",
    ] {
        let key = format!("{prefix}{tail}");
        assert_eq!(
            redact(&format!("use {key} now")),
            "use [redacted:key] now",
            "{prefix}"
        );
        assert_eq!(
            redact(&format!("x?access={key}")),
            "x?access=[redacted:key]",
            "{prefix}"
        );
        // Too short to be one.
        let short = format!("{prefix}abc");
        assert_eq!(redact(&short), short);
    }
    let aws = format!("AKIA{}", "ABCD2345".repeat(2));
    assert_eq!(redact(&format!("id {aws} end")), "id [redacted:key] end");
    let not_aws = [
        format!("AKIA{}", "ABCD2345ABCD234"),
        format!("AKIA{}", "abcd2345".repeat(2)),
        format!("AKIA{}", "ABCD2345".repeat(3)),
    ];
    for word in not_aws {
        assert_eq!(redact(&word), word);
    }
    untouched(&[
        "skip-this-very-long-word-here ghost_writer_of_the_long_name xoxo-hugs-and-kisses-to-all",
    ]);
}

/// A step's text is redacted when it's kept, and again in any frame that carries it: the second
/// pass took "bearer" inside `[redacted:bearer]` for the word before a secret.
#[test]
fn text_already_redacted_stays_as_it_is() {
    let text = "curl --token=abc123 -H \"Authorization: Bearer abc.def\" https://example.test \
                KEY_ID=x --password hunter2 sk-proj-0123456789abcdefgh done";
    let once = redact(text);
    assert_eq!(redact(&once), once);
    // A mark that came in someone's text ends a wait for a secret too.
    untouched(&["--token [redacted:flag] next"]);
    // Something that only starts like a mark doesn't stop the marks after it being seen.
    untouched(&[
        "[redacted: not a mark] --token [redacted:flag] next",
        "[redacted:] and [redacted:TOO] then Bearer [redacted:bearer] next",
    ]);
}

#[test]
fn ordinary_text_is_untouched() {
    untouched(&[
        "start failed: file is not a database (path X:\\data\\gathr.db) a=b --port 47101\n",
        "the key to this is a token of thanks; pass the secret sauce, auth or not",
        "fn key(token: &str) -> Password { secret(token, key = value) }",
        "sorted(items, key=len) and x.password = y",
    ]);
}

/// A name run together (`--accesstoken`) is a secret's as much as `--access-token`: a word that
/// ends in `token`, `secret`, `password` or `passwd`, or in `key` after a word that names one.
#[test]
fn a_secret_word_at_the_end_of_a_run_together_name_counts() {
    for name in [
        "accesstoken",
        "APITOKEN",
        "clientsecret",
        "secretkey",
        "authtoken",
        "bottoken",
        "dbpassword",
        "rootpasswd",
        "accesskey",
        "privatekey",
        "ApiKey",
    ] {
        let spaced = (
            format!("x --{name} abc y"),
            format!("x --{name} [redacted:flag] y"),
        );
        let equals = (
            format!("x --{name}=abc y"),
            format!("x --{name}=[redacted:flag] y"),
        );
        reads(&[(&spaced.0, &spaced.1), (&equals.0, &equals.1)]);
    }
    untouched(&[
        "x --tokenizer a --max-tokens 5 --keyboard us --monkey 1 --hotkey f5 --turkey x",
        "x --secretary bob --passwords-file f --tokens 3 --keyname k --publickey p",
    ]);
}

/// Raw JSON text is redacted as text (streamed tool input joined, a frame that didn't parse), and
/// goes on to Diagnostics and bundles: a wait for a secret that meets the JSON's own quotes and
/// brackets ends there, and the text stays JSON.
#[test]
fn a_wait_for_a_secret_stops_at_json_structure() {
    let untouched = [
        r#"{"pattern":"Bearer"}"#,
        r#"{"command":"x --token"}"#,
        r#"{"args":["--token"],"cwd":"X"}"#,
        r#"{"pattern":"Bearer ","path":"src"}"#,
        r#"{"header":"Authorization: ","n":1}"#,
        r#"{"header":"X-Api-Key: "}"#,
        r#"{"command":"export FOO=","n":1}"#,
        r#"{"command":"x --token=","n":1}"#,
        r#"{"command":"x --token= ","n":1}"#,
        r#"{"command":"$env:GH_TOKEN = ","n":1}"#,
        r#"["x -Token:","next"]"#,
    ];
    let hidden = [
        (
            r#"{"args":["--token","abc"],"n":1}"#,
            r#"{"args":["--token","[redacted:flag]"],"n":1}"#,
        ),
        (r#"{"--token":"abc"}"#, r#"{"--token":"[redacted:flag]"}"#),
        (
            r#"{"command":"x --token abc","n":1}"#,
            r#"{"command":"x --token [redacted:flag]","n":1}"#,
        ),
        (
            r#"{"command":"export FOO=bar","n":1}"#,
            r#"{"command":"export FOO=[redacted:env]","n":1}"#,
        ),
        (
            r#"{"header":"Authorization: Basic abc"}"#,
            r#"{"header":"Authorization: Basic [redacted:header]"}"#,
        ),
        // As before this change: the wait goes on past a comma and a space to the next string.
        (
            r#"{"a": "Bearer", "b": "c"}"#,
            r#"{"a": "Bearer", "[redacted:bearer]": "c"}"#,
        ),
    ];
    let same = untouched.iter().map(|text| (*text, *text));
    for (input, expected) in same.chain(hidden) {
        let out = redact(input);
        assert_eq!(out, expected, "input: {input}");
        serde_json::from_str::<Value>(&out).unwrap_or_else(|e| panic!("{out} is not JSON: {e}"));
    }
}

/// The same through the stream's own path: fragments joined, redacted, and put back.
#[test]
fn streamed_tool_input_stays_json_when_a_flag_ends_its_string() {
    let piece = |text: &str| {
        json!({ "type": "stream_event", "event": { "type": "content_block_delta", "index": 1,
            "delta": { "type": "input_json_delta", "partial_json": text } } })
    };
    let mut run = [
        piece(r#"{"args":["--token","ab"#),
        piece(r#"c"],"command":"x --tok"#),
        piece(r#"en"}"#),
    ];
    super::stream::redact_run(&mut run);
    let joined: String = run
        .iter()
        .map(|f| f["event"]["delta"]["partial_json"].as_str().unwrap())
        .collect();
    assert_eq!(
        joined,
        r#"{"args":["--token","[redacted:flag]"],"command":"x --token"}"#
    );
    serde_json::from_str::<Value>(&joined).expect("the joined input is still JSON");
}

/// A URL glued to what comes before it, and what must not get weaker for it: a secret flag or an
/// environment value that is a URL is still hidden whole.
#[test]
fn a_url_inside_a_longer_word_still_loses_its_password() {
    reads(&[
        (
            "pip install --index-url=https://bob:hunter2@pypi.example.test/simple x",
            "pip install --index-url=https://bob:[redacted:url]@pypi.example.test/simple x",
        ),
        (
            "see <https://bob:hunter2@example.test> now",
            "see <https://bob:[redacted:url]@example.test> now",
        ),
        (
            "DATABASE_URL=postgres://app:s3cret@db.example.test/main run",
            "DATABASE_URL=[redacted:env] run",
        ),
        (
            "x --token=https://bob:hunter2@example.test y",
            "x --token=[redacted:flag] y",
        ),
    ]);
    untouched(&["--index-url=https://pypi.example.test/simple a=b://c 9://x:y@z"]);
}

/// curl takes a header with no space after its colon. Only `Authorization` and `X-…` names are
/// read that way: `re-auth:needed` is prose.
#[test]
fn a_header_with_no_space_after_its_colon_hides_its_value() {
    reads(&[
        (
            "curl -H \"X-Api-Key:abc123\" x",
            "curl -H \"X-Api-Key:[redacted:header]\" x",
        ),
        (
            "Authorization:Basic dXNlcjpwYXNz",
            "Authorization:Basic [redacted:header]",
        ),
        (
            "Authorization:abc123 x",
            "Authorization:[redacted:header] x",
        ),
    ]);
    untouched(&[
        "re-auth:needed pre-token:abc x-ray:on X-Request-Id:abc",
        "x-auth://not-a-header Authorization:",
    ]);
}

#[test]
fn a_hyphened_header_name_is_a_secret_key_in_json_too() {
    let frame =
        json!({ "headers": { "X-Api-Key": "abc123", "x-auth-token": "t-1", "Accept": "a/b" } });
    let expected = json!({ "headers": { "X-Api-Key": "[redacted:named]",
        "x-auth-token": "[redacted:named]", "Accept": "a/b" } });
    assert_eq!(redact_json(&frame), expected);
}

/// A quoted secret may start with what JSON closes with. It reads as JSON's own structure only
/// when what follows the bracket, or the comma or colon, is what JSON would have there.
#[test]
fn a_quoted_secret_that_starts_like_json_structure_is_still_hidden() {
    reads(&[
        (
            "mysql --password \"}Q7x000pw\" db",
            "mysql --password \"[redacted:flag]\" db",
        ),
        (
            "mytool --password '}Q7x001pw'",
            "mytool --password '[redacted:flag]'",
        ),
        (
            "mytool --token \"]abc000sq\"",
            "mytool --token \"[redacted:flag]\"",
        ),
        (
            "mytool --token \":123abc000\"",
            "mytool --token \"[redacted:flag]\"",
        ),
        (
            "mytool --token \",-abc000c\"",
            "mytool --token \"[redacted:flag]\"",
        ),
        (
            "mytool --token \":-abc000\"",
            "mytool --token \"[redacted:flag]\"",
        ),
        (
            "mytool --token \":true00abc\"",
            "mytool --token \"[redacted:flag]\"",
        ),
        (
            "mytool --token \":1.5e3x\" y",
            "mytool --token \"[redacted:flag]\" y",
        ),
        ("Bearer \"}brr000x\"", "Bearer \"[redacted:bearer]\""),
        (
            "echo Bearer \"]brr001x\"",
            "echo Bearer \"[redacted:bearer]\"",
        ),
        (
            "mytool --password=\"}Q7x003pw\"",
            "mytool --password=\"[redacted:flag]\"",
        ),
        (
            "export API_KEY=\"}env000x\"",
            "export API_KEY=\"[redacted:env]\"",
        ),
        (
            "$env:GH_TOKEN = \"}ps000x\"",
            "$env:GH_TOKEN = \"[redacted:env]\"",
        ),
        (
            "Authorization: \"}auth000x\"",
            "Authorization: \"[redacted:header]\"",
        ),
        // Not quoted, starting with a bracket: the bracket is passed, the rest goes.
        ("x --password }abc y", "x --password }[redacted:flag] y"),
        ("x --token=]abc y", "x --token=][redacted:flag] y"),
    ]);
}

/// A value with no quotes ends at JSON's closing brackets, so they are still there after it.
#[test]
fn a_bare_value_leaves_jsons_closing_brackets() {
    reads(&[
        ("[\"Bearer\",5]", "[\"Bearer\",[redacted:bearer]]"),
        (
            "{\"args\":[\"--token\",1]}",
            "{\"args\":[\"--token\",[redacted:flag]]}",
        ),
    ]);
    let nested = r#"{"a":{"b":["x --token"]},"c":["Bearer "],"d":{"e":"FOO="} , "n":1}"#;
    assert_eq!(redact(nested), nested);
}

/// A key standing as a URL's whole login, redacted a second time: the mark's own colon was read
/// as the one between a name and a password.
#[test]
fn a_key_as_a_urls_login_reads_the_same_redacted_twice() {
    for prefix in ["ghp_", "xoxb-"] {
        let key = format!("{prefix}{}", "0a1B".repeat(9));
        let once = redact(&format!("git clone https://{key}@example.test/x/y.git"));
        assert_eq!(
            once,
            "git clone https://[redacted:url]@example.test/x/y.git"
        );
        assert_eq!(redact(&once), once);
    }
}

#[test]
fn a_key_named_after_a_longer_run_together_word_counts() {
    reads(&[
        (
            "x --secretaccesskey abc",
            "x --secretaccesskey [redacted:flag]",
        ),
        (
            "x --awssecretaccesskey=abc y",
            "x --awssecretaccesskey=[redacted:flag] y",
        ),
        ("x --myapikey abc", "x --myapikey [redacted:flag]"),
    ]);
    untouched(&["x --monkey 1 --hotkey f5 --turkey x --keychain login --donkey d"]);
}
