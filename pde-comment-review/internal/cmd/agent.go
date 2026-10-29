package cmd

import (
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"slices"
	"strconv"
	"strings"

	"pde-comment-review/internal/review"
)

// What status, draft, set and submit tell an agent about the branch's
// review.
type agentReply struct {
	Review   string        `json:"review,omitempty"`
	State    string        `json:"state,omitempty"`
	Items    []review.Item `json:"items,omitempty"`
	Notes    []noteForAI   `json:"notes,omitempty"`
	Problems []string      `json:"problems,omitempty"`
	Next     string        `json:"next"`
}

// Returns the state of the branch's review at dir, with the items or
// notes the agent must act on.
func replyToStatus(dir string) agentReply {
	name, r, d, err := loadBranchReview(dir)
	if err != nil {
		return agentReply{Problems: listProblems(err), Next: "Fix each problem, then try again."}
	}
	reply, notes := agentReply{Review: name}, listNotes(r)
	switch {
	case d != nil && len(d.Proposals.Items) > 0:
		reply.State, reply.Items = "drafting", d.Proposals.Items
		reply.Next = "Call review_set for each item whose proposal should change, then call review_submit."
		// Draft is refused while a round waits, so only a first round can start over.
		if d.Round == 1 {
			reply.Next += " To start over, call review_draft."
		}
	case d != nil:
		reply.State = "done"
		reply.Next = "No comment on this branch needs the user's answer. Tell the user there is nothing to review."
	case len(r.Rounds) == 0:
		reply.State = "no_review"
		reply.Next = "Call review_draft to find the comments the user must answer."
	case r.AppliedRound == len(r.Rounds):
		reply.State = "done"
		reply.Next = "The user's answers are written into the code. Call review_draft after changing comments again."
	case len(notes) > 0:
		reply.State, reply.Notes = "notes_waiting", notes
		reply.Next = "The user left notes. Answer every note with review_set and its item's id, then call review_submit once."
	default:
		reply.State = "waiting_for_user"
		reply.Next = fmt.Sprintf("Tell the user: Review %[1]s is ready; press Ctrl+b e in Herdr, or run pde-comment-review open %[1]s. Do not answer its items yourself.", name)
	}
	return reply
}

// Saves the branch's draft, which only submit sends to the owner.
func replyToDraft(dir, checker string) agentReply {
	_, err := saveBranchDraft(dir, checker)
	return replyAfter(dir, err)
}

// Proposes keep, cut or reword for item id, with text as plain sentences.
func replyToSet(
	dir string,
	id int,
	answer string,
	text string,
) agentReply {
	err := changeDraftItem(
		dir,
		id,
		answer,
		text,
	)
	reply := replyAfter(dir, err)
	// The whole draft would fill a small model's context on every set.
	reply.Items = slices.DeleteFunc(reply.Items, func(item review.Item) bool { return item.ID != id })
	return reply
}

// Changes item id's proposal in the branch's draft, or starts the next
// round's draft for an item the owner left a note on.
func changeDraftItem(
	dir string,
	id int,
	answer string,
	text string,
) error {
	switch {
	case !slices.Contains([]string{review.ProposeKeep, review.ProposeReword, review.ProposeCut}, answer):
		return fmt.Errorf("answer %q must be keep, reword or cut", answer)
	case answer != review.ProposeReword && text != "":
		return fmt.Errorf("%s takes no text; only reword does", answer)
	case answer == review.ProposeReword && len(strings.Fields(text)) == 0:
		return errors.New("reword needs text, the new wording as plain sentences")
	case strings.HasPrefix(strings.TrimSpace(text), "//"):
		return errors.New("write the text as plain sentences, without //")
	}
	name, r, d, err := loadBranchReview(dir)
	if err != nil {
		return err
	}
	if d == nil && len(r.Rounds) > 0 && r.AppliedRound == len(r.Rounds) {
		return errors.New("the user's answers are already written; call review_draft to start a new review")
	}
	if d == nil {
		d = &review.Draft{Round: len(r.Rounds) + 1}
	}
	at := slices.IndexFunc(d.Proposals.Items, func(item review.Item) bool { return item.ID == id })
	notes := listNotes(r)
	noted := slices.IndexFunc(notes, func(n noteForAI) bool { return n.ID == id })
	switch {
	case at < 0 && noted < 0:
		return fmt.Errorf("item %d is not in the draft and has no note from the user, so it cannot change now", id)
	case at < 0:
		d.Proposals.Version, d.Proposals.Repo = review.Version, r.Repo()
		d.Proposals.Items = append(d.Proposals.Items, notes[noted].Item)
		at = len(d.Proposals.Items) - 1
	}
	item := &d.Proposals.Items[at]
	if item.CodeRemoved && answer != review.ProposeCut {
		data, err := os.ReadFile(filepath.Join(d.Proposals.Repo, item.File))
		if err != nil || !strings.Contains(string(data), item.Old) {
			return fmt.Errorf("item %d's code was removed, so only cut fits; to keep its comment, first put the code back with that comment", id)
		}
		item.CodeRemoved = false
	}
	item.Proposal, item.New = answer, ""
	if answer == review.ProposeReword {
		checker, err := findCheckerPath()
		if err != nil {
			return err
		}
		comment, err := convertSentencesToComment(
			checker,
			d.Proposals.Repo,
			item.File,
			item.Old,
			text,
		)
		if err != nil {
			return err
		}
		item.New = comment
	}
	return review.SaveDraft(name, *d)
}

