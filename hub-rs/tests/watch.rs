mod common;

use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, SystemTime};

use hub::external::herdr::{Agent, AgentStatus, HerdrError};
use hub::shared::run_folder::RunFolder;
use hub::watch::background::start_in_background;
use hub::watch::wait::{WaitRequest, run_wait};
use hub::watch::{
    IdleClock, WatchOptions, WatchStart, check_once, restart_stopped_watch, run_watch,
    run_watch_start,
};
use jiff::{SignedDuration, Zoned};
use serde_json::{Value, json};

use common::{TempFolder, TestTools, make_agent, read_events, read_script_calls};

/// The worker most tests watch, named after its card as hubs name
/// workers.
const WORKER: &str = "demo-kb-007-impl";

/// Makes the run folder `herdr-hub.test` beside the test's other
/// files, a name the watch reads, with nothing in it.
fn make_run_folder(test_tools: &TestTools) -> RunFolder {
    let path = test_tools.path().join("herdr-hub.test");
    fs::create_dir(&path).unwrap();
    RunFolder::open(&path).unwrap()
}

/// Makes the run folder, with the live hub `kanban-hub-1` named in its
/// log, as `hub watch start` names it.
fn make_watched_run(test_tools: &mut TestTools) -> RunFolder {
    let run = make_run_folder(test_tools);
    test_tools
        .herdr
        .agents
        .push(make_agent("kanban-hub-1", AgentStatus::Idle));
    let started =
        json!({"at": "2026-11-12T08:00:00Z", "event": "watch_started", "hub": "kanban-hub-1"});
    append_event_line(run.path(), &started);
    run
}

/// Adds one line to the run folder's event log, as a hub command does.
fn append_event_line(run_path: &Path, event: &Value) {
    let mut log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(run_path.join("events.jsonl"))
        .unwrap();
    writeln!(log, "{event}").unwrap();
}

/// Watch options that read the test's run folders and send phone
/// notifications to the stand-in program `notifier`.
fn make_options(test_tools: &TestTools, notifier: &Path, checks: usize) -> WatchOptions {
    WatchOptions {
        poll: Duration::ZERO,
        checks: Some(checks),
        runs_folder: test_tools.path().to_owned(),
        notifier: notifier.to_owned(),
    }
}

/// Runs one check at each of these minutes after `start`, with one
/// watch's clock.
fn check_at(test_tools: &TestTools, start: &Zoned, minutes: &[i64]) {
    let notifier = test_tools.path().join("moshi-hook");
    let options = make_options(test_tools, &notifier, 1);
    let mut clock = IdleClock::default();
    for minute in minutes {
        let now = start
            .checked_add(SignedDuration::from_mins(*minute))
            .unwrap();
        let failures = check_once(&test_tools.tools(), &options, &mut clock, &now).unwrap();
        assert!(failures.is_empty(), "{failures:?}");
    }
}

/// Returns each text Herdr was asked to type into agent NAME's pane.
fn read_prompts(test_tools: &TestTools, name: &str) -> Vec<String> {
    test_tools
        .herdr
        .prompts
        .borrow()
        .iter()
        .filter(|(agent, _)| agent == name)
        .map(|(_, text)| text.clone())
        .collect()
}

/// Has the worker register a wait, as `hub wait` does.
fn register_wait(
    run: &RunFolder,
    pid: Option<u32>,
    file: Option<PathBuf>,
    minutes: u32,
    now: &Zoned,
) {
    let request = WaitRequest {
        name: WORKER.parse().unwrap(),
        pid,
        file,
        deadline_minutes: minutes,
    };
    run_wait(run, &request, now).unwrap();
}

/// Returns the ID of a process that already ended.
fn find_ended_pid() -> u32 {
    let mut child = Command::new("true").spawn().unwrap();
    let pid = child.id();
    child.wait().unwrap();
    pid
}

/// Sets a file's modification time a minute back.
fn age_file_one_minute(path: &Path) {
    let a_minute_ago = SystemTime::now() - Duration::from_secs(60);
    File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(a_minute_ago)
        .unwrap();
}

