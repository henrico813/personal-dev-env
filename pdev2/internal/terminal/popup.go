package terminal

import (
	"fmt"
	"io"
	"os"
	"os/signal"
	"strings"
	"sync"
	"syscall"
	"time"
	"unicode"

	"pdev2/internal/approval"
)

// Rows are 1-based, as in mouse reports. Buttons span the full width, so hit
// ignores x.
type Layout struct{ ApproveTop, approveBottom, DeclineTop, declineBottom int }

func (l Layout) hit(x, y int) string {
	if y >= l.ApproveTop && y <= l.approveBottom {
		return approval.ChoiceYes
	}
	if y >= l.DeclineTop && y <= l.declineBottom {
		return approval.ChoiceNo
	}
	return approval.ChoiceNone
}

// View is what the popup shows for one request. Queue reads like "1 of 2" and
// shares the first line with what is being asked. selected is
// approval.ChoiceYes or approval.ChoiceNo while a button waits for its
// confirming tap.
type View struct {
	Rec                            approval.Record
	Queue, typed, notice, selected string
}

// PopupScreen returns the popup's lines for a cols x rows terminal and the rows
// where a tap answers. docs/explanation/popup-and-phone-approval.md shows the
// result at Herdr's 37x14.
func PopupScreen(v View, cols, rows int) ([]string, Layout) {
	if cols < 1 {
		cols = 1
	}
	if rows < 1 {
		rows = 1
	}
	summary := approval.Summary(v.Rec)
	header := headerLine(visibleScreenText(summary[0]), v.Queue, cols)
	if rows < 7 {
		lines := []string{header}
		for len(lines) < rows {
			lines = append(lines, "")
		}
		return lines, Layout{}
	}
	// Herdr's popup is 37x14. Two-row buttons there leave six rows for the
	// context and the text that will be posted; three-row buttons left three,
	// which the context alone filled. From 16 rows, three-row buttons still
	// leave six.
	buttonHeight := 3
	if rows < 11 {
		buttonHeight = 1
	} else if rows < 16 {
		buttonHeight = 2
	}
	// Fixed rows: header, typed text and notice, key hint, and the gap between
	// buttons.
	requestRows := rows - (4 + buttonHeight*2)
	lines := []string{header}
	for _, text := range summary[1:] {
		if requestRows <= 0 {
			break
		}
		lines = append(lines, clip(visibleScreenText(text), cols))
		requestRows--
	}
	for requestRows > 0 {
		lines = append(lines, "")
		requestRows--
	}
	lines = append(lines, typedLine(v.typed, v.notice, cols))
	lines = append(lines, clip("v: full; g: focus; q/Esc: dismiss", cols))
	approveTop := len(lines) + 1
	lines = append(lines, buttonRows(buttonLabel("APPROVE", v.selected == approval.ChoiceYes), cols, buttonHeight)...)
	lines = append(lines, "")
	declineTop := len(lines) + 1
	lines = append(lines, buttonRows(buttonLabel("DECLINE", v.selected == approval.ChoiceNo), cols, buttonHeight)...)
	return lines, Layout{approveTop, approveTop + buttonHeight - 1, declineTop, declineTop + buttonHeight - 1}
}

// typedLine shares one row between the typed keys and the notice, such as
// "Typed: yes  one moment...". When both do not fit, the notice wins: it says
// what the next tap or Enter will do, and typing a key clears it.
func typedLine(typed, notice string, cols int) string {
	line := "Typed: " + typed
	if notice == "" {
		return clip(line, cols)
	}
	if both := line + "  " + notice; len([]rune(both)) <= cols {
		return both
	}
	return clip(notice, cols)
}

func headerLine(what, queue string, cols int) string {
	room := cols - len(queue) - 1
	if queue == "" || room < 1 {
		return clip(what, cols)
	}
	return fmt.Sprintf("%-*s %s", room, clip(what, room), queue)
}

// The agent writes the title and body. Control characters in them are shown as
// text, such as ^[ for ESC, so they cannot clear the screen or move the cursor
// and draw a different request over the real one.
func visibleScreenText(text string) string {
	var out strings.Builder
	for _, r := range text {
		if unicode.IsControl(r) {
			if r == 0x7f {
				out.WriteByte('^')
				out.WriteByte('?')
			} else if r < 0x20 {
				out.WriteByte('^')
				out.WriteByte(byte('@' + r))
			} else {
				fmt.Fprintf(&out, "\\u%04x", r)
			}
			continue
		}
		out.WriteRune(r)
	}
	return out.String()
}

func buttonLabel(name string, selected bool) string {
	if selected {
		return "[>> " + name + " <<]"
	}
	return "[ " + name + " ]"
}

func buttonRow(label string, width int) string {
	if label == "" {
		return strings.Repeat(" ", width)
	}
	if len(label) >= width {
		return label[:width]
	}
	left := (width - len(label)) / 2
	return strings.Repeat(" ", left) + label + strings.Repeat(" ", width-left-len(label))
}

