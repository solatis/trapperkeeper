package cmd

import (
	"github.com/spf13/cobra"
)

var (
	configFile string
	dbURL      string
	logLevel   string
	logFormat  string
)

var rootCmd = &cobra.Command{
	Use:   "trapperkeeper",
	Short: "TrapperKeeper data quality rule engine",
	Long:  `TrapperKeeper provides real-time event evaluation with sub-millisecond per-event performance.`,
}

func init() {
	rootCmd.PersistentFlags().StringVar(&configFile, "config", "", "config file path")
	rootCmd.PersistentFlags().StringVar(&dbURL, "db-url", "", "database connection URL (sqlite://path or postgres://...)")
	rootCmd.PersistentFlags().StringVar(&logLevel, "log-level", "info", "log level (debug, info, warn, error)")
	rootCmd.PersistentFlags().StringVar(&logFormat, "log-format", "json", "log format (json, text)")
}

func Execute() error {
	return rootCmd.Execute()
}
