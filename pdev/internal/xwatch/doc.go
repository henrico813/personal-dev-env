// Package xwatch is the code of pde-x-watch, which sends a desktop notice when
// Xorg keeps most of a CPU core busy or one program holds thousands of X
// windows, as a leaking terminal did. Start with WatchDisplay and
// watcher.checkOnce; xres.go asks the X server how many windows each program
// holds.
package xwatch
