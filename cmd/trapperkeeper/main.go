package main

import (
	"os"

	"github.com/solatis/trapperkeeper/cmd/trapperkeeper/cmd"
)

func main() {
	if err := cmd.Execute(); err != nil {
		os.Exit(1)
	}
}
