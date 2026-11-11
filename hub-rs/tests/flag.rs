mod common;

use hub::commands::flag::{run_flag, run_unflag};
use hub::external::herdr::{Agent, AgentStatus};
use hub::shared::{AgentName, Tools};

use common::{InMemoryHerdr, make_agent};

/// The user flags a session to see "needs you" in the sidebar, and
/// unflags it after. Herdr marks panes, so the agent name in place
/// of its pane ID marks nothing. A blocked session marked on idle
/// would keep the mark after the user answers.
#[test]
fn run_flag_marks_pane_and_unflag_clears() {
    let cases = [
        (
            "idle",
            AgentStatus::Idle,
            vec![AgentStatus::Idle, AgentStatus::Done],
        ),
        ("blocked", AgentStatus::Blocked, vec![AgentStatus::Blocked]),
    ];

    for (case_name, status, expected_states) in cases {
        let herdr = InMemoryHerdr {
            agents: vec![Agent {
                pane_id: "w4:p2".to_owned(),
                ..make_agent("docs-fix", status)
            }],
            ..InMemoryHerdr::default()
        };
        let tools = Tools { herdr: &herdr };
        let name: AgentName = "docs-fix".parse().unwrap();

        run_flag(&tools, &name).unwrap();
        run_unflag(&tools, &name).unwrap();

        assert_eq!(
            *herdr.marks.borrow(),
            [("w4:p2".to_owned(), expected_states)],
            "{case_name}"
        );
        assert_eq!(*herdr.cleared.borrow(), ["w4:p2"], "{case_name}");
    }
}
