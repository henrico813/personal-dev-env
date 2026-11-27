//! This module holds the help that `glimmer help COMMAND` prints under each
//! command's usage.

pub(crate) const ABOUT: &str = "\
Reviews an agent's Go, Python and Rust comments with the user, one per
screen, and keeps agents from changing the comments the user approved.";

pub(crate) const GLIMMER: &str = "\
The steps run in order: check-comments, make-feedback-request-list,
then send-agent-suggestions. Run `glimmer help COMMAND` for what a
command prints and what to do about it.

Every command but screen, open, mcp and help prints one JSON reply:
  status  the review's status, or null when there is none or the
          command refused
  result  what the command found, or null when it refused
  error   null, or {code, message} when it refused
  next    the step to take next, or null
It exits with 0 without an error, 1 with one, and 2 for bad arguments.";

pub(crate) const BASE: &str = "Your task's start; defaults to the branch start";

pub(crate) const CHECK_COMMENTS: &str = "\
Checks the comments and function names in each Go, Python and Rust
file changed since the base, and writes nothing. Without a base, it
checks the changes since the branch split from main. result holds:

  failures     Fix each one, then run the check again.
  reported     Findings that do not fail. List each one in your
               final message.
  unsupported  Changed files in other languages, which the check
               cannot read. Check their comments and names yourself.

Each finding gives its file and line, its name (the function or type,
or null), the rule it breaks, a detail such as the filler phrase (or
null), and the sentence of the code-documentation skill that the rule
checks. A reported finding's note says why it does not fail: approved
(the user approved this text), existing (the text or function was
there at the base), header (a license header) or report_only (the
rule never fails).

error means the check did not run, such as for a file that does not
parse; do what its message says. next is make_feedback_request_list
once failures is empty, even with findings reported.";

pub(crate) const MAKE_FEEDBACK_REQUEST_LIST: &str = "\
Builds the Feedback Request List the user answers, and saves nothing.
Without a base, it lists the changes since the branch split from
main. It refuses with check_failed while check-comments finds
failures. result holds:

  list         {base, requests}. Each request asks about one comment:
               id, file, name, line, text, can_reword, reason, and
               suggestion, which is null for you to fill in.
  unsupported  Changed files no language rule reads.

reason is new, was_cut (the user cut this comment before),
differs_from_approved (the user approved other text) or
agent_wants_change (from a change request). A comment the branch did
not change is listed only for a change request; check-comments
reports one that goes against its approval or cut, with the note
existing. next is send_agent_suggestions, or
finish_without_feedback_requests when nothing needs the user.

A change request asks the user to reword a comment they approved.
Pass change requests in the --input file, or as the MCP tool's
change_requests list:

  {\"change_requests\": [{\"file\": \"store.go\", \"name\": \"LoadAnswers\"}]}

Give each file and name as the comment's entry shows them in the
approvals file, pde/code-comments.json in the clone's shared Git
folder. While a list waits for the user, this refuses with
waiting_for_user_feedback, and after a write into the code that did
not finish, with write_not_finished. While User Notes wait, it lists
the noted requests again with their IDs; pass the same base or none.";

pub(crate) const SEND_AGENT_SUGGESTIONS: &str = "\
Saves the Feedback Request List and puts it in front of the user.
Send make-feedback-request-list's result.list with a suggestion added
to each request and nothing else changed:

  {\"kind\": \"keep\", \"reasoning\": \"Says what it returns.\", \"also_at\": []}

kind is \"keep\", \"cut\" or {\"reword\": {\"text\": \"Returns the total.\"}}.
A reword's text is plain sentences without // or #, which glimmer
wraps and indents. A request whose can_reword is false takes only
keep or cut. The user sees reasoning as why:. also_at lists other
places that say the same thing, as file:line.

result.list_number numbers this list among the lists sent in this
review, from 1. next is tell_user_and_wait. On a refusal:

  list_changed        The code changed. Build the list again with the
                      same change requests, then add the suggestions
                      again.
  missing_suggestion, bad_input, reword_fails_rules, would_not_write
                      Fix what the message names and send again.
  any other code      Follow next, such as after check_failed or
                      waiting_for_user_feedback. When next is null,
                      do what the message says.";

pub(crate) const SHOW_STATUS: &str = "\
Shows what this worktree's review waits on and the step to take next,
and writes nothing. Run it at any time. status is
waiting_for_user_feedback, user_notes_waiting, writing_user_feedback,
done, dismissed, or null when there is no review. result holds:

  waiting      \"nothing\", {\"feedback_requests\": [ID, ...]}, the
               requests the user has not answered, or
               {\"user_notes\": {\"ID\": \"note\", ...}}, the User Notes
               you answer next.
  write_error  null, or {code, detail} when writing the answers into
               the code failed and the list waits for the user again.";

pub(crate) const SCREEN: &str = "\
Prints nothing. When the screen cannot open, it prints why and exits
with 1.";

pub(crate) const OPEN: &str = "\
Prints nothing. When the screen cannot open, it prints why and exits
with 1.";

pub(crate) const DISMISS: &str = "\
result.worktree names the worktree whose review changed.";

pub(crate) const RESTORE: &str = "\
result.worktree names the worktree whose review changed.";

pub(crate) const MCP: &str = "\
Serves check_comments, make_feedback_request_list,
send_agent_suggestions and show_status, one JSON-RPC message per line.
Each tool takes dir, the worktree's full path, and its command's
options: base, change_requests (the requests themselves, not a file)
and list (the list itself, not a file). Each gives the same JSON reply
as its command.";

pub(crate) const DROP_GONE_APPROVALS: &str = "\
Removes approvals and cuts for functions or files that no worktree
has any more, and refuses and removes nothing when a file does not
parse. result holds dropped, how many it removed, first_dropped, the
first few, and backup, {\"saved_at\": FILE} with the file as it was,
or \"nothing_written\".";
