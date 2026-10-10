package main

import (
	"bufio"
	"fmt"
	"io"
	"os"
	"strings"
)

// Input typed before the question is discarded. EOF or a closed terminal is
// not an answer.
func askYesNo(tty *os.File) (string, error) {
	if err := flushInput(tty); err != nil {
		return "", err
	}
	fmt.Fprint(tty, "Approve this pull request? Type yes or no, then Enter: ")
	scanner := bufio.NewScanner(tty)
	for scanner.Scan() {
		choice := strings.ToLower(strings.TrimSpace(scanner.Text()))
		if choice == "yes" || choice == "no" {
			return choice, nil
		}
		fmt.Fprint(tty, "Please type yes or no, then Enter: ")
	}
	if err := scanner.Err(); err != nil {
		return "", err
	}
	return "", io.EOF
}
