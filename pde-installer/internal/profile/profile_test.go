package profile

import "testing"

func TestParseAcceptsSupportedProfiles(t *testing.T) {
	for _, selected := range []Profile{Full, Terminal} {
		t.Run(string(selected), func(t *testing.T) {
			got, err := Parse(string(selected))
			if err != nil || got != selected {
				t.Fatalf("Parse() = %q, %v", got, err)
			}
		})
	}
}

func TestProfilesReportValidity(t *testing.T) {
	tests := map[string]struct {
		profile Profile
		want    bool
	}{
		"full":     {profile: Full, want: true},
		"terminal": {profile: Terminal, want: true},
		"empty":    {},
		"unknown":  {profile: Profile("desktop")},
	}
	for name, test := range tests {
		t.Run(name, func(t *testing.T) {
			if got := test.profile.Valid(); got != test.want {
				t.Fatalf("Valid() = %t, want %t", got, test.want)
			}
		})
	}
}

func TestParseRejectsInvalidProfiles(t *testing.T) {
	for name, value := range map[string]string{"empty": "", "unknown": "desktop"} {
		t.Run(name, func(t *testing.T) {
			_, err := Parse(value)
			want := `invalid profile "` + value + `"; use full or terminal`
			if err == nil || err.Error() != want {
				t.Fatalf("Parse() error = %v, want %q", err, want)
			}
		})
	}
}
