package terminal

import (
	"testing"

	"pdev2/internal/approval"
)

// The tap rules decide whether a stray tap answers a request. Accidental phone
// taps decided two requests in live use, so one tap only selects a button and
// a second tap on the same button answers. A tap's release is not a second
// tap. For 600 ms after each request appears, taps and Enter are ignored and
// "one moment..." stays shown, even while keys are typed, so a fast "yes"
// and Enter that does nothing still says why.
func TestPopupAppliesTapAndSettleRules(t *testing.T) {
	_, layout := PopupScreen(View{Rec: approval.Record{ID: "id"}}, 37, 14)
	press := popupInput{b: keyEscape, isMouse: true, mouse: mouseReport{x: 10, y: layout.ApproveTop, press: true}}
	release := popupInput{b: keyEscape, isMouse: true, mouse: mouseReport{x: 10, y: layout.ApproveTop}}
	declinePress := popupInput{b: keyEscape, isMouse: true, mouse: mouseReport{x: 10, y: layout.DeclineTop, press: true}}
	typedYes := []popupInput{{b: 'y'}, {b: 'e'}, {b: 's'}, {b: '\r'}}
	// "yess", Backspace, then Enter, so both editing keys run during the pause.
	correctedYes := []popupInput{{b: 'y'}, {b: 'e'}, {b: 's'}, {b: 's'}, {b: keyBackspace}, {b: '\r'}}
	cases := []struct {
		name     string
		settling bool
		inputs   []popupInput
		want     string // The choice after the last input; earlier inputs must give none.
	}{
		{name: "second tap answers", inputs: []popupInput{press, press}, want: approval.ChoiceYes},
		{name: "switch to decline", inputs: []popupInput{press, declinePress, declinePress}, want: approval.ChoiceNo},
		{name: "release does not confirm", inputs: []popupInput{press, release}, want: approval.ChoiceNone},
		{name: "settling ignores taps", settling: true, inputs: []popupInput{press, press}, want: approval.ChoiceNone},
		{name: "Enter answers typed yes", inputs: typedYes, want: approval.ChoiceYes},
		{name: "settling ignores Enter", settling: true, inputs: correctedYes, want: approval.ChoiceNone},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			// AskPopup shows the pause notice when the popup opens.
			v := &View{}
			if tc.settling {
				v.notice = SettleNotice
			}

			got := approval.ChoiceNone
			for i, input := range tc.inputs {
				if got != approval.ChoiceNone {
					t.Fatalf("answered %q before input %d", got, i)
				}
				got, _ = v.handle(input, layout, tc.settling)
				if tc.settling && v.notice != SettleNotice {
					t.Fatalf("notice after input %d = %q, want %q", i, v.notice, SettleNotice)
				}
			}

			if got != tc.want {
				t.Fatalf("choice = %q, want %q", got, tc.want)
			}
		})
	}
}
