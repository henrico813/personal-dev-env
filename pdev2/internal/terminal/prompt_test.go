package terminal

import (
	"bufio"
	"os"
	"testing"
	"time"

	"pdev2/internal/approvaltest"
)

// The old bash prompt approved on one stray key. Input typed before the question
// and a lone "y" must not decide.
func TestPromptNeedsDeliberateAnswer(t *testing.T) {
	cases := []struct{ name, typedEarly, typed, want string }{
		{name: "y is not yes", typed: "y\nno\n", want: "no"},
		{name: "typed early is discarded", typedEarly: "yes\n", typed: "no\n", want: "no"},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			// Writing to the pseudo-terminal's master side is typing into it.
			// Early input lands before the prompt starts and must be discarded.
			master, terminal := approvaltest.OpenPTY(t)
			if _, err := master.WriteString(tc.typedEarly); err != nil {
				t.Fatal(err)
			}
			result := make(chan string, 1)
			go func() { choice, _ := AskYesNo(terminal); result <- choice }()

			// Type the answer only after the question is printed; earlier
			// input would be discarded too.
			waitForQuestion(t, master)
			if _, err := master.WriteString(tc.typed); err != nil {
				t.Fatal(err)
			}

			select {
			case got := <-result:
				if got != tc.want {
					t.Errorf("answer = %q, want %q", got, tc.want)
				}
			case <-time.After(2 * time.Second):
				t.Fatal("prompt did not return")
			}
		})
	}
}

func waitForQuestion(t *testing.T, master *os.File) {
	t.Helper()
	asked := make(chan struct{})
	go func() {
		reader := bufio.NewReader(master)
		for {
			char, err := reader.ReadByte()
			if err != nil || char == '?' {
				close(asked)
				return
			}
		}
	}()
	select {
	case <-asked:
	case <-time.After(2 * time.Second):
		t.Fatal("prompt never asked")
	}
}