func buttonRows(label string, width, height int) []string {
	rows := make([]string, height)
	for i := range rows {
		rows[i] = buttonRow("", width)
	}
	rows[height/2] = buttonRow(label, width)
	return rows
}

func drawPopup(in *os.File, out io.Writer, v View) Layout {
	cols, rows := terminalSize(in)
	lines, layout := PopupScreen(v, cols, rows)
	drawLines(out, lines)
	return layout
}

func drawLines(out io.Writer, lines []string) {
	fmt.Fprint(out, "\x1b[2J\x1b[H"+mouseReportingOn)
	fmt.Fprint(out, strings.Join(lines, "\r\n"))
}

// enterPopup makes in raw and throws away input typed before the popup
// draws. It returns the settings to restore and leave, which restores them
// and turns mouse reporting off.
func enterPopup(in *os.File, out io.Writer) (old *syscall.Termios, leave func(), err error) {
	old, err = setRaw(in)
	if err != nil {
		return nil, nil, err
	}
	leave = func() {
		fmt.Fprint(out, mouseReportingOff)
		restore(in, old)
	}
	if err := flushInput(in); err != nil {
		leave()
		return nil, nil, err
	}
	return old, leave, nil
}

// An impatient second tap would otherwise answer the request that just
// replaced the one it was meant for. Herdr also opens the first popup with
// focus while the user may be typing or tapping elsewhere.
const PopupSettle = 600 * time.Millisecond

const SettleNotice = "one moment..."

// AskPopup draws the popup and returns "yes" or "no" for a confirmed tap on a
// button or a typed yes or no and Enter, "dismiss" for q or Esc, or "focus"
// for g. Input typed before the popup is drawn is thrown away. The program
// asks Herdr to put the keyboard on the popup when it opens, so keys meant for
// another terminal could otherwise answer. settle ignores taps and Enter for
// that long after the popup opens; the queue passes PopupSettle for every
// request.
func AskPopup(in *os.File, out io.Writer, rec approval.Record, queue string, settle time.Duration) (string, error) {
	// Runs for each queued request too, so input left from the previous answer
	// is dropped.
	old, leave, err := enterPopup(in, out)
	if err != nil {
		return "", err
	}
	defer leave()
	v := &View{Rec: rec, Queue: queue}
	if settle > 0 {
		v.notice = SettleNotice
	}
	settleUntil := time.Now().Add(settle)
	// Registered before the first draw, so a resize right after it is not lost.
	resize := make(chan os.Signal, 1)
	signal.Notify(resize, syscall.SIGWINCH)
	defer signal.Stop(resize)
	layout := drawPopup(in, out, *v)
	// The loop holds mu except while waiting for input, so the timer that
	// clears the hint never draws at the same time or after the popup moved on.
	var mu sync.Mutex
	closed := false
	mu.Lock()
	defer mu.Unlock()
	defer func() { closed = true }()
	if settle > 0 {
		timer := time.AfterFunc(settle, func() {
			mu.Lock()
			defer mu.Unlock()
			if !closed && v.notice == SettleNotice {
				v.notice = ""
				layout = drawPopup(in, out, *v)
			}
		})
		defer timer.Stop()
	}
	// The reader goroutine reads only when asked, so nothing reads the
	// terminal while less has it, and the loop can redraw on a resize while
	// it waits.
	want := make(chan struct{})
	inputs := make(chan popupInput)
	defer close(want)
	go readPopupInput(in, want, inputs)
	for {
		want <- struct{}{}
		mu.Unlock()
		var input popupInput
		for waiting := true; waiting; {
			select {
			case input = <-inputs:
				waiting = false
			case <-resize:
				mu.Lock()
				layout = drawPopup(in, out, *v)
				mu.Unlock()
			}
		}
		mu.Lock()
		if input.err != nil {
			return "", input.err
		}
		if input.b == 'v' {
			// With reporting on, each tap would reach less as stray characters.
			// drawPopup turns reporting back on.
			fmt.Fprint(out, mouseReportingOff)
			restore(in, old)
			showDetails(rec)
			old, err = setRaw(in)
			if err != nil {
				return "", err
			}
			// Keys meant for less, such as a "yes" typed into its search, must not answer.
			if err := flushInput(in); err != nil {
				return "", err
			}
			layout = drawPopup(in, out, *v)
			continue
		}
		choice, redraw := v.handle(input, layout, time.Now().Before(settleUntil))
		if choice != approval.ChoiceNone {
			return choice, nil
		}
		if redraw {
			layout = drawPopup(in, out, *v)
		}
	}
}

func showDetails(rec approval.Record) {
	tty, err := os.OpenFile("/dev/tty", os.O_RDWR, 0)
	if err != nil {
		return
	}
	defer tty.Close()
	_ = ShowPager(tty, rec.ApprovalText)
}

func clip(s string, width int) string {
	if width <= 0 {
		return ""
	}
	if runes := []rune(s); len(runes) > width {
		return string(runes[:width])
	}
	return s
}
