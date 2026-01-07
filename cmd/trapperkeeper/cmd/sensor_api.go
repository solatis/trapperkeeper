package cmd

import (
	"context"
	"database/sql"
	"fmt"
	"log"
	"os"
	"os/signal"
	"syscall"

	"github.com/solatis/trapperkeeper/internal/core/api"
	"github.com/solatis/trapperkeeper/internal/core/auth"
	"github.com/solatis/trapperkeeper/internal/core/config"
	"github.com/solatis/trapperkeeper/internal/core/db"
	"github.com/solatis/trapperkeeper/internal/core/server"
	"github.com/solatis/trapperkeeper/internal/rules"
	"github.com/spf13/cobra"
)

const Version = "0.1.0"

var sensorAPICmd = &cobra.Command{
	Use:   "sensor-api",
	Short: "Start gRPC sensor API service",
	RunE:  runSensorAPI,
}

func init() {
	rootCmd.AddCommand(sensorAPICmd)
	sensorAPICmd.Flags().String("host", "0.0.0.0", "gRPC server host")
	sensorAPICmd.Flags().Int("port", 50051, "gRPC server port")
}

func runSensorAPI(cmd *cobra.Command, args []string) error {
	ctx := context.Background()

	cfg, err := config.LoadConfig(configFile)
	if err != nil {
		return fmt.Errorf("failed to load config: %w", err)
	}

	if cmd.Flags().Changed("host") {
		host, _ := cmd.Flags().GetString("host")
		cfg.Host = host
	}
	if cmd.Flags().Changed("port") {
		port, _ := cmd.Flags().GetInt("port")
		cfg.Port = port
	}

	if dbURL == "" {
		return fmt.Errorf("--db-url required")
	}
	database, err := db.Open(dbURL)
	if err != nil {
		return fmt.Errorf("failed to open database: %w", err)
	}
	defer database.Close()

	var migrationID string
	checkQuery := `SELECT migration_id FROM migrations WHERE migration_id = '003_hmac_api_keys.sql'`
	err = database.Get(&migrationID, database.Rebind(checkQuery))
	if err != nil {
		if err == sql.ErrNoRows {
			return fmt.Errorf("migration 003_hmac_api_keys not applied - run 'trapperkeeper migrate' first")
		}
		return fmt.Errorf("failed to check migrations: %w", err)
	}

	queries, err := db.LoadQueries(database)
	if err != nil {
		return fmt.Errorf("failed to load queries: %w", err)
	}

	secrets, err := config.HMACSecrets()
	if err != nil {
		return fmt.Errorf("failed to load HMAC secrets: %w", err)
	}
	if len(secrets) == 0 {
		return fmt.Errorf("no HMAC secrets configured (set TK_HMAC_SECRET environment variable)")
	}

	authenticator := auth.NewAuthenticator(secrets, queries)

	rulesEngine := rules.NewEngine()

	service, err := api.NewSensorAPIService(database, rulesEngine, cfg)
	if err != nil {
		return fmt.Errorf("failed to create service: %w", err)
	}

	grpcServer, err := server.NewGRPCServer(cfg, service, authenticator)
	if err != nil {
		return fmt.Errorf("failed to create server: %w", err)
	}

	log.Printf("Starting TrapperKeeper sensor API v%s on %s:%d", Version, cfg.Host, cfg.Port)
	errChan := make(chan error, 1)
	go func() {
		errChan <- grpcServer.Start(ctx)
	}()

	sigChan := make(chan os.Signal, 1)
	signal.Notify(sigChan, syscall.SIGINT, syscall.SIGTERM)

	select {
	case err := <-errChan:
		return err
	case <-sigChan:
		log.Println("Shutting down gracefully...")
		return grpcServer.Shutdown(ctx)
	}
}
