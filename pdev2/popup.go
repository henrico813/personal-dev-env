package main

import (
	"bufio"
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

// With reporting on, the terminal sends each tap or click to the program as
// text on its input, such as "\x1b[<0;10;9M" for a left-button press at column
// 10, row 9; the release ends in "m". These are xterm modes 1000 and 1006.
const (
	mouseReportingOn  = "\x1b[?1000h\x1b[?1006h"
	mouseReportingOff = "\x1b[?1000l\x1b[?1006l"
)

const (
	keyBackspace = 0x08
	keyEscape    = 0x1b
	keyDelete    = 0x7f
)

// Rows are 1-based, as in mouse reports. Buttons span the full width, so hit
// ignores x.
type popupLayout struct{ approveTop, approveBottom, declineTop, declineBottom int }

func (l popupLayout) hit(x, y int) string {
	if y >= l.approveTop && y <= l.approveBottom {
		return approval.ChoiceYes
	}
	if y >= l.declineTop && y <= l.declineBottom {
		return approval.ChoiceNo
	}
	return approval.ChoiceNone
}

// What the popup shows for one request. queue reads like "1 of 2" and shares
// the first line with what is being asked. selected is approval.ChoiceYes or
// approval.ChoiceNo while a button waits for its confirming tap.
type popupView struct {
	rec                            approval.Record
	queue, typed, notice, selected string
}

// popupScreen returns the popup's lines for a cols x rows terminal and the rows
// where a tap answers. docs/explanation/popup-and-phone-approval.md shows the
// result at Herdr's 37x14.
func popupScreen(v popupView, cols, rows int) ([]string, popupLayout) {
	if cols < 1 {
		cols = 1
	}
	if rows < 1 {
		rows = 1
	}
	summary := approval.Summary(v.rec)
	header := headerLine(visibleScreenText(summary[0]), v.queue, cols)
	if rows < 7 {
		lines := []string{header}
		for len(lines) < rows {
			lines = append(lines, "")
		}
		return lines, popupLayout{}
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
	return lines, popupLayout{approveTop, approveTop + buttonHeight - 1, declineTop, declineTop + buttonHeight - 1}
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

func drawPopup(in *os.File, out io.Writer, v popupView) popupLayout {
	cols, rows := terminalSize(in)
	lines, layout := popupScreen(v, cols, rows)
	fmt.Fprint(out, "\x1b[2J\x1b[H"+mouseReportingOn)
	fmt.Fprint(out, strings.Join(lines, "\r\n"))
	return layout
}

// An impatient second tap would otherwise answer the request that just
// replaced the one it was meant for. Herdr also opens the first popup with
// focus while the user may be typing or tapping elsewhere.
const popupSettle = 600 * time.Millisecond

const settleNotice = "one moment..."

// askPopup draws the popup and returns "yes" or "no" for a confirmed tap on a
// button or a typed yes or no and Enter, "dismiss" for q or Esc, or "focus"
// for g. Input typed before the popup is drawn is thrown away. The program
// asks Herdr to put the keyboard on the popup when it opens, so keys meant for
// another terminal could otherwise answer. settle ignores taps and Enter for
// that long after the popup opens; the queue passes popupSettle for every
// request.
func askPopup(in *os.File, out io.Writer, rec approval.Record, queue string, settle time.Duration) (string, error) {
	old, err := setRaw(in)
	if err != nil {
		return "", err
	}
	defer restore(in, old)
	defer fmt.Fprint(out, mouseReportingOff)
	// Runs for each queued request too, so input left from the previous answer
	// is dropped.
	if err := flushInput(in); err != nil {
		return "", err
	}
	v := &popupView{rec: rec, queue: queue}
	if settle > 0 {
		v.notice = settleNotice
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
			if !closed && v.notice == settleNotice {
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

// handle applies one key or tap to the view. It returns the popup's choice, or
// approval.ChoiceNone with redraw set when the view changed. settling ignores
// taps and Enter.
func (v *popupView) handle(in popupInput, layout popupLayout, settling bool) (choice string, redraw bool) {
	switch b := in.b; {
	case b == keyEscape:
		if !in.isMouse {
			return approval.ChoiceDismiss, false
		}
		// A tap also sends a release. Counting it would confirm the button the
		// same tap just selected.
		if !in.mouse.press || in.mouse.button != 0 || settling {
			return approval.ChoiceNone, false
		}
		// A tap only selects a button and a second tap on it answers, because
		// accidental phone taps decided requests.
		hit := layout.hit(in.mouse.x, in.mouse.y)
		if hit != approval.ChoiceNone && hit == v.selected {
			return hit, false
		}
		v.selected, v.notice = hit, ""
		if hit == approval.ChoiceYes {
			v.notice = "tap APPROVE again to confirm"
		} else if hit == approval.ChoiceNo {
			v.notice = "tap DECLINE again to confirm"
		}
		return approval.ChoiceNone, true
	case b == '\r' || b == '\n':
		if settling {
			return approval.ChoiceNone, false
		}
		typed := strings.ToLower(v.typed)
		if typed == approval.ChoiceYes || typed == approval.ChoiceNo {
			return typed, false
		}
		v.typed, v.selected = "", ""
		v.notice = "Type yes or no, then Enter"
		return approval.ChoiceNone, true
	case b == 'q':
		return approval.ChoiceDismiss, false
	case b == 'g':
		return approval.ChoiceFocus, false
	case b == keyBackspace || b == keyDelete:
		if len(v.typed) > 0 {
			v.typed = v.typed[:len(v.typed)-1]
		}
		v.notice, v.selected = keyNotice(settling), ""
		return approval.ChoiceNone, true
	case b >= ' ' && b <= '~': // Printable ASCII.
		v.typed += string(b)
		// yes and no need only the last few keys; the cap keeps the Typed line short.
		if len(v.typed) > 8 {
			v.typed = v.typed[len(v.typed)-8:]
		}
		v.notice, v.selected = keyNotice(settling), ""
		return approval.ChoiceNone, true
	}
	return approval.ChoiceNone, false
}

// A typed key clears other notices, but the pause notice stays until the
// pause ends, or Enter after a fast "yes" would do nothing with no hint why.
func keyNotice(settling bool) string {
	if settling {
		return settleNotice
	}
	return ""
}

// One key, or an escape sequence and its parsed mouse report.
type popupInput struct {
	b       byte
	mouse   mouseReport
	isMouse bool
	err     error
}

// Sends one input for each request on want and stops when want closes.
func readPopupInput(in *os.File, want <-chan struct{}, inputs chan<- popupInput) {
	reader := bufio.NewReader(in)
	for range want {
		var input popupInput
		input.b, input.err = reader.ReadByte()
		if input.err == nil && input.b == keyEscape {
			input.mouse, input.isMouse, input.err = readMouse(reader)
		}
		inputs <- input
	}
}

type mouseReport struct {
	button, x, y int
	press        bool
}

func readMouse(r *bufio.Reader) (mouseReport, bool, error) {
	var result mouseReport
	// A terminal sends a whole tap report at once, so an Esc with nothing
	// buffered behind it is the Esc key.
	if r.Buffered() == 0 {
		return result, false, nil
	}
	peek, err := r.Peek(1)
	if err != nil {
		return result, false, err
	}
	if peek[0] != '[' {
		return result, false, nil
	}
	if _, err := r.ReadByte(); err != nil {
		return result, false, err
	}
	if a, err := r.ReadByte(); err != nil || a != '<' {
		return result, false, io.ErrUnexpectedEOF
	}
	var text []byte
	for {
		b, err := r.ReadByte()
		if err != nil {
			return result, false, err
		}
		if b == 'M' || b == 'm' {
			parts := strings.Split(string(text), ";")
			if len(parts) != 3 {
				return result, false, io.ErrUnexpectedEOF
			}
			_, _ = fmt.Sscanf(strings.Join(parts, " "), "%d %d %d", &result.button, &result.x, &result.y)
			result.press = b == 'M'
			return result, true, nil
		}
		text = append(text, b)
	}
}

func showDetails(rec approval.Record) {
	tty, err := os.OpenFile("/dev/tty", os.O_RDWR, 0)
	if err != nil {
		return
	}
	defer tty.Close()
	_ = showPager(tty, rec.ApprovalText)
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
