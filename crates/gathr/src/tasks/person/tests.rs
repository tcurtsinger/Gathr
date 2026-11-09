use super::*;

fn task(form: Form, labels: &[&str]) -> Task {
    Task {
        number: 7,
        chat: "c".into(),
        giver: Some("lead".into()),
        owner: None,
        given_in: None,
        parent: None,
        kind: Kind::Person,
        what: "Which?".into(),
        why: None,
        state: State::Ready,
        reason: None,
        after: Vec::new(),
        blocked_on: None,
        pr: None,
        reviewed: None,
        outdated: false,
        form: Some(form),
        choices: labels
            .iter()
            .map(|l| Choice {
                label: l.to_string(),
                detail: None,
            })
            .collect(),
        files: Vec::new(),
        request: None,
        answer: None,
        created_us: 0,
    }
}

fn said(a: Answer, t: &Task) -> String {
    match answer_line(t, &a) {
        Ok(line) => line,
        Err(e) => format!("refused: {e}"),
    }
}

#[test]
fn each_form_takes_its_own_answer_or_words_and_refuses_the_rest() {
    let one = task(Form::Choice, &["A", "B"]);
    let any = task(Form::Choices, &["A", "B", "C"]);
    let yesno = task(Form::Yesno, &[]);
    let done = task(Form::Done, &[]);
    let text = task(Form::Text, &[]);
    let pick = |p: &[&str]| Answer {
        pick: p.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    };
    let words = |w: &str| Answer {
        text: Some(w.into()),
        ..Default::default()
    };
    let cases = [
        (said(pick(&["B"]), &one), "B"),
        (said(pick(&["A", "C"]), &any), "A, C"),
        (
            said(
                Answer {
                    yes: Some(false),
                    text: Some("not yet".into()),
                    ..Default::default()
                },
                &yesno,
            ),
            "No: not yet",
        ),
        (
            said(
                Answer {
                    done: true,
                    ..Default::default()
                },
                &done,
            ),
            "Done",
        ),
        (
            said(words("  Use the short one "), &text),
            "Use the short one",
        ),
        // Words alone answer any task: what's typed is never dropped (SerenityOS, 2026-11-04).
        (said(words("neither"), &one), "neither"),
        (
            said(pick(&["A", "B"]), &one),
            "refused: #7 is answered with one of its choices, or words",
        ),
        (said(pick(&["Z"]), &any), "refused: #7 has no choice Z"),
        (
            said(words("   "), &yesno),
            "refused: #7 is answered with yes or no, or words",
        ),
        (
            said(Answer::default(), &done),
            "refused: #7 is answered with done, or words",
        ),
    ];
    for (got, expected) in cases {
        assert_eq!(got, expected);
    }
}

#[test]
fn choices_come_with_a_choice_form_two_or_more_each_once() {
    let c = |labels: &[&str]| -> Vec<Choice> {
        labels
            .iter()
            .map(|l| Choice {
                label: l.to_string(),
                detail: None,
            })
            .collect()
    };
    assert!(check_choices(Form::Choice, &c(&["A", "B"])).is_ok());
    assert!(check_choices(Form::Text, &c(&[])).is_ok());
    let refused = |form, labels: &[&str]| match check_choices(form, &c(labels)) {
        Err(e) => e.to_string(),
        Ok(()) => "allowed".to_string(),
    };
    assert_eq!(
        refused(Form::Choice, &["A"]),
        "a choice needs at least 2 choices, your pick first"
    );
    assert_eq!(
        refused(Form::Yesno, &["A", "B"]),
        "choices go with form choice or choices, not yesno"
    );
    assert_eq!(
        refused(Form::Choices, &["A", "A"]),
        "the choice A is there twice"
    );
    assert!(refused(Form::Choice, &["A", " B"]).starts_with("a choice's label is 1 to 80"));
}
