// Package cmd runs pde-gh-write, pde-pr-approve and the gh guard, and Main
// picks the mode from the program name and first argument. Every call to gh,
// herdr, and moshi-hook is made from this package.
package cmd
