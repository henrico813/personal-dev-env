package terminal

import (
	"bufio"
	"fmt"
	"io"
	"os"
	"strings"

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

// handle applies one key or tap to the view. It returns the popup's choice, or
// approval.ChoiceNone with redraw set when the view changed. settling ignores
// taps and Enter.
func (v *View) handle(in popupInput, layout Layout, settling bool) (choice string, redraw bool) {
	switch b := in.b; {
	case b == keyEscape:
		// Esc also interrupts an OpenCode agent, and the popup takes the
		// keyboard when it opens, so an Esc meant for the agent dismissed a
		// request. Only q dismisses.
		if !in.isMouse {
			return approval.ChoiceNone, false
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
		return SettleNotice
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
	a, err := r.ReadByte()
	if err != nil {
		return result, false, err
	}
	// A key such as Up, ESC [ A, is read as Esc, which the popup ignores. As
	// an error it closed the popup, and Up is OpenCode's history key.
	if a != '<' {
		return result, false, nil
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
