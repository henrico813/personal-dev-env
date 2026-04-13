// Package replace provides splice-based replacement for implementation sections
// in canonical planner-rendered markdown. It preserves untouched content
// byte-for-byte while replacing only targeted scopes.
package replace

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"

	"planner/inspect"
	"planner/render"
	"planner/schema"
	"planner/validate"
)

// Contract describes what was replaced in a successful Run call.
type Contract struct {
	Section       string `json:"section"`
	StepsReplaced []int  `json:"steps_replaced"`
}

// Run replaces the specified section in sourcePath with the patch from patchPath,
// writing the result to outputPath. Only implementation and implementation.N
// sections are supported in v1.
func Run(sourcePath string, section string, patchPath string, outputPath string) (Contract, error) {
	sourceRaw, err := os.ReadFile(sourcePath)
	if err != nil {
		return Contract{}, err
	}

	plan, sectionSpans, stepSpans, err := inspect.ParseMarkdown(string(sourceRaw))
	if err != nil {
		return Contract{}, err
	}

	patchRaw, err := os.ReadFile(patchPath)
	if err != nil {
		return Contract{}, err
	}

	updated := plan
	stepsReplaced := []int{}
	if section == "implementation" {
		var steps []schema.Step
		if err := decodeStrictJSON(patchRaw, &steps); err != nil {
			return Contract{}, err
		}
		updated.Implementation = steps
		stepsReplaced = make([]int, 0, len(steps))
		for i := range steps {
			stepsReplaced = append(stepsReplaced, i+1)
		}
	} else if strings.HasPrefix(section, "implementation.") {
		idx, err := strconv.Atoi(strings.TrimPrefix(section, "implementation."))
		if err != nil {
			return Contract{}, fmt.Errorf("invalid section %q", section)
		}
		if idx < 1 || idx > len(updated.Implementation) {
			return Contract{}, fmt.Errorf("step index out of range: %d", idx)
		}
		var step schema.Step
		if err := decodeStrictJSON(patchRaw, &step); err != nil {
			return Contract{}, err
		}
		updated.Implementation[idx-1] = step
		stepsReplaced = []int{idx}
	} else {
		return Contract{}, fmt.Errorf("replace supports only implementation and implementation.N")
	}

	if err := validate.ValidatePlan(updated); err != nil {
		return Contract{}, err
	}

	replacementSection, replacementSteps, err := renderReplacement(updated)
	if err != nil {
		return Contract{}, err
	}

	out := string(sourceRaw)
	if section == "implementation" {
		out = splice(out, sectionSpans.Implementation, replacementSection)
	} else {
		idx := stepsReplaced[0] - 1
		out = splice(out, stepSpans[idx], replacementSteps[idx])
	}

	_, newSectionSpans, newStepSpans, err := inspect.ParseMarkdown(out)
	if err != nil {
		return Contract{}, err
	}

	if err := assertPreserved(string(sourceRaw), out, section, sectionSpans, newSectionSpans, stepSpans, newStepSpans); err != nil {
		return Contract{}, err
	}
	if err := writeAtomic(outputPath, []byte(out)); err != nil {
		return Contract{}, err
	}

	return Contract{
		Section:       section,
		StepsReplaced: stepsReplaced,
	}, nil
}

func decodeStrictJSON(raw []byte, target any) error {
	dec := json.NewDecoder(bytes.NewReader(raw))
	dec.DisallowUnknownFields()
	if err := dec.Decode(target); err != nil {
		return fmt.Errorf("decode patch: %w", err)
	}
	return nil
}

func renderReplacement(plan schema.Plan) (string, []string, error) {
	replacementSection, err := render.RenderImplementationSection(plan.Implementation)
	if err != nil {
		return "", nil, err
	}
	steps := make([]string, 0, len(plan.Implementation))
	for i, step := range plan.Implementation {
		renderedStep, err := render.RenderImplementationStep(i+1, step)
		if err != nil {
			return "", nil, err
		}
		steps = append(steps, renderedStep)
	}
	return replacementSection, steps, nil
}

func splice(raw string, span inspect.Span, replacement string) string {
	return raw[:span.Start] + replacement + raw[span.End:]
}

func assertPreserved(beforeRaw string, afterRaw string, section string, beforeSections inspect.SectionSpans, afterSections inspect.SectionSpans, beforeSteps []inspect.Span, afterSteps []inspect.Span) error {
	if rawAt(beforeRaw, beforeSections.Overview) != rawAt(afterRaw, afterSections.Overview) {
		return fmt.Errorf("overview changed unexpectedly")
	}
	if rawAt(beforeRaw, beforeSections.DefinitionOfDone) != rawAt(afterRaw, afterSections.DefinitionOfDone) {
		return fmt.Errorf("definition_of_done changed unexpectedly")
	}
	if rawAt(beforeRaw, beforeSections.Verification) != rawAt(afterRaw, afterSections.Verification) {
		return fmt.Errorf("verification changed unexpectedly")
	}

	if strings.HasPrefix(section, "implementation.") {
		idx, _ := strconv.Atoi(strings.TrimPrefix(section, "implementation."))
		if len(beforeSteps) != len(afterSteps) {
			return fmt.Errorf("implementation step count changed unexpectedly")
		}
		for i := range beforeSteps {
			if i == idx-1 {
				continue
			}
			if rawAt(beforeRaw, beforeSteps[i]) != rawAt(afterRaw, afterSteps[i]) {
				return fmt.Errorf("implementation.%d changed unexpectedly", i+1)
			}
		}
	}

	return nil
}

func rawAt(raw string, span inspect.Span) string {
	if span.Start < 0 || span.End > len(raw) || span.Start >= span.End {
		return ""
	}
	return raw[span.Start:span.End]
}

func writeAtomic(path string, data []byte) error {
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}
	tmp, err := os.CreateTemp(filepath.Dir(path), ".planner-replace-*.tmp")
	if err != nil {
		return err
	}
	tmpPath := tmp.Name()
	if _, err := tmp.Write(data); err != nil {
		_ = tmp.Close()
		_ = os.Remove(tmpPath)
		return err
	}
	if err := tmp.Close(); err != nil {
		_ = os.Remove(tmpPath)
		return err
	}
	if err := os.Rename(tmpPath, path); err != nil {
		_ = os.Remove(tmpPath)
		return err
	}
	return nil
}
