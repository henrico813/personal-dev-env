package tmux

import (
	"archive/tar"
	"compress/gzip"
	"errors"
	"fmt"
	"io"
	"os"
	"path"
	"path/filepath"
	"strings"
)

const maxExtractedSize = 1024 * 1024 * 1024

func extractArchive(archive, destination, root string) (err error) {
	file, err := os.Open(archive)
	if err != nil {
		return err
	}
	defer func() { err = errors.Join(err, file.Close()) }()
	zipper, err := gzip.NewReader(file)
	if err != nil {
		return err
	}
	defer func() { err = errors.Join(err, zipper.Close()) }()
	if err := os.MkdirAll(destination, 0o755); err != nil {
		return err
	}

	reader := tar.NewReader(zipper)
	var extracted int64
	for {
		header, nextErr := reader.Next()
		if nextErr == io.EOF {
			return nil
		}
		if nextErr != nil {
			return nextErr
		}
		clean := path.Clean(header.Name)
		prefix := root + "/"
		if clean == root && header.Typeflag == tar.TypeDir {
			continue
		}
		if path.IsAbs(clean) || !strings.HasPrefix(clean, prefix) {
			return fmt.Errorf("unsafe archive path %q", header.Name)
		}
		relative := strings.TrimPrefix(clean, prefix)
		if relative == "" || relative == "." || relative == ".." || strings.HasPrefix(relative, "../") {
			return fmt.Errorf("unsafe archive path %q", header.Name)
		}
		target := filepath.Join(destination, filepath.FromSlash(relative))
		switch header.Typeflag {
		case tar.TypeDir:
			if err := os.MkdirAll(target, os.FileMode(header.Mode).Perm()); err != nil {
				return err
			}
		case tar.TypeReg, tar.TypeRegA:
			if header.Size < 0 || extracted > maxExtractedSize-header.Size {
				return fmt.Errorf("archive exceeds extracted size limit")
			}
			extracted += header.Size
			if err := os.MkdirAll(filepath.Dir(target), 0o755); err != nil {
				return err
			}
			output, err := os.OpenFile(target, os.O_CREATE|os.O_EXCL|os.O_WRONLY, os.FileMode(header.Mode).Perm())
			if err != nil {
				return err
			}
			_, copyErr := io.CopyN(output, reader, header.Size)
			closeErr := output.Close()
			if copyErr != nil {
				return copyErr
			}
			if closeErr != nil {
				return closeErr
			}
		default:
			return fmt.Errorf("unsupported archive entry %q", header.Name)
		}
	}
}
