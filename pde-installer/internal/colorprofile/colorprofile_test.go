package colorprofile

import "testing"

func TestParseAcceptsColorProfiles(t *testing.T) {
	for _, selected := range All() {
		t.Run(string(selected), func(t *testing.T) {
			got, err := Parse(string(selected))
			if err != nil || got != selected {
				t.Fatalf("Parse() = %q, %v", got, err)
			}
		})
	}
}

func TestParseRejectsInvalidColors(t *testing.T) {
	for name, value := range map[string]string{"empty": "", "unknown": "nord"} {
		t.Run(name, func(t *testing.T) {
			_, err := Parse(value)
			want := `invalid color profile "` + value + `"; use tokyo-night, everforest-dark, or gruvbox-dark`
			if err == nil || err.Error() != want {
				t.Fatalf("Parse() error = %v, want %q", err, want)
			}
		})
	}
}
