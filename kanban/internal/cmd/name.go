package cmd

import (
	"fmt"
	"io"
	"strings"

	"github.com/spf13/cobra"

	"kanban/internal/card"
)

// Builds "card name RULE [VALUE...]", which needs no vault.
func newNameCommand(e *env) *cobra.Command {
	return &cobra.Command{
		Use:   "name RULE [VALUE...]",
		Short: "make a name from a naming rule",
		Long: "Make a name from a naming rule in names.toml. VALUEs fill the rule's\n" +
			"fields in order, and the last field takes the rest.",
		Example: "  card name branch feat retry uploads\n  card name agent DEMO-012 build",
		Args: func(_ *cobra.Command, args []string) error {
			if len(args) == 0 {
				return usageError{msg: "the following arguments are required: rule"}
			}
			return nil
		},
		RunE: func(command *cobra.Command, args []string) error {
			return runName(e, command.OutOrStdout(), args[0], args[1:])
		},
	}
}

// Prints the name a rule makes from values, such as feat/retry-uploads for
// the branch rule and "feat retry uploads". It refuses a name the rule's
// own pattern would not read back, such as one with an unknown branch type.
func runName(e *env, out io.Writer, ruleName string, values []string) error {
	rules, err := loadNameRules(e)
	if err != nil {
		return err
	}
	rule, err := rules.FindNameRule(ruleName)
	if err != nil {
		return err
	}
	fields := rule.ListFields()
	if len(values) < len(fields) {
		return fmt.Errorf("card name %s needs %s", rule.Name, strings.ToUpper(strings.Join(fields, " ")))
	}
	filled := map[string]string{}
	for i, field := range fields {
		filled[field] = values[i]
	}
	if len(fields) > 0 {
		filled[fields[len(fields)-1]] = strings.Join(values[len(fields)-1:], " ")
	}
	name := card.MakeName(rule, filled)
	if _, ok := card.ParseName(rule, name); !ok {
		return fmt.Errorf("%s does not fit the %s rule %s", name, rule.Name, rule.Make)
	}
	_, err = fmt.Fprintln(out, name)
	return err
}
