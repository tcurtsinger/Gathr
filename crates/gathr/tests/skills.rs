//! Skills through a real service: the Skills folder is the list, skills made there by hand
//! included, and a change never reaches outside it.

mod common;

use std::path::PathBuf;

use common::{TempDir, TestService};
use gathr::folders::{beside, real};
use serde_json::Value;

/// The agent folder of a service on `data`: beside it, as for any data folder not the system's.
fn agents(data: &TempDir) -> PathBuf {
    real(&beside(&data.0))
}

fn write(path: PathBuf, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

#[test]
fn a_skill_name_or_path_that_leaves_the_skills_folder_is_refused_and_nothing_outside_changes() {
    let data = TempDir::new();
    let service = TestService::start(&data.0);
    let root = agents(&data);
    let outside = root.join("Kept").join("SKILL.md");
    write(
        outside.clone(),
        "---\nname: kept\ndescription: Not a skill\n---\n",
    );
    write(
        root.join("Skills").join("notes").join("SKILL.md"),
        "---\ndescription: Bo's notes\n---\n",
    );
    let auth = format!("Bearer {}", service.info.token);
    let post = |path: &str| {
        gathr::http::post(service.info.port, path, &[("Authorization", &auth)], "x").unwrap()
    };

    for name in [
        "..",
        "..%2FKept",
        "..%5CKept",
        "C%3A%5CKept",
        "%2Ftmp",
        "notes%2F..",
    ] {
        let removed = service.call("DELETE", &format!("/api/skills/{name}"));
        assert_eq!(removed.status, 409, "remove {name}: {}", removed.body);
        let added = post(&format!("/api/skills/{name}/files?path=SKILL.md&new=true"));
        assert_eq!(added.status, 409, "add to {name}: {}", added.body);
    }
    for path in [
        "../../Kept/SKILL.md",
        "..%5C..%5CKept%5CSKILL.md",
        "%2Ftmp%2Fx.md",
        "C%3A%2Fx.md",
        "a/../../x",
    ] {
        let added = post(&format!("/api/skills/notes/files?path={path}"));
        assert_eq!(added.status, 409, "a file at {path}: {}", added.body);
    }

    assert_eq!(
        std::fs::read_to_string(&outside).unwrap(),
        "---\nname: kept\ndescription: Not a skill\n---\n"
    );
    let mut left: Vec<String> = std::fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert_eq!(left, ["Kept", "Skills"]);
    let in_notes = std::fs::read_dir(root.join("Skills").join("notes"))
        .unwrap()
        .count();
    assert_eq!(in_notes, 1, "only its SKILL.md");
    let listed = service.call("GET", "/api/skills").json();
    assert_eq!(listed["skills"].as_array().unwrap().len(), 1, "{listed}");
}

#[test]
fn a_skill_folder_made_by_hand_is_listed_as_it_reads() {
    let data = TempDir::new();
    let service = TestService::start(&data.0);
    let skills = agents(&data).join("Skills");
    let folded = "---\nname: waiver-claims\ndescription: >\n  Rank waiver pickups\n  and size a bid\n---\n# Waivers\n";
    write(skills.join("waiver-claims").join("SKILL.md"), folded);
    write(
        skills.join("quoted").join("SKILL.md"),
        "---\ndescription: \"Ann's \\\"house\\\" style\"\n---\nText\n",
    );
    std::fs::create_dir_all(skills.join("empty")).unwrap();
    write(
        skills.join("plain").join("SKILL.md"),
        "Just text, no front matter\n",
    );
    write(
        skills.join("nameless").join("SKILL.md"),
        "---\nname: nameless\n---\n",
    );
    write(skills.join("stray.md"), "a file, not a folder");

    let listed = service.call("GET", "/api/skills").json();
    assert_eq!(
        listed["folder"],
        Value::from(skills.to_string_lossy().as_ref())
    );
    let shown: Vec<(String, Value, Value)> = listed["skills"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            assert_eq!(s["enabled"], true);
            (
                s["name"].as_str().unwrap().to_string(),
                s["description"].clone(),
                s["problem"].clone(),
            )
        })
        .collect();
    let row = |name: &str, d: Option<&str>, p: Option<&str>| {
        (name.to_string(), Value::from(d), Value::from(p))
    };
    assert_eq!(
        shown,
        [
            row("empty", None, Some("It has no SKILL.md")),
            row("nameless", None, Some("SKILL.md has no description")),
            row("plain", None, Some("SKILL.md has no front matter")),
            row("quoted", Some("Ann's \"house\" style"), None),
            row(
                "waiver-claims",
                Some("Rank waiver pickups and size a bid"),
                None
            ),
        ]
    );
    let one = service.call("GET", "/api/skills/waiver-claims").json();
    assert_eq!(one["text"], "# Waivers\n");
}
