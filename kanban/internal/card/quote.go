package card

import (
	"fmt"
	"strings"
	"unicode/utf16"
)

// Quotes text as Python's json.dumps does by default, with every non-ASCII
// character escaped, such as "Größe".
func QuoteASCII(text string) string {
	var quoted strings.Builder
	quoted.WriteByte('"')
	for _, r := range text {
		switch {
		case r == '"' || r == '\\':
			quoted.WriteString(`\` + string(r))
		case r == '\n':
			quoted.WriteString(`\n`)
		case r == '\r':
			quoted.WriteString(`\r`)
		case r == '\t':
			quoted.WriteString(`\t`)
		case r == '\b':
			quoted.WriteString(`\b`)
		case r == '\f':
			quoted.WriteString(`\f`)
		case r < 0x20 || (r >= 0x7f && r <= 0xffff):
			fmt.Fprintf(&quoted, `\u%04x`, r)
		case r > 0xffff:
			high, low := utf16.EncodeRune(r)
			fmt.Fprintf(&quoted, `\u%04x\u%04x`, high, low)
		default:
			quoted.WriteRune(r)
		}
	}
	quoted.WriteByte('"')
	return quoted.String()
}
