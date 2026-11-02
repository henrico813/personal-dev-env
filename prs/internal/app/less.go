package app

import (
	"errors"
	"os"
	"os/exec"
	"strings"
	"unicode/utf8"
)

// The lesskey lines, in less's key file format, that every less screen
// shares. Esc cancels a search, at the cost of the arrow keys there, which
// start with Esc.
const lessKeys = `#line-edit
\e abort
#command
h quit h
q quit q
l noaction
J forw-scroll
K back-scroll
`

// Shows text in less with prompt as its help text row, and returns the
// key that closed it, or 'q' for less's own ways out such as ZZ. keys holds
// lesskey lines such as "c quit c", whose letter's code is the exit status.
func (a *prsApp) openInLess(
	prompt,
	text,
	keys string,
) byte {
	// The key file goes in on descriptor 3, so prs writes no file for it.
	keysRead, keysWrite, err := os.Pipe()
	if err != nil {
		a.printError("prs: " + err.Error())
		return 'q'
	}
	defer func() { _ = keysRead.Close() }()
	_, _ = keysWrite.WriteString(lessKeys + keys)
	_ = keysWrite.Close()
	cmd := exec.Command(
		"less",
		"-R",
		"-x2",
		"-c",
		"--mouse",
		"--wheel-lines=3",
		"--lesskey-src=/dev/fd/3",
		"-Ps"+escapePrompt(prompt),
	)
	cmd.ExtraFiles = []*os.File{keysRead}
	cmd.Stdin = strings.NewReader(text)
	cmd.Stdout = a.stdout
	cmd.Stderr = a.stderr
	err = cmd.Run()
	var exit *exec.ExitError
	if errors.As(
		err,
		&exit,
	) {
		if exit.ExitCode() > 0 && exit.ExitCode() < 128 {
			return byte(exit.ExitCode())
		}
		return 'q'
	}
	if err != nil {
		a.printError("prs: less failed: " + err.Error())
		a.waitForKey("Could not show this screen.")
	}
	return 'q'
}

// Returns the help text row for a less screen width columns wide. It keeps
// the long keys when they fit, adding the title, or just its first word
// such as 1/9, when there is room.
func fitLessPrompt(
	width int,
	title,
	long,
	short string,
) string {
	count, _, _ := strings.Cut(
		title,
		" ",
	)
	for _, prompt := range []string{
		long + "  " + title,
		long + "  " + count,
		long,
		short + "  " + title,
	} {
		// less shifts a prompt that reaches the last 2 columns.
		if utf8.RuneCountInString(prompt) <= width-2 {
			return prompt
		}
	}
	return short
}

// Returns title with a backslash before each character less reads as a
// prompt code. Without it, a file name such as edit.go loses its dot.
func escapePrompt(title string) string {
	var out strings.Builder
	for _, r := range title {
		if strings.ContainsRune(
			`?:.%\`,
			r,
		) {
			out.WriteByte('\\')
		}
		out.WriteRune(r)
	}
	return out.String()
}
