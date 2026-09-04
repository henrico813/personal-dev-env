// Package fsutil provides guarded filesystem mutation helpers. Durable
// journals restore interrupted changes below HOME without overwriting paths
// changed by another process.
package fsutil
