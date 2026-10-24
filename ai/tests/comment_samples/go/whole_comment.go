package config

const (
	DefaultHost = "localhost"
	// Used when the config file sets no port. Ports below 1024 need root
	// on Linux, so the default is above them.
	DefaultPort = 8080
)