/// Holds this machine's watch lock as a running watch does, so nothing
/// starts another.
fn hold_watch_lock(test_tools: &TestTools) -> File {
    let folder = test_tools.state_folder().join("hub");
    fs::create_dir_all(&folder).unwrap();
    let lock = File::create(folder.join("watch.lock")).unwrap();
    lock.lock().unwrap();
    lock
}

/// Returns the calls a stand-in program started in the background
/// logged, waiting up to five seconds for the first.
fn wait_for_script_calls(script: &Path) -> Vec<Vec<String>> {
    for _ in 0..100 {
        let calls = read_script_calls(script);
        if !calls.is_empty() {
            return calls;
        }
        thread::sleep(Duration::from_millis(50));
    }
    Vec::new()
}

/// A worker that ends its turn with no report and no wait waits on
/// nobody, as one did for six hours overnight. It gets one prod after
/// 10 idle minutes and no more in that Herdr state, even from a
/// restarted watch, or a quiet worker is typed at again and again.
#[test]
fn check_once_prods_stalled_worker_once() {
    let mut test_tools = TestTools::new();
    make_watched_run(&mut test_tools);
    test_tools
        .herdr
        .agents
        .push(make_agent(WORKER, AgentStatus::Idle));
    let start = Zoned::now();

    check_at(&test_tools, &start, &[0, 9, 11, 12]);
    // A restarted watch starts a new clock.
    check_at(&test_tools, &start, &[20, 31]);

    let prompts = read_prompts(&test_tools, WORKER);
    assert_eq!(prompts.len(), 1, "{prompts:?}");
    assert!(
        prompts[0].starts_with("hub watch: you have been idle"),
        "{prompts:?}"
    );
}

/// A watch learns which prods it sent from every run's log. When two
/// runs logged prods of one worker, an older state number in one log
/// must not hide the newer one in the other. A review found a worker
/// prodded three times in three checks that way.
#[test]
fn check_once_keeps_newest_prod_across_runs() {
    let mut test_tools = TestTools::new();
    let older = make_watched_run(&mut test_tools);
    // This run comes first in folder order, so it checks the worker by
    // its card name.
    let newer = test_tools.path().join("herdr-hub.a");
    fs::create_dir(&newer).unwrap();
    test_tools
        .herdr
        .agents
        .push(make_agent("auto-hub-1", AgentStatus::Idle));
    let started =
        json!({"at": "2026-11-12T08:00:00Z", "event": "watch_started", "hub": "auto-hub-1"});
    append_event_line(&newer, &started);
    test_tools.herdr.agents.push(Agent {
        state_seq: 1,
        ..make_agent(WORKER, AgentStatus::Idle)
    });
    for (run_path, state_seq) in [(newer.as_path(), 1), (older.path(), 0)] {
        let prod = json!({"at": "2026-11-12T08:05:00Z", "event": "worker_prodded",
            "name": WORKER, "state_seq": state_seq, "why": "stalled"});
        append_event_line(run_path, &prod);
    }

    check_at(&test_tools, &Zoned::now(), &[0, 11, 12]);

    assert!(read_prompts(&test_tools, WORKER).is_empty());
}

/// A worker that reported and got no message since waits on the hub,
/// such as a finished worker before `hub clean` closes it. Prodded, it
/// spends a turn and may wake the hub with a needless report. A message
/// after its report means it owes a new one again.
#[test]
fn check_once_prods_only_worker_owing_report() {
    let cases = [
        ("reported-after-last-message", -2, 0),
        ("message-after-report", 0, 1),
    ];
    for (case_name, delivered_minutes, expected_prods) in cases {
        let mut test_tools = TestTools::new();
        let run = make_watched_run(&mut test_tools);
        test_tools
            .herdr
            .agents
            .push(make_agent("docs-fix", AgentStatus::Idle));
        let report = run.path().join("report-docs-fix.0800.read");
        fs::write(&report, "PR opened.\n").unwrap();
        age_file_one_minute(&report);
        let start = Zoned::now();
        let delivered = start.timestamp() + SignedDuration::from_mins(delivered_minutes);
        let message = json!({"at": delivered.to_string(), "event": "delivered",
            "name": "docs-fix", "id": "docs-fix.20261112-080000.1", "via": "send"});
        append_event_line(run.path(), &message);

        check_at(&test_tools, &start, &[0, 11]);

        let prompts = read_prompts(&test_tools, "docs-fix");
        assert_eq!(prompts.len(), expected_prods, "{case_name}: {prompts:?}");
    }
}

