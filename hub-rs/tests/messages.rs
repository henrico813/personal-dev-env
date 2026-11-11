mod common;

use std::fs;
use std::path::Path;
use std::time::Duration;

use hub::commands::Outcome;
use hub::commands::next::{NextOptions, run_next};
use hub::commands::send::{SendRequest, run_inbox, run_send};
use hub::outside::herdr::{Agent, AgentStatus, HerdrCode};
use hub::shared::{HubError, Tools};
use serde_json::Value;

use common::{InMemoryHerdr, TestTools, make_agent, make_refusal, make_time, read_events};

/// OpenCode's input line holding a message it did not send.
const PASTED_SCREEN: &str = "  some output\n  ┃  [Pasted ~1 lines]\n";

/// Runs `hub send NAME TEXT` on this machine at `22:14`.
fn send(
    tools: &Tools<'_>,
    test_tools: &TestTools,
    name: &str,
    text: &str,
) -> Result<Outcome, HubError> {
    let request = SendRequest {
        name: name.parse().unwrap(),
        machine: None,
        text: text.to_owned(),
    };
    run_send(
        tools,
        &test_tools.run(),
        &request,
        &make_time("2026-11-11T22:14:00+00:00[UTC]"),
    )
}

/// Runs one `hub next` check and returns its line.
fn check_once(test_tools: &TestTools) -> String {
    let options = NextOptions {
        timeout: Some(Duration::ZERO),
        poll: Duration::ZERO,
        json: false,
        process_id: 1,
    };
    let outcome = run_next(&test_tools.tools(), &test_tools.run(), &options).unwrap();
    outcome.lines.join("\n")
}

