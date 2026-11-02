package view

import (
	"sort"
	"strconv"
	"strings"
	"time"

	"prs/internal/github"
)

// Holds one row of the comments screen, which is a comment, a review or a
// code thread.
type Item struct {
	post   post
	thread *github.Thread
}

// One comment or review in the conversation.
type post struct {
	who   string
	state string
	at    time.Time
	body  string
}

// The tags after a code thread's file and line.
var (
	openTag = coloredWords{
		words: "open",
		color: yellow,
	}
	outdatedTag = coloredWords{
		words: "outdated",
		color: gray,
	}
	resolvedTag = coloredWords{
		words: "resolved",
		color: green,
	}
)

// Returns the comments screen's items. Open code threads come first, since
// they are what needs a reply, then the conversation oldest first, then the
// resolved threads.
func CollectItems(f github.Feedback) []Item {
	var open, resolved []Item
	for i := range f.Threads {
		item := Item{thread: &f.Threads[i]}
		if f.Threads[i].IsResolved {
			resolved = append(
				resolved,
				item,
			)
		} else {
			open = append(
				open,
				item,
			)
		}
	}
	for _, post := range collectConversation(f) {
		open = append(
			open,
			Item{post: post},
		)
	}
	return append(
		open,
		resolved...,
	)
}

// Returns one row per item for fzf, the item's index first and hidden,
// then its kind or thread state, age and author, which come first since a
// phone cuts the row's end. A thread's row ends with its file and line, and
// a comment's or review's row with the start of its text.
func RenderItemRows(
	items []Item,
	s Style,
) string {
	dot := " " + s.paintGray("·") + " "
	var out strings.Builder
	for i, item := range items {
		var shown string
		if item.thread == nil {
			kind := "comment"
			if item.post.state != "" {
				kind = s.formatReview(item.post.state)
			}
			shown = kind + dot + s.paintGray(s.formatAge(item.post.at)) + dot + s.paintCyan(item.post.who)
			if first, _, _ := strings.Cut(
				strings.TrimSpace(item.post.body),
				"\n",
			); first != "" {
				shown += "  " + s.paintGray(first)
			}
		} else {
			thread := item.thread
			who, age := "", ""
			if len(thread.Comments) > 0 {
				who = thread.Comments[0].Author.Login
				age = s.formatAge(thread.Comments[0].CreatedAt)
			}
			var tags []string
			for _, tag := range listThreadTags(*thread) {
				tags = append(
					tags,
					s.paintWords(tag),
				)
			}
			shown = strings.Join(
				tags,
				" ",
			) + dot + s.paintGray(age) + dot + s.paintCyan(who) + dot + thread.Path + ":" + strconv.Itoa(findLine(*thread))
		}
		shown = rowBreaks.Replace(shown)
		out.WriteString(strconv.Itoa(i) + "\t" + shown + "\n")
	}
	return out.String()
}

// Returns an item in full. A comment or review shows its byline and text,
// and a thread shows its file and line, the last lines of its code and every
// comment.
func RenderItem(
	item Item,
	s Style,
) string {
	if item.thread == nil {
		text := s.formatByline(
			item.post.who,
			item.post.state,
			item.post.at,
		) + "\n"
		if item.post.body != "" {
			text += WrapProse(
				item.post.body,
				s.Width,
			) + "\n"
		}
		return text
	}
	thread := *item.thread
	var out strings.Builder
	out.WriteString(s.formatPlace(
		thread,
		listThreadTags(thread)...,
	) + "\n")
	if len(thread.Comments) > 0 {
		for _, line := range keepLastFourLines(thread.Comments[0].DiffHunk) {
			out.WriteString(s.paintHunkLine(line) + "\n")
		}
	}
	for _, comment := range thread.Comments {
		out.WriteString("\n" + s.formatByline(
			comment.Author.Login,
			"",
			comment.CreatedAt,
		) + "\n")
		out.WriteString(WrapProse(
			comment.Body,
			s.Width,
		) + "\n")
	}
	return out.String()
}

