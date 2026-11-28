package cmd

import (
	"bytes"
	"encoding/json"
	"io"
	"unicode/utf8"

	"kanban/internal/card"
)

// Most lines fit 60 columns, and board lines 40, as in the Python card.
const (
	lineWidth  = 60
	boardWidth = 40
)

// One card in JSON replies, with fields in the order scripts expect.
type cardReply struct {
	Key           string          `json:"key"`
	Title         string          `json:"title"`
	Stage         string          `json:"stage"`
	Kind          string          `json:"kind"`
	Project       string          `json:"project"`
	StageDate     string          `json:"stage_date"`
	Plans         []string        `json:"plans"`
	PRs           []string        `json:"prs"`
	Sessions      []string        `json:"sessions"`
	OpenQuestions []questionReply `json:"open_questions"`
}

// One Open Questions item in a cardReply. Asked prints null without a date.
type questionReply struct {
	Number int      `json:"number"`
	Asked  *string  `json:"asked"`
	Text   string   `json:"text"`
	Lines  []string `json:"lines"`
}

// One name and value of an orderedObject.
type namedValue struct {
	name  string
	value any
}

// A JSON object that keeps its names in the order they were added, which a
// Go map cannot, such as show --json's sections in card order.
type orderedObject []namedValue

// Fills a cardReply, with empty lists rather than nil ones.
func describeCard(c card.Card) cardReply {
	fields := card.ReadFields(c.Text)
	questions := []questionReply{}
	for _, question := range card.ListQuestions(c.Text) {
		questions = append(questions, questionReply(question))
	}
	return cardReply{
		Key:           c.CardKey,
		Title:         c.ReadTitle(),
		Stage:         c.ReadStage(),
		Kind:          c.ReadKind(),
		Project:       c.ReadProject(),
		StageDate:     c.ReadStageDate(),
		Plans:         fields["plans"].AsList(),
		PRs:           fields["prs"].AsList(),
		Sessions:      fields["sessions"].AsList(),
		OpenQuestions: questions,
	}
}

// Writes value with a two-space indent and no HTML escaping, as Python's
// json.dumps(indent=2, ensure_ascii=False) does.
func printJSON(w io.Writer, value any) error {
	var encoded bytes.Buffer
	encoder := json.NewEncoder(&encoded)
	encoder.SetEscapeHTML(false)
	encoder.SetIndent("", "  ")
	if err := encoder.Encode(value); err != nil {
		return err
	}
	_, err := w.Write(unescapeLineSeparators(encoded.Bytes()))
	return err
}

// Writes U+2028 and U+2029 raw, as Python's encoder does. Go's encoder
// escapes them even with HTML escaping off.
func unescapeLineSeparators(encoded []byte) []byte {
	out := make([]byte, 0, len(encoded))
	for i := 0; i < len(encoded); i++ {
		if encoded[i] != '\\' || i+1 == len(encoded) {
			out = append(out, encoded[i])
			continue
		}
		// An escape is a backslash and the next byte, so an escaped
		// backslash before "u2028" is left alone.
		switch escape := string(encoded[i:min(i+6, len(encoded))]); escape {
		case `\u2028`:
			out = utf8.AppendRune(out, '\u2028')
			i += 5
		case `\u2029`:
			out = utf8.AppendRune(out, '\u2029')
			i += 5
		default:
			out = append(out, encoded[i], encoded[i+1])
			i++
		}
	}
	return out
}

// Writes the object's names and values in order. printJSON indents the
// result with the rest of the reply.
func (o orderedObject) MarshalJSON() ([]byte, error) {
	var encoded bytes.Buffer
	inner := json.NewEncoder(&encoded)
	inner.SetEscapeHTML(false)
	encoded.WriteByte('{')
	for i, field := range o {
		if i > 0 {
			encoded.WriteByte(',')
		}
		if err := inner.Encode(field.name); err != nil {
			return nil, err
		}
		encoded.WriteByte(':')
		if err := inner.Encode(field.value); err != nil {
			return nil, err
		}
	}
	encoded.WriteByte('}')
	return encoded.Bytes(), nil
}

// Cuts text to width runes, ending in "…" when cut, as the Python card's fit
// does.
func fitLine(text string, width int) string {
	runes := []rune(text)
	if len(runes) <= width {
		return text
	}
	return string(runes[:width-1]) + "…"
}