/// A worker that waits on a long job ends its turn, so nothing in its
/// chat wakes it. The watch prods it once the job's process ends, the
/// file it named changes or the deadline passes, and then drops the
/// wait, or the worker sleeps past the end of its job.
#[test]
fn check_once_prods_and_drops_ended_wait() {
    // Each case also writes the flag file, which only the file wait
    // watches.
    let cases = [
        ("pid-ended", Some(find_ended_pid()), false, 60, 1),
        ("file-changed", None, true, 60, 1),
        ("deadline-passed", Some(std::process::id()), false, 5, 6),
    ];
    for (case_name, pid, waits_for_file, deadline, minute) in cases {
        let mut test_tools = TestTools::new();
        let run = make_watched_run(&mut test_tools);
        test_tools
            .herdr
            .agents
            .push(make_agent(WORKER, AgentStatus::Idle));
        let flag = test_tools.path().join("done.flag");
        let start = Zoned::now();
        register_wait(
            &run,
            pid,
            waits_for_file.then(|| flag.clone()),
            deadline,
            &start,
        );
        fs::write(&flag, "done\n").unwrap();

        check_at(&test_tools, &start, &[minute]);

        let prompts = read_prompts(&test_tools, WORKER);
        assert_eq!(prompts.len(), 1, "{case_name}: {prompts:?}");
        assert!(
            prompts[0].starts_with("hub watch: your wait ended"),
            "{case_name}"
        );
        let wait_file = run.path().join(format!("waits/{WORKER}.json"));
        assert!(!wait_file.exists(), "{case_name}");
    }
}

/// A worker idle beside a job it registered with `hub wait` is not
/// stalled. Prodded, it would wake every 10 minutes to say the job
/// still runs.
#[test]
fn check_once_skips_stall_while_wait_runs() {
    let mut test_tools = TestTools::new();
    let run = make_watched_run(&mut test_tools);
    test_tools
        .herdr
        .agents
        .push(make_agent(WORKER, AgentStatus::Idle));
    let start = Zoned::now();
    register_wait(&run, Some(std::process::id()), None, 60, &start);

    check_at(&test_tools, &start, &[0, 11, 30]);

    assert!(read_prompts(&test_tools, WORKER).is_empty());
}

/// A worker blocked at an approval waits on the user even while the
/// hub's `hub next` is dead. Its pane shows "needs you" on the blocked
/// state only, so the mark goes once it works again.
#[test]
fn check_once_marks_blocked_worker() {
    let mut test_tools = TestTools::new();
    make_watched_run(&mut test_tools);
    test_tools
        .herdr
        .agents
        .push(make_agent(WORKER, AgentStatus::Blocked));

    check_at(&test_tools, &Zoned::now(), &[0]);

    assert_eq!(
        *test_tools.herdr.marks.borrow(),
        [(format!("w1:{WORKER}"), vec![AgentStatus::Blocked])]
    );
}

/// A report the hub was never told about means no `hub next` ran for
/// 10 minutes, as when reports sat unread until the user asked. The
/// hub hears of it from its next `hub next`, which reads the
/// `report_missed` event, once per report version. Nothing types into
/// the hub's pane, which may be the user's own chat.
#[test]
fn check_once_records_missed_report_without_typing() {
    let mut test_tools = TestTools::new();
    let run = make_watched_run(&mut test_tools);
    fs::write(run.path().join("report-docs-fix.md"), "PR opened.\n").unwrap();

    check_at(&test_tools, &Zoned::now(), &[0, 11, 12]);

    let missed: Vec<Value> = read_events(run.path())
        .into_iter()
        .filter(|event| event["event"] == "report_missed")
        .collect();
    assert_eq!(missed.len(), 1, "{missed:?}");
    assert_eq!(missed[0]["name"], "docs-fix");
    assert!(test_tools.herdr.prompts.borrow().is_empty());
}