// The comment count of a pull request without comments.
const NoComments = "No comments"

// Returns the PR summary screen's line about the comments, such as
// "5 comments, 2 open threads". Comments counts the conversation's comments
// and reviews, and every comment in a code thread.
func FormatCommentCount(f github.Feedback) string {
	comments := len(collectConversation(f))
	open := 0
	for _, thread := range f.Threads {
		comments += len(thread.Comments)
		if !thread.IsResolved {
			open++
		}
	}
	if comments == 0 {
		return NoComments
	}
	return countNoun(
		comments,
		"comment",
	) + ", " + countNoun(
		open,
		"open thread",
	)
}

// Returns a count and its noun, such as "1 comment" or "2 comments".
func countNoun(
	n int,
	noun string,
) string {
	if n != 1 {
		noun += "s"
	}
	return strconv.Itoa(n) + " " + noun
}

// Returns a thread's open or resolved tag, then its outdated tag.
func listThreadTags(thread github.Thread) []coloredWords {
	tags := []coloredWords{openTag}
	if thread.IsResolved {
		tags = []coloredWords{resolvedTag}
	}
	if thread.IsOutdated {
		tags = append(
			tags,
			outdatedTag,
		)
	}
	return tags
}

// Returns the thread's line, or where it was when it is gone from the
// latest diff.
func findLine(thread github.Thread) int {
	if thread.Line == 0 {
		return thread.OriginalLine
	}
	return thread.Line
}

// Returns the comments and reviews in time order, without your unsent
// PENDING draft or a COMMENTED review with no text, whose comments show in
// their threads. An approval with no text stays, since it is a decision.
func collectConversation(f github.Feedback) []post {
	var posts []post
	for _, comment := range f.Comments {
		posts = append(
			posts,
			post{
				who:  comment.Author.Login,
				at:   comment.CreatedAt,
				body: comment.Body,
			},
		)
	}
	for _, review := range f.Reviews {
		if review.State == "PENDING" || (review.Body == "" && review.State == "COMMENTED") {
			continue
		}
		posts = append(
			posts,
			post{
				who:   review.Author.Login,
				state: review.State,
				at:    review.SubmittedAt,
				body:  review.Body,
			},
		)
	}
	sort.SliceStable(
		posts,
		func(
			i,
			j int,
		) bool {
			return posts[i].at.Before(posts[j].at)
		},
	)
	return posts
}

func (s Style) paintHunkLine(line string) string {
	switch {
	case line == "":
	case line[0] == '+':
		return s.paintGreen(line)
	case line[0] == '-':
		return s.paintRed(line)
	case strings.HasPrefix(
		line,
		"@@",
	):
		return s.paintCyan(line)
	}
	return s.paintGray(line)
}

func (s Style) formatByline(
	who string,
	state string,
	at time.Time,
) string {
	review := ""
	if state != "" {
		review = " " + s.formatReview(state)
	}
	return "-- " + s.paintCyan(who) + review + " " + s.paintGray(s.formatAge(at))
}

// Returns the thread's file and line, then its tags, with the path cut
// from the left so the file name and the tags fit.
func (s Style) formatPlace(
	thread github.Thread,
	tags ...coloredWords,
) string {
	number := strconv.Itoa(findLine(thread))
	var plain, painted string
	for _, tag := range tags {
		plain += " " + tag.words
		painted += " " + s.paintWords(tag)
	}
	room := s.Width - 4 - len(number) - len(plain)
	path := thread.Path
	if room >= 2 {
		path = cutLeft(
			path,
			room,
		)
	}
	return s.paintGray("##") + " " + s.paintBold(path+":"+number) + painted
}

func keepLastFourLines(text string) []string {
	lines := strings.Split(
		text,
		"\n",
	)
	return lines[max(
		0,
		len(lines)-4,
	):]
}
