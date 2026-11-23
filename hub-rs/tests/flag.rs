mod common;

use hub::commands::flag::{run_flag, run_unflag};
use hub::external::herdr::{Agent, AgentStatus};
use hub::shared::{AgentName, Tools};

use common::{InMemoryHerdr, TestTools, make_agent};

/// Herdr with one idle agent, `docs-fix`, in pane `w4:p2`.
fn make_flag_herdr() -> InMemoryHerdr {
    InMemoryHerdr {
        agents: vec![Agent {
            pane_id: "w4:p2".to_owned(),
            ..make_agent("docs-fix", AgentStatus::Idle)
        }],
        ..InMemoryHerdr::default()
    }
}

/// The user flags a session to see "needs you" in the sidebar, and
/// unflags it after. Herdr marks panes, so the agent name in place of
/// its pane ID marks nothing. Which states show the mark is the
/// `choose_marked_states` unit test's job.
#[test]
fn run_flag_marks_pane_and_unflag_clears() {
    let herdr = make_flag_herdr();
    let test_tools = TestTools::new();
    let tools = Tools {
        herdr: &herdr,
        ..test_tools.tools()
    };
    let name: AgentName = "docs-fix".parse().unwrap();

    run_flag(&tools, &name, true).unwrap();
    run_unflag(&tools, &name, true).unwrap();

    assert_eq!(herdr.marks.borrow().len(), 1);
    assert_eq!(herdr.marks.borrow()[0].0, "w4:p2");
    assert_eq!(*herdr.cleared.borrow(), ["w4:p2"]);
}

/// A flag or unflag moves the session in the "go to" list at once,
/// even after Herdr lost the agent filter. With `--machine`, this
/// machine's pass would read the wrong run folders and clear the
/// other machine's tags, so it must not run.
#[test]
fn run_flag_filters_list_only_here() {
    let cases = [("this-machine", true, 2), ("other-machine", false, 0)];

    for (case_name, runs_here, expected_filters) in cases {
        let herdr = make_flag_herdr();
        let test_tools = TestTools::new();
        let tools = Tools {
            herdr: &herdr,
            ..test_tools.tools()
        };
        let name: AgentName = "docs-fix".parse().unwrap();

        run_flag(&tools, &name, runs_here).unwrap();
        run_unflag(&tools, &name, runs_here).unwrap();

        assert_eq!(herdr.agent_filters.get(), expected_filters, "{case_name}");
    }
}
