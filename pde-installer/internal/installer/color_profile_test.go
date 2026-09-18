package installer

import (
	"strings"
	"testing"

	"pde-installer/internal/colorprofile"
)

func TestColorProfileResolution(t *testing.T) {
	tests := map[string]struct {
		config, request, failure string
		want                     colorprofile.Profile
	}{
		"fresh default": {
			want: colorprofile.TokyoNight,
		},
		"saved profile": {
			config: `{"color_profile":"everforest-dark"}`,
			want:   colorprofile.EverforestDark,
		},
		"missing field": {
			config: `{"profile":"full"}`,
			want:   colorprofile.TokyoNight,
		},
		"explicit repairs saved": {
			config:  `{"color_profile":"nord"}`,
			request: "gruvbox-dark",
			want:    colorprofile.GruvboxDark,
		},
		"invalid saved profile": {
			config:  `{"color_profile":"nord"}`,
			failure: `invalid color profile "nord"`,
		},
	}
	for name, test := range tests {
		t.Run(name, func(t *testing.T) {
			home := t.TempDir()
			if test.config != "" {
				home = profileHome(t, test.config)
			}
			got, err := resolveColorProfile(home, test.request)
			if test.failure != "" {
				if err == nil || !strings.Contains(err.Error(), test.failure) {
					t.Fatalf("resolveColorProfile() error = %v", err)
				}
				return
			}
			if err != nil || got != test.want {
				t.Fatalf("resolveColorProfile() = %q, %v", got, err)
			}
		})
	}
}
