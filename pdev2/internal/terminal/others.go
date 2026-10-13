package terminal

import (
	"io"
	"os"
	"os/signal"
	"syscall"
)

// ShowOtherMachines is the popup when no approval waits on this machine. It
// says it is checking, shows the lines check returns, and returns when the
// user closes it with q, Esc, Enter, or a tap. Herdr shrinks the popup to fit
// a smaller client, so it redraws when its size changes.
func ShowOtherMachines(in *os.File, out io.Writer, check func() []string) error {
	_, leave, err := enterPopup(in, out)
	if err != nil {
		return err
	}
	defer leave()
	// Registered before the first draw, so a resize right after it is not lost.
	resize := make(chan os.Signal, 1)
	signal.Notify(resize, syscall.SIGWINCH)
	defer signal.Stop(resize)
	var lines []string
	checking := true
	draw := func() {
		cols, rows := terminalSize(in)
		drawLines(out, otherMachinesScreen(lines, checking, cols, rows))
	}
	draw()
	// The check takes seconds; a tap must still close the popup meanwhile.
	result := make(chan []string, 1)
	go func() { result <- check() }()
	want := make(chan struct{})
	inputs := make(chan popupInput)
	defer close(want)
	go readPopupInput(in, want, inputs)
	want <- struct{}{}
	for {
		select {
		case lines = <-result:
			checking = false
			draw()
		case <-resize:
			draw()
		case input := <-inputs:
			if input.err != nil {
				return input.err
			}
			if closesScreen(input) {
				return nil
			}
			want <- struct{}{}
		}
	}
}

func closesScreen(in popupInput) bool {
	if in.isMouse {
		return in.mouse.press && in.mouse.button == 0
	}
	return in.b == 'q' || in.b == keyEscape || in.b == '\r' || in.b == '\n'
}

func otherMachinesScreen(lines []string, checking bool, cols, rows int) []string {
	screen := []string{"No approvals waiting here."}
	switch {
	case checking:
		screen = append(screen, "Checking other machines...")
	case len(lines) == 0:
		screen = append(screen, "No other machines found.")
	default:
		for _, line := range lines {
			screen = append(screen, visibleScreenText(line))
		}
		screen = append(screen, "", "To answer, switch to that machine", "in Herdr, then press Ctrl+b a.")
	}
	// The switching steps, then machine lines, give way, so the close hint
	// always shows.
	if room := max(rows-2, 1); len(screen) > room {
		screen = screen[:room]
	}
	screen = append(screen, "", "q, Esc, or tap: close")
	for i := range screen {
		screen[i] = clip(screen[i], max(cols, 1))
	}
	return screen[:min(len(screen), max(rows, 1))]
}