/// With the run's hub gone from Herdr, no `hub next` will ever print a
/// missed report, so the user's phone hears of it once. While the hub
/// runs, the phone stays quiet, since the hub hears of it. A report a
/// day old sends nothing, since old run folders keep unread reports
/// from hubs closed long ago and would buzz the phone at every start.
#[test]
fn check_once_phones_only_when_hub_gone() {
    let cases = [
        ("hub-gone-report-11-minutes-old", false, 0, 1),
        ("hub-gone-report-a-day-old", false, 25 * 60, 0),
        ("hub-running", true, 0, 0),
    ];
    for (case_name, hub_runs, extra_minutes, expected_calls) in cases {
        let mut test_tools = TestTools::new();
        let run = make_watched_run(&mut test_tools);
        if !hub_runs {
            test_tools.herdr.agents.clear();
        }
        let notifier = test_tools.write_script("moshi-hook", "", "", 0);
        let report = run.path().join("report-docs-fix.md");
        fs::write(&report, "PR opened.\n").unwrap();
        let written = SystemTime::now() - Duration::from_secs(60 * extra_minutes);
        File::options()
            .write(true)
            .open(&report)
            .unwrap()
            .set_modified(written)
            .unwrap();

        check_at(&test_tools, &Zoned::now(), &[11, 12]);

        let calls = read_script_calls(&notifier);
        assert_eq!(calls.len(), expected_calls, "{case_name}: {calls:?}");
        if let Some(call) = calls.first() {
            assert_eq!(call[4], "kanban-hub-1 is gone", "{case_name}");
        }
    }
}

/// An auto run has two hubs on one machine, and each card's worker
/// belongs to one of them. A worker that reported to its own hub must
/// get no prod from the other run, whose only tie to it is the card in
/// its name. That holds after its own hub closed too, or the prod would
/// send its next report to the wrong hub.
#[test]
fn check_once_skips_worker_another_run_claims() {
    for (case_name, claiming_hub_runs) in [("own-hub-live", true), ("own-hub-closed", false)] {
        let mut test_tools = TestTools::new();
        let claiming = make_watched_run(&mut test_tools);
        if !claiming_hub_runs {
            test_tools.herdr.agents.clear();
        }
        // This run comes first in folder order, so it would check the
        // worker by its card name if no run claimed it.
        let other = test_tools.path().join("herdr-hub.auto");
        fs::create_dir(&other).unwrap();
        test_tools
            .herdr
            .agents
            .push(make_agent("auto-hub-1", AgentStatus::Idle));
        let started =
            json!({"at": "2026-11-12T08:00:00Z", "event": "watch_started", "hub": "auto-hub-1"});
        append_event_line(&other, &started);
        test_tools
            .herdr
            .agents
            .push(make_agent(WORKER, AgentStatus::Idle));
        let report = claiming.path().join(format!("report-{WORKER}.0800.read"));
        fs::write(&report, "PR opened.\n").unwrap();

        check_at(&test_tools, &Zoned::now(), &[0, 11]);

        assert!(read_prompts(&test_tools, WORKER).is_empty(), "{case_name}");
    }
}

/// A prod typed while the user is in the worker's pane lands in the
/// middle of what the user types. The watch holds it until the user
/// leaves the pane.
#[test]
fn check_once_skips_prod_while_pane_focused() {
    let mut test_tools = TestTools::new();
    make_watched_run(&mut test_tools);
    test_tools.herdr.agents.push(Agent {
        focused: true,
        ..make_agent(WORKER, AgentStatus::Idle)
    });

    check_at(&test_tools, &Zoned::now(), &[0, 11]);

    assert!(read_prompts(&test_tools, WORKER).is_empty());
}

