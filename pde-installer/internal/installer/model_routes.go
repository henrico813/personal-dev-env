package installer

import (
	"bytes"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"time"

	"pde-installer/internal/run"
)

const routeCheckLimit = 30 * time.Second

// Runs model-routing's `pde_route.py check`, which compares the routing table
// with `opencode models`. A failed or slow check only warns, because a missing
// model must not fail an install whose files are already in place.
func checkModelRoutes(home string, runner run.Runner, limit time.Duration) error {
	script := filepath.Join(home, ".agents", "skills", "model-routing", "scripts", "pde_route.py")
	// opencode installs to ~/.local/bin, which may not be on PATH yet.
	path := filepath.Join(home, ".local", "bin") + string(os.PathListSeparator) + os.Getenv("PATH")
	command := run.Command{
		Name:    "python3",
		Args:    []string{script, "check"},
		Env:     []string{"PATH=" + path},
		Timeout: limit,
	}
	if runner.DryRun {
		return runner.Plan("check that each model-routing job kind has a model", &command)
	}
	quiet := run.Runner{Stdout: io.Discard, Stderr: io.Discard}
	output, err := quiet.RunOutput("pde_route.py check", command)
	output = bytes.TrimSpace(output)
	if err != nil {
		fmt.Fprintf(runner.Err(), "warning: %v; some agent jobs may have no model\n", err)
		if len(output) > 0 {
			fmt.Fprintf(runner.Err(), "%s\n", output)
		}
		return nil
	}
	// Exit 0 with output lists models missing here, such as one a provider renamed.
	if len(output) > 0 {
		fmt.Fprintf(runner.Out(), "note: pde_route.py check found missing models\n%s\n", output)
	}
	return nil
}
