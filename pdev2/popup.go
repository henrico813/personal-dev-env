package main

import (
	"bufio"
	"fmt"
	"io"
	"os"
	"os/signal"
	"strings"
	"syscall"
	"unicode"
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
		return "yes"
	}
	if y >= l.declineTop && y <= l.declineBottom {
		return "no"
	}
	return ""
}

// popupScreen returns the popup's lines for a cols x rows terminal and the rows
// where a tap answers. docs/explanation/popup-and-phone-approval.md shows the
// result at Herdr's 37x14.
func popupScreen(rec record, typed, notice string, cols, rows int) ([]string, popupLayout) {
	if cols < 1 {
		cols = 1
	}
	if rows < 1 {
		rows = 1
	}
	if rows < 7 {
		lines := []string{clip("PDE approval "+rec.ID[:min(8, len(rec.ID))], cols)}
		for len(lines) < rows {
			lines = append(lines, "")
		}
		return lines, popupLayout{}
	}
	buttonHeight := 3
	if rows < 11 {
		buttonHeight = 1
	}
	// Fixed rows: header, Typed, notice, key hint, and the gap between buttons.
	requestRows := rows - (5 + buttonHeight*2)
	lines := []string{clip("PDE approval "+rec.ID[:min(8, len(rec.ID))], cols)}
	for _, text := range popupSummary(rec) {
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
	lines = append(lines, clip("Typed: "+typed, cols))
	lines = append(lines, clip(notice, cols))
	lines = append(lines, clip("v: full text; q/Esc: dismiss", cols))
	approveTop := len(lines) + 1
	lines = append(lines, buttonRows("APPROVE", cols, buttonHeight)...)
	lines = append(lines, "")
	declineTop := len(lines) + 1
	lines = append(lines, buttonRows("DECLINE", cols, buttonHeight)...)
	return lines, popupLayout{approveTop, approveTop + buttonHeight - 1, declineTop, declineTop + buttonHeight - 1}
}

// The popup has room for a few lines, so it shows what will run, the title,
// and the body. The full approval text starts with ID, time, and command lines
// that would fill the popup; v shows it.
func popupSummary(rec record) []string {
	operation, target := "", ""
	if len(rec.Args) > 1 {
		operation = rec.Args[1]
		target, _ = findTarget(operation, rec.Args[2:])
	}
	summary := []string{"gh pr " + operation}
	if target != "" {
		summary[0] += " " + target
	}
	if rec.Title != "" {
		summary = append(summary, "Title: "+rec.Title)
	}
	if operation == "edit" {
		summary = append(summary, editChanges(rec.ApprovalText)...)
	}
	if rec.Body != "" {
		summary = append(summary, strings.Split(rec.Body, "\n")...)
	}
	return summary
}

// editChanges returns the "-old" and "+new" lines of a pr edit. It stops at the
// blank line before the body, so Markdown bullets ("- item") in the body are
// not shown as removed lines.
func editChanges(approvalText string) []string {
	_, changes, found := strings.Cut(approvalText, editHeader)
	if !found {
		return nil
	}
	changes, _, _ = strings.Cut(changes, "\n\n")
	var lines []string
	for _, line := range strings.Split(changes, "\n") {
		if (strings.HasPrefix(line, "+") && !strings.HasPrefix(line, "+++")) ||
			(strings.HasPrefix(line, "-") && !strings.HasPrefix(line, "---")) {
			lines = append(lines, line)
		}
	}
	return lines
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

func buttonRow(label string, width int) string {
	if label == "" {
		return strings.Repeat(" ", width)
	}
	label = "[ " + label + " ]"
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

func drawPopup(in *os.File, out io.Writer, rec record, typed, notice string) popupLayout {
	cols, rows := terminalSize(in)
	lines, layout := popupScreen(rec, typed, notice, cols, rows)
	fmt.Fprint(out, "\x1b[2J\x1b[H"+mouseReportingOn)
	fmt.Fprint(out, strings.Join(lines, "\r\n"))
	return layout
}

// askPopup draws the popup and returns "yes" or "no" for a tap on a button or
// a typed yes or no and Enter, or "" when q or Esc closes it. Input typed
// before the popup is drawn is thrown away. The program asks Herdr to put the
// keyboard on the popup when it opens, so keys meant for another terminal
// could otherwise answer.
func askPopup(in *os.File, out io.Writer, rec record) (string, error) {
	old, err := setRaw(in)
	if err != nil {
		return "", err
	}
	defer restore(in, old)
	defer fmt.Fprint(out, mouseReportingOff)
	if err := flushInput(in); err != nil {
		return "", err
	}
	layout := drawPopup(in, out, rec, "", "")
	resize := make(chan os.Signal, 1)
	signal.Notify(resize, syscall.SIGWINCH)
	defer signal.Stop(resize)
	redraw := make(chan struct{}, 1)
	go func() {
		for range resize {
			select {
			case redraw <- struct{}{}:
			default:
			}
		}
	}()
	reader := bufio.NewReader(in)
	typed := ""
	notice := ""
	for {
		select {
		case <-redraw:
			layout = drawPopup(in, out, rec, typed, notice)
		default:
		}
		b, err := reader.ReadByte()
		if err != nil {
			return "", err
		}
		if b == keyEscape {
			seq, mouse, err := readMouse(reader)
			if err != nil {
				return "", err
			}
			if !mouse {
				return "", nil
			}
			if seq.button == 0 {
				if hit := layout.hit(seq.x, seq.y); hit != "" {
					return hit, nil
				}
			}
			continue
		}
		if b == 'v' {
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
			layout = drawPopup(in, out, rec, typed, notice)
			continue
		}
		if b == '\r' || b == '\n' {
			choice := strings.ToLower(typed)
			if choice == "yes" || choice == "no" {
				return choice, nil
			}
			typed = ""
			notice = "Type yes or no, then Enter"
			layout = drawPopup(in, out, rec, typed, notice)
			continue
		}
		if b == 'q' {
			return "", nil
		}
		if b == keyBackspace || b == keyDelete {
			if len(typed) > 0 {
				typed = typed[:len(typed)-1]
			}
			notice = ""
			layout = drawPopup(in, out, rec, typed, notice)
			continue
		}
		if b >= ' ' && b <= '~' { // Printable ASCII.
			typed += string(b)
			// yes and no need only the last few keys; the cap keeps the Typed line short.
			if len(typed) > 8 {
				typed = typed[len(typed)-8:]
			}
			notice = ""
			layout = drawPopup(in, out, rec, typed, notice)
		}
	}
}

type mouseReport struct{ button, x, y int }

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
			return result, true, nil
		}
		text = append(text, b)
	}
}

func showDetails(rec record) {
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
