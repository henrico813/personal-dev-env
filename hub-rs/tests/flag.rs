mod common;

use hub::commands::flag::{run_flag, run_unflag};
use hub::external::herdr::{Agent, AgentStatus};
use hub::shared::{AgentName, Tools};

use common::{InMemoryHerdr, TestTools, make_agent};

/// The user flags a session to see "needs you" in the sidebar, and
/// unflags it after. Herdr marks panes, so the agent name in place of
/// its pane ID marks nothing. Which states show the mark is the
/// `choose_marked_states` unit test's job.
#[test]
fn run_flag_marks_pane_and_unflag_clears() {
    let herdr = InMemoryHerdr {
        agents: vec![Agent {
            pane_id: "w4:p2".to_owned(),
            ..make_agent("docs-fix", AgentStatus::Idle)
        }],
        ..InMemoryHerdr::default()
    };
    let test_tools = TestTools::new();
    let tools = Tools {
        herdr: &herdr,
        ..test_tools.tools()
    };
    let name: AgentName = "docs-fix".parse().unwrap();

    run_flag(&tools, &name).unwrap();
    run_unflag(&tools, &name).unwrap();

    assert_eq!(herdr.marks.borrow().len(), 1);
    assert_eq!(herdr.marks.borrow()[0].0, "w4:p2");
    assert_eq!(*herdr.cleared.borrow(), ["w4:p2"]);
}
