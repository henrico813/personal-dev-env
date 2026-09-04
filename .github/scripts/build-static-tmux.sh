#!/usr/bin/env bash
set -euo pipefail

arch="${1:?usage: build-static-tmux.sh ARCH OUTPUT}"
output="${2:?usage: build-static-tmux.sh ARCH OUTPUT}"
case "$arch" in
amd64 | arm64) ;;
*) exit 2 ;;
esac
mkdir -p "$output"
output="$(cd "$output" && pwd)"

docker run --rm --platform "linux/$arch" \
	-v "$output:/out" \
	alpine:3.22.1@sha256:4bcff63911fcb4448bd4fdacec207030997caf25e9bea4045fa6c8c44de311d1 \
	sh -euxc '
		apk add --no-cache \
			bison=3.8.2-r2 build-base=0.5-r3 ca-certificates=20260611-r0 \
			curl=8.14.1-r3 file=5.46-r2 gzip=1.14-r2 \
			libevent-dev=2.1.13-r0 libevent-static=2.1.13-r0 \
			musl-dev=1.2.5-r12 ncurses-dev=6.5_p20250503-r0 \
			ncurses-static=6.5_p20250503-r0 pax-utils=1.3.8-r1 \
			pkgconf=2.4.3-r0 tar=1.35-r3
		curl -fsSLO https://github.com/tmux/tmux/releases/download/3.6a/tmux-3.6a.tar.gz
		printf "%s  %s\n" \
			b6d8d9c76585db8ef5fa00d4931902fa4b8cbe8166f528f44fc403961a3f3759 \
			tmux-3.6a.tar.gz | sha256sum -c -
		tar -xzf tmux-3.6a.tar.gz
		cd tmux-3.6a
		CC=cc CFLAGS="-O2" LDFLAGS="-static" \
			PKG_CONFIG="pkg-config --static" ./configure --enable-static
		make -j"$(getconf _NPROCESSORS_ONLN)"
		strip tmux
		file tmux | grep -q "statically linked"
		# Reject dynamic dependencies before publishing.
		test -z "$(scanelf -nq tmux)"
		test "$(./tmux -V)" = "tmux 3.6a"
		./tmux -L pde-release -f /dev/null new-session -d
		test "$(./tmux -L pde-release display-message -p "#{version}")" = 3.6a
		./tmux -L pde-release kill-server
		stage=/tmp/pde-tmux-stage
		install -Dm755 tmux "$stage/tmux-3.6a/bin/tmux"
		install -Dm644 COPYING "$stage/tmux-3.6a/LICENSE"
		printf "%s\n" tmux-3.6a-pde.1 >"$stage/tmux-3.6a/.pde-release"
		{ cat /etc/alpine-release; apk info -v | LC_ALL=C sort; } > \
			"$stage/tmux-3.6a/BUILD-DEPENDENCIES"
		# Fix archive metadata so identical inputs produce identical archives.
		TZ=UTC tar --sort=name --mtime=@0 --owner=0 --group=0 \
			--numeric-owner -C "$stage" -cf - tmux-3.6a | gzip -n > \
			"/out/tmux-3.6a-linux-'"$arch"'.tar.gz"
	'