/// A Herdr failure in one check goes to the log and leaves the watch
/// running. A watch that ended on it would leave every worker unwatched
/// until systemd or a worker's report starts it again.
#[test]
fn run_watch_logs_herdr_error_then_continues() {
    let mut test_tools = TestTools::new();
    make_watched_run(&mut test_tools);
    test_tools
        .herdr
        .agents
        .push(make_agent(WORKER, AgentStatus::Blocked));
    let no_server = HerdrError::Failed("herdr: no server at /run/herdr.sock".to_owned());
    test_tools.herdr.fail_next("list_agents", no_server);
    let options = make_options(&test_tools, Path::new("moshi-hook"), 2);
    let mut log = Vec::new();

    run_watch(&test_tools.tools(), &options, &mut log).unwrap();

    assert_eq!(
        String::from_utf8(log).unwrap(),
        "hub watch: herdr: no server at /run/herdr.sock\n"
    );
    assert_eq!(test_tools.herdr.marks.borrow().len(), 1);
}

/// OpenCode's background service can hand a tool command the
/// `HERDR_PANE_ID` of another pane. The hub is found by its own session
/// instead. Named from the pane, a worker would count as the run's
/// hub and the real hub as a worker.
#[test]
fn run_watch_start_trusts_session_over_pane() {
    let mut test_tools = TestTools::new();
    let run = make_run_folder(&test_tools);
    for (name, session) in [("kanban-hub-1", "ses_hub1"), ("docs-fix", "ses_docs1")] {
        test_tools.herdr.agents.push(Agent {
            session_id: Some(session.to_owned()),
            ..make_agent(name, AgentStatus::Idle)
        });
    }
    test_tools.own_pane = Some("w1:docs-fix".to_owned());
    let _lock = hold_watch_lock(&test_tools);
    let start = WatchStart {
        program: PathBuf::from("/usr/local/bin/hub"),
        session_id: Some("ses_hub1".to_owned()),
        runs_folder: test_tools.path().to_owned(),
    };

    let outcome = run_watch_start(&test_tools.tools(), &run, &start, &Zoned::now()).unwrap();

    assert_eq!(
        outcome.lines,
        ["hub watch runs; it watches the workers of kanban-hub-1"]
    );
    assert_eq!(read_events(run.path())[0]["hub"], "kanban-hub-1");
}

/// A watch that died comes back at a worker's next `hub report` or
/// `hub wait`. On a machine without the systemd unit, nothing else
/// starts it.
#[test]
fn restart_stopped_watch_starts_watch_again() {
    let mut test_tools = TestTools::new();
    let run = make_watched_run(&mut test_tools);
    let program = test_tools.write_script("hub", "", "", 0);

    let line = restart_stopped_watch(&run, test_tools.state_folder(), &program);

    assert_eq!(line.as_deref(), Some("hub watch restarted"));
    assert_eq!(wait_for_script_calls(&program), [["watch", "run"]]);
}

/// A report in a folder with no hub, such as a handoff folder or a
/// test's folder, starts no watch. Each such start would leave a
/// process that never ends.
#[test]
fn restart_stopped_watch_skips_run_without_hub() {
    let test_tools = TestTools::new();
    let run = make_run_folder(&test_tools);
    let program = test_tools.write_script("hub", "", "", 0);

    let line = restart_stopped_watch(&run, test_tools.state_folder(), &program);

    assert_eq!(line, None);
    assert!(read_script_calls(&program).is_empty());
}

/// The watch must outlive the harness command that started it.
/// OpenCode and Claude Code stop a command by signalling its process
/// group, so a watch left in that group would stop with no notice. The
/// test reads the program's output from the log, so output that misses
/// the log fails it too.
#[test]
fn start_in_background_leads_own_process_group() {
    let folder = TempFolder::new();
    let log = folder.path().join("watch.log");
    let args: [OsString; 2] = ["-c".into(), "echo $$ $(ps -o pgid= -p $$)".into()];

    let mut child = start_in_background(Path::new("/bin/sh"), &args, &log).unwrap();
    child.wait().unwrap();

    let written = fs::read_to_string(&log).unwrap();
    let ids: Vec<&str> = written.split_whitespace().collect();
    assert_eq!(ids.len(), 2, "{written}");
    assert_eq!(ids[0], ids[1], "{written}");
}