/// Returns the file names in the run folder's `sends/`, sorted.
fn list_send_files(run_path: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(run_path.join("sends"))
        .map(|entries| {
            entries
                .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// Returns the message JSON in one file of `sends/`.
fn read_send_file(run_path: &Path, file_name: &str) -> Value {
    let text = fs::read_to_string(run_path.join("sends").join(file_name)).unwrap();
    serde_json::from_str(&text).unwrap()
}

/// A message to a working OpenCode worker goes at once, so it lands in
/// the same turn. Sending it twice makes the worker do the work twice,
/// and a file left behind would show it as unsure.
#[test]
fn run_send_prompts_free_session_once() {
    let mut test_tools = TestTools::new();
    test_tools.herdr.agents = vec![make_agent("docs-fix", AgentStatus::Working)];

    let outcome = send(
        &test_tools.tools(),
        &test_tools,
        "docs-fix",
        "Rebase on main.",
    )
    .unwrap();

    assert_eq!(outcome.lines, ["sent docs-fix"]);
    assert_eq!(
        *test_tools.herdr.prompts.borrow(),
        [("docs-fix".to_owned(), "Rebase on main.".to_owned())]
    );
    assert_eq!(
        list_send_files(&test_tools.run_path()),
        Vec::<String>::new()
    );
    let events = read_events(&test_tools.run_path());
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["event"], "delivered");
    assert_eq!(events[0]["via"], "send");
}

/// A message to a free session still waits behind that session's older
/// held messages, and `hub send` says how many. Counting other
/// sessions' messages holds it for nothing and tells the hub a wrong
/// count.
#[test]
fn run_send_counts_older_messages_of_session() {
    let mut test_tools = TestTools::new();
    test_tools.herdr.agents = vec![
        make_agent("docs-fix", AgentStatus::Blocked),
        make_agent("notes-fix", AgentStatus::Blocked),
    ];
    send(&test_tools.tools(), &test_tools, "docs-fix", "First.").unwrap();
    send(&test_tools.tools(), &test_tools, "docs-fix", "Second.").unwrap();
    send(&test_tools.tools(), &test_tools, "notes-fix", "Check CI.").unwrap();
    test_tools.herdr.agents = vec![
        make_agent("docs-fix", AgentStatus::Idle),
        make_agent("notes-fix", AgentStatus::Blocked),
    ];

    let outcome = send(&test_tools.tools(), &test_tools, "docs-fix", "Third.").unwrap();

    assert_eq!(outcome.lines, ["held docs-fix: 2 older messages wait"]);
    assert_eq!(test_tools.herdr.prompts.borrow().len(), 0);
}

/// When Herdr fails, the text must stay on disk. After a timeout it may
/// have arrived, so it stays `.sending` and the hub checks the pane
/// before sending again. After a refusal it did not arrive, so it
/// becomes `.notsent` with the reason, ready to send again.
#[test]
fn run_send_keeps_text_when_herdr_fails() {
    struct Case {
        name: &'static str,
        code: HerdrCode,
        file: &'static str,
        event: &'static str,
        unsure: bool,
    }
    let cases = [
        Case {
            name: "unsure",
            code: HerdrCode::Timeout,
            file: "docs-fix.20261111-221400.1.sending",
            event: "unsure",
            unsure: true,
        },
        Case {
            name: "not-sent",
            code: HerdrCode::Other("agent_blocked".to_owned()),
            file: "docs-fix.20261111-221400.1.notsent",
            event: "not_sent",
            unsure: false,
        },
    ];

    for case in cases {
        let mut test_tools = TestTools::new();
        test_tools.herdr.agents = vec![make_agent("docs-fix", AgentStatus::Idle)];
        test_tools
            .herdr
            .fail_next("prompt", make_refusal(case.code));

        let error = send(
            &test_tools.tools(),
            &test_tools,
            "docs-fix",
            "Rebase on main.",
        )
        .unwrap_err();

        let run_path = test_tools.run_path();
        assert_eq!(list_send_files(&run_path), [case.file], "{}", case.name);
        let message = read_send_file(&run_path, case.file);
        assert_eq!(message["text"], "Rebase on main.", "{}", case.name);
        assert_eq!(
            read_events(&run_path)[0]["event"],
            case.event,
            "{}",
            case.name
        );
        match error {
            HubError::Unsure { .. } => assert!(case.unsure, "{}", case.name),
            HubError::NotSent { .. } => {
                assert!(!case.unsure, "{}", case.name);
                assert_eq!(message["reason"], "Herdr said no", "{}", case.name);
            }
            other => panic!("{}: {other:?}", case.name),
        }
    }
}

/// A message that may have arrived keeps its `.sending` file until the
/// hub checks the pane. A newer message in the same second must take
/// the next number, or claiming it later renames over that file and
/// the older text is lost.
#[test]
fn run_send_numbers_past_unsure_message() {
    let mut test_tools = TestTools::new();
    test_tools.herdr.agents = vec![make_agent("docs-fix", AgentStatus::Idle)];
    test_tools
        .herdr
        .fail_next("prompt", make_refusal(HerdrCode::Timeout));
    send(&test_tools.tools(), &test_tools, "docs-fix", "First.").unwrap_err();
    test_tools.herdr.agents = vec![make_agent("docs-fix", AgentStatus::Blocked)];

    send(&test_tools.tools(), &test_tools, "docs-fix", "Second.").unwrap();

    assert_eq!(
        list_send_files(&test_tools.run_path()),
        [
            "docs-fix.20261111-221400.1.sending",
            "docs-fix.20261111-221400.2.held",
        ]
    );
}

/// A stalled prompt can leave the text pasted but not sent, and one
/// Enter sends it. Enter with nothing pasted, or in a pane the user
/// focused meanwhile, would send whatever the user typed instead. Enter
/// in a permission prompt that opened meanwhile would answer it.
#[test]
fn run_send_presses_enter_only_when_safe() {
    struct Case {
        name: &'static str,
        screen: &'static str,
        /// The agent as it is once the prompt was tried.
        after_prompt: Vec<Agent>,
        sent: bool,
    }
    let cases = [
        Case {
            name: "pasted-free",
            screen: PASTED_SCREEN,
            after_prompt: Vec::new(),
            sent: true,
        },
        Case {
            name: "nothing-pasted",
            screen: "  some output\n  ┃\n",
            after_prompt: Vec::new(),
            sent: false,
        },
        Case {
            name: "user-focused",
            screen: PASTED_SCREEN,
            after_prompt: vec![Agent {
                focused: true,
                ..make_agent("docs-fix", AgentStatus::Idle)
            }],
            sent: false,
        },
        Case {
            name: "blocked-meanwhile",
            screen: PASTED_SCREEN,
            after_prompt: vec![make_agent("docs-fix", AgentStatus::Blocked)],
            sent: false,
        },
    ];

    for case in cases {
        let mut test_tools = TestTools::new();
        test_tools.herdr.agents = vec![make_agent("docs-fix", AgentStatus::Idle)];
        test_tools.herdr.screens = [("docs-fix".to_owned(), case.screen.to_owned())].into();
        test_tools.herdr.after_prompt = case.after_prompt;
        test_tools
            .herdr
            .fail_next("prompt", make_refusal(HerdrCode::AgentPromptStalled));

        let result = send(
            &test_tools.tools(),
            &test_tools,
            "docs-fix",
            "Rebase on main.",
        );

        let pressed = test_tools.herdr.pressed.borrow().len();
        if case.sent {
            assert_eq!(result.unwrap().lines, ["sent docs-fix"], "{}", case.name);
            assert_eq!(pressed, 1, "{}", case.name);
        } else {
            let error = result.unwrap_err();
            assert!(
                matches!(error, HubError::Unsure { .. }),
                "{}: {error:?}",
                case.name
            );
            assert_eq!(pressed, 0, "{}", case.name);
        }
    }
}

/// A message held for a blocked session must go out once it frees,
/// exactly once. Held forever, the worker never hears it; sent on every
/// check, the worker does the work again each time.
#[test]
fn run_next_sends_held_message_once_free() {
    let mut test_tools = TestTools::new();
    test_tools.herdr.agents = vec![make_agent("docs-fix", AgentStatus::Blocked)];
    let held = send(
        &test_tools.tools(),
        &test_tools,
        "docs-fix",
        "Rebase on main.",
    )
    .unwrap();
    check_once(&test_tools);
    let prompts_while_blocked = test_tools.herdr.prompts.borrow().len();
    test_tools.herdr.agents = vec![make_agent("docs-fix", AgentStatus::Idle)];

    check_once(&test_tools);
    check_once(&test_tools);

    assert_eq!(held.lines, ["held docs-fix: it is blocked"]);
    assert_eq!(prompts_while_blocked, 0);
    assert_eq!(
        *test_tools.herdr.prompts.borrow(),
        [("docs-fix".to_owned(), "Rebase on main.".to_owned())]
    );
    assert_eq!(
        list_send_files(&test_tools.run_path()),
        Vec::<String>::new()
    );
}

/// Messages to one session queue and go out in the order they were
/// sent, even in the same second. A newer answer arriving first
/// confuses the worker.
#[test]
fn run_next_sends_held_messages_oldest_first() {
    let mut test_tools = TestTools::new();
    test_tools.herdr.agents = vec![make_agent("docs-fix", AgentStatus::Blocked)];
    for text in ["First.", "Second.", "Third."] {
        send(&test_tools.tools(), &test_tools, "docs-fix", text).unwrap();
    }
    test_tools.herdr.agents = vec![make_agent("docs-fix", AgentStatus::Idle)];

    check_once(&test_tools);
    check_once(&test_tools);
    check_once(&test_tools);

    let texts: Vec<String> = test_tools
        .herdr
        .prompts
        .borrow()
        .iter()
        .map(|(_, text)| text.clone())
        .collect();
    assert_eq!(texts, ["First.", "Second.", "Third."]);
}

/// A first message can make a worker ask for permission a moment after
/// Herdr shows it working. Text typed into that permission prompt
/// answers it, so the next message must wait for a later check.
#[test]
fn run_next_holds_second_message_until_later_check() {
    let mut test_tools = TestTools::new();
    test_tools.herdr.agents = vec![make_agent("docs-fix", AgentStatus::Blocked)];
    send(&test_tools.tools(), &test_tools, "docs-fix", "First.").unwrap();
    send(&test_tools.tools(), &test_tools, "docs-fix", "Second.").unwrap();
    test_tools.herdr.agents = vec![make_agent("docs-fix", AgentStatus::Idle)];
    test_tools.herdr.after_prompt = vec![make_agent("docs-fix", AgentStatus::Working)];

    check_once(&test_tools);

    assert_eq!(
        *test_tools.herdr.prompts.borrow(),
        [("docs-fix".to_owned(), "First.".to_owned())]
    );
    assert_eq!(
        list_send_files(&test_tools.run_path()),
        ["docs-fix.20261111-221400.2.held"]
    );
}

/// Each prompt in a check can take up to a minute, and the user may
/// enter another session's pane meanwhile. That session's message must
/// wait, or its text lands in the middle of what the user is typing.
#[test]
fn run_next_holds_message_focused_during_check() {
    let mut test_tools = TestTools::new();
    test_tools.herdr.agents = vec![
        make_agent("docs-fix", AgentStatus::Blocked),
        make_agent("notes-fix", AgentStatus::Blocked),
    ];
    send(&test_tools.tools(), &test_tools, "docs-fix", "First.").unwrap();
    send(&test_tools.tools(), &test_tools, "notes-fix", "Second.").unwrap();
    test_tools.herdr.agents = vec![
        make_agent("docs-fix", AgentStatus::Idle),
        make_agent("notes-fix", AgentStatus::Idle),
    ];
    test_tools.herdr.after_prompt = vec![Agent {
        focused: true,
        ..make_agent("notes-fix", AgentStatus::Idle)
    }];

    check_once(&test_tools);

    assert_eq!(
        *test_tools.herdr.prompts.borrow(),
        [("docs-fix".to_owned(), "First.".to_owned())]
    );
    assert_eq!(
        list_send_files(&test_tools.run_path()),
        ["notes-fix.20261111-221400.1.held"]
    );
}

/// A worker that hands off on its card leaves a new session on the same
/// card. A message held for the old name must reach the newest session
/// on the card. Left behind, it is lost with the old session, and sent
/// to an older session still open there, it reaches the wrong worker.
#[test]
fn run_next_moves_message_to_card_successor() {
    let mut test_tools = TestTools::new();
    test_tools.herdr.agents = vec![make_agent("demo-kb-012-impl", AgentStatus::Blocked)];
    send(
        &test_tools.tools(),
        &test_tools,
        "demo-kb-012-impl",
        "Rebase on main.",
    )
    .unwrap();
    test_tools.herdr.agents = vec![
        Agent {
            session_id: Some("ses_plan".to_owned()),
            ..make_agent("demo-kb-012-plan", AgentStatus::Idle)
        },
        Agent {
            session_id: Some("ses_new".to_owned()),
            ..make_agent("demo-kb-012-fix", AgentStatus::Idle)
        },
    ];
    test_tools.cards.sessions = [(
        "DEMO-KB-012".to_owned(),
        vec![
            "ses_plan".to_owned(),
            "ses_old".to_owned(),
            "ses_new".to_owned(),
        ],
    )]
    .into();

    check_once(&test_tools);
    check_once(&test_tools);

    assert_eq!(
        *test_tools.herdr.prompts.borrow(),
        [("demo-kb-012-fix".to_owned(), "Rebase on main.".to_owned())]
    );
    let events = read_events(&test_tools.run_path());
    let moved = events
        .iter()
        .find(|event| event["event"] == "moved")
        .unwrap();
    assert_eq!(moved["name"], "demo-kb-012-impl");
    assert_eq!(moved["to"], "demo-kb-012-fix");
}

/// Two processes may reach the same held message, such as `hub next`
/// and a worker's `hub inbox`. Only the one whose claim succeeds may
/// send it; the other must leave it alone, or the worker gets it twice.
#[test]
fn run_next_skips_message_claimed_elsewhere() {
    let mut test_tools = TestTools::new();
    test_tools.herdr.agents = vec![
        make_agent("docs-fix", AgentStatus::Blocked),
        make_agent("notes-fix", AgentStatus::Blocked),
    ];
    send(&test_tools.tools(), &test_tools, "docs-fix", "First.").unwrap();
    send(&test_tools.tools(), &test_tools, "notes-fix", "Second.").unwrap();
    let sends = test_tools.run_path().join("sends");
    // While docs-fix's prompt runs, another process claims notes-fix's
    // message.
    test_tools.herdr.on_prompt = Some(Box::new(move || {
        let held = sends.join("notes-fix.20261111-221400.1.held");
        let _ = fs::rename(&held, held.with_extension("sending"));
    }));
    test_tools.herdr.agents = vec![
        make_agent("docs-fix", AgentStatus::Idle),
        make_agent("notes-fix", AgentStatus::Idle),
    ];

    check_once(&test_tools);

    assert_eq!(
        *test_tools.herdr.prompts.borrow(),
        [("docs-fix".to_owned(), "First.".to_owned())]
    );
    assert_eq!(
        list_send_files(&test_tools.run_path()),
        ["notes-fix.20261111-221400.1.sending"]
    );
}

/// A message sent with `--machine laptop` must go through that
/// machine's Herdr when `hub next` sends it later. Sent through this
/// machine's Herdr, it reaches a different session or none.
#[test]
fn run_next_sends_through_message_machine() {
    let mut test_tools = TestTools::new();
    let laptop = InMemoryHerdr {
        agents: vec![make_agent("docs-fix", AgentStatus::Blocked)],
        ..InMemoryHerdr::default()
    };
    let request = SendRequest {
        name: "docs-fix".parse().unwrap(),
        machine: Some("laptop".to_owned()),
        text: "Rebase on main.".to_owned(),
    };
    let laptop_tools = Tools {
        herdr: &laptop,
        ..test_tools.tools()
    };
    let now = make_time("2026-11-11T22:14:00+00:00[UTC]");
    run_send(&laptop_tools, &test_tools.run(), &request, &now).unwrap();
    let laptop = InMemoryHerdr {
        agents: vec![make_agent("docs-fix", AgentStatus::Idle)],
        ..InMemoryHerdr::default()
    };
    test_tools.herdr.machines = [("laptop".to_owned(), laptop)].into();

    check_once(&test_tools);

    assert_eq!(test_tools.herdr.prompts.borrow().len(), 0);
    assert_eq!(
        *test_tools.herdr.machines["laptop"].prompts.borrow(),
        [("docs-fix".to_owned(), "Rebase on main.".to_owned())]
    );
}

/// A saved machine whose Herdr cannot be reached, such as a laptop
/// that is asleep, says nothing about its sessions. Its messages must
/// stay held for a later check, not become not sent as if the session
/// were gone.
#[test]
fn run_next_holds_message_of_unreachable_machine() {
    let mut test_tools = TestTools::new();
    let laptop = InMemoryHerdr {
        agents: vec![make_agent("docs-fix", AgentStatus::Blocked)],
        ..InMemoryHerdr::default()
    };
    let request = SendRequest {
        name: "docs-fix".parse().unwrap(),
        machine: Some("laptop".to_owned()),
        text: "Rebase on main.".to_owned(),
    };
    let laptop_tools = Tools {
        herdr: &laptop,
        ..test_tools.tools()
    };
    let now = make_time("2026-11-11T22:14:00+00:00[UTC]");
    run_send(&laptop_tools, &test_tools.run(), &request, &now).unwrap();
    let laptop = InMemoryHerdr::default();
    laptop.fail_next(
        "list_agents",
        make_refusal(HerdrCode::Other("server_unavailable".to_owned())),
    );
    test_tools.herdr.machines = [("laptop".to_owned(), laptop)].into();

    let line = check_once(&test_tools);

    assert_eq!(line, "timeout (hub -)");
    assert_eq!(
        list_send_files(&test_tools.run_path()),
        ["docs-fix.20261111-221400.1.held"]
    );
}

/// A held message whose session is gone, with no new session on its
/// card, must reach the hub as `not sent` with its text kept. Each one
/// needs its own line, since the hub hears of it only once.
#[test]
fn run_next_prints_each_not_sent_message() {
    let mut test_tools = TestTools::new();
    test_tools.herdr.agents = vec![
        make_agent("docs-fix", AgentStatus::Blocked),
        make_agent("notes-fix", AgentStatus::Blocked),
    ];
    send(&test_tools.tools(), &test_tools, "docs-fix", "First.").unwrap();
    send(&test_tools.tools(), &test_tools, "notes-fix", "Second.").unwrap();
    test_tools.herdr.agents = Vec::new();

    let first = check_once(&test_tools);
    let second = check_once(&test_tools);
    let third = check_once(&test_tools);

    assert_eq!(first, "not sent docs-fix: the session is gone (hub -)");
    assert_eq!(second, "not sent notes-fix: the session is gone (hub -)");
    assert_eq!(third, "timeout (hub -)");
    let run_path = test_tools.run_path();
    assert_eq!(
        list_send_files(&run_path),
        [
            "docs-fix.20261111-221400.1.notsent",
            "notes-fix.20261111-221400.1.notsent",
        ]
    );
    let message = read_send_file(&run_path, "docs-fix.20261111-221400.1.notsent");
    assert_eq!(message["text"], "First.");
    assert_eq!(message["reason"], "the session is gone");
}

/// When Herdr fails on a held message, `hub next` must print it, since
/// nothing prints it again. A refusal keeps the text as not sent with
/// its reason. A timeout keeps it as sending, since it may have arrived.
#[test]
fn run_next_prints_failed_held_message() {
    struct Case {
        name: &'static str,
        code: HerdrCode,
        line: &'static str,
        file: &'static str,
        reason: Option<&'static str>,
    }
    let cases = [
        Case {
            name: "not-sent",
            code: HerdrCode::Other("agent_blocked".to_owned()),
            line: "not sent docs-fix: Herdr said no (hub -)",
            file: "docs-fix.20261111-221400.1.notsent",
            reason: Some("Herdr said no"),
        },
        Case {
            name: "unsure",
            code: HerdrCode::Timeout,
            line: "unsure docs-fix: Herdr said no (hub -)",
            file: "docs-fix.20261111-221400.1.sending",
            reason: None,
        },
    ];

    for case in cases {
        let mut test_tools = TestTools::new();
        test_tools.herdr.agents = vec![make_agent("docs-fix", AgentStatus::Blocked)];
        send(
            &test_tools.tools(),
            &test_tools,
            "docs-fix",
            "Rebase on main.",
        )
        .unwrap();
        test_tools.herdr.agents = vec![make_agent("docs-fix", AgentStatus::Idle)];
        test_tools
            .herdr
            .fail_next("prompt", make_refusal(case.code));

        let line = check_once(&test_tools);

        assert_eq!(line, case.line, "{}", case.name);
        let run_path = test_tools.run_path();
        assert_eq!(list_send_files(&run_path), [case.file], "{}", case.name);
        let message = read_send_file(&run_path, case.file);
        assert_eq!(message["text"], "Rebase on main.", "{}", case.name);
        assert_eq!(message["reason"].as_str(), case.reason, "{}", case.name);
    }
}

/// A worker copies what `hub inbox` prints into its handoff. Each
/// printed message must count as delivered, or `hub next` sends it
/// again to a session that already has it.
#[test]
fn run_inbox_marks_held_messages_delivered() {
    let mut test_tools = TestTools::new();
    test_tools.herdr.agents = vec![make_agent("docs-fix", AgentStatus::Blocked)];
    send(&test_tools.tools(), &test_tools, "docs-fix", "First.").unwrap();
    send(&test_tools.tools(), &test_tools, "docs-fix", "Second.").unwrap();
    let name = "docs-fix".parse().unwrap();
    let now = make_time("2026-11-11T22:20:00+00:00[UTC]");

    let outcome = run_inbox(&test_tools.run(), &name, &now).unwrap();
    test_tools.herdr.agents = vec![make_agent("docs-fix", AgentStatus::Idle)];
    check_once(&test_tools);

    assert_eq!(
        outcome.lines,
        [
            "2 held messages for docs-fix",
            "--- 22:14",
            "First.",
            "--- 22:14",
            "Second.",
        ]
    );
    assert_eq!(test_tools.herdr.prompts.borrow().len(), 0);
    let events = read_events(&test_tools.run_path());
    let via_inbox = events
        .iter()
        .filter(|event| event["via"] == "inbox")
        .count();
    assert_eq!(via_inbox, 2);
}