// Sends the branch's draft to the owner as the next round.
func replyToSubmit(dir, checker string) agentReply {
	return replyAfter(dir, saveDraftAsNextRound(dir, checker))
}

func saveDraftAsNextRound(dir, checker string) error {
	name, _, d, err := loadBranchReview(dir)
	if err != nil {
		return err
	}
	if d == nil || len(d.Proposals.Items) == 0 {
		return errors.New("no draft waits to be sent")
	}
	data, err := json.Marshal(d.Proposals)
	if err == nil {
		_, err = saveRound(
			name,
			data,
			checker,
		)
	}
	return err
}

// Returns the name of the branch's review at dir, the review, and its
// unsent draft or nil.
func loadBranchReview(dir string) (string, *review.Review, *review.Draft, error) {
	repo, name, err := findBranchReviewName(dir)
	if err != nil {
		return "", nil, nil, err
	}
	r, d, err := review.LoadReviewAndDraft(name)
	if err != nil {
		return "", nil, nil, err
	}
	reviewRepo := repo
	if d != nil {
		reviewRepo = d.Proposals.Repo
	} else if len(r.Rounds) > r.AppliedRound {
		reviewRepo = r.Repo()
	}
	// Another clone can check out a branch of the same name.
	if reviewRepo != repo {
		return "", nil, nil, fmt.Errorf(
			"review %s is for %s, not %s; tell the owner if that is another checkout",
			name,
			reviewRepo,
			repo,
		)
	}
	return name, r, d, nil
}

// Returns the status after an agent command, with err's problems if it
// failed.
func replyAfter(dir string, err error) agentReply {
	reply := replyToStatus(dir)
	// A failed status already names the problem.
	if err != nil && len(reply.Problems) == 0 {
		reply.Items = nil
		reply.Problems = listProblems(err)
	}
	return reply
}

// Returns one problem per line of err, keeping an indented line, such as
// quoted comment text, with the line above.
func listProblems(err error) []string {
	var problems []string
	for _, line := range strings.Split(err.Error(), "\n") {
		if n := len(problems); n > 0 && strings.HasPrefix(line, " ") {
			problems[n-1] += "\n" + line
			continue
		}
		problems = append(problems, line)
	}
	return problems
}

// Runs set with the command line's id, answer and words of text.
func replyToSetArgs(args []string) agentReply {
	id, err := strconv.Atoi(args[0])
	if err != nil {
		return replyAfter(".", fmt.Errorf("item id %q must be a number", args[0]))
	}
	return replyToSet(
		".",
		id,
		args[1],
		strings.Join(args[2:], " "),
	)
}

// Prints reply with shell commands as its steps, and returns 1 on problems.
func printReply(w io.Writer, reply agentReply) int {
	fmt.Fprint(w, nameShellCommands(encodeReply(reply)))
	if len(reply.Problems) > 0 {
		return 1
	}
	return 0
}

// Returns text with each tool name, such as review_set, as its command.
func nameShellCommands(text string) string {
	for _, command := range []string{"status", "draft", "set", "submit"} {
		text = strings.ReplaceAll(
			text,
			"review_"+command,
			"pde-comment-review "+command,
		)
	}
	return text
}

// Returns reply as indented JSON that keeps < > and & as they are.
func encodeReply(reply agentReply) string {
	var out strings.Builder
	encoder := json.NewEncoder(&out)
	encoder.SetEscapeHTML(false)
	encoder.SetIndent("", "  ")
	// Strings, numbers and lists always encode.
	_ = encoder.Encode(reply)
	return out.String()
}
