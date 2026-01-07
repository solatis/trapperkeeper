package db

import (
	"crypto/sha256"
	"embed"
	"fmt"
	"io/fs"
	"path/filepath"
	"sort"
	"strings"
	"time"

	"github.com/jmoiron/sqlx"
	embeddedmigrations "github.com/solatis/trapperkeeper/migrations"
)

// MigrationStatus represents the state of a single migration.
type MigrationStatus struct {
	ID          string
	Checksum    string
	Applied     bool
	AppliedAt   *time.Time
	ExecutionMs int64
}

// MigrateUp runs all pending migrations against the database.
// Detects driver type, selects appropriate embedded migrations,
// validates checksums, and applies pending migrations in order.
func MigrateUp(db *sqlx.DB) error {
	driver := db.DriverName()

	var migrationsFS embed.FS
	var migrationsDir string

	switch driver {
	case "sqlite3":
		migrationsFS = embeddedmigrations.SqliteMigrations
		migrationsDir = "sqlite"
	case "postgres":
		migrationsFS = embeddedmigrations.PostgresMigrations
		migrationsDir = "postgres"
	default:
		return fmt.Errorf("unsupported database driver: %s", driver)
	}

	// Create migrations tracking table if not exists
	if err := createMigrationsTable(db); err != nil {
		return fmt.Errorf("failed to create migrations table: %w", err)
	}

	// Parse migration files from embedded filesystem
	migrations, err := parseMigrationFiles(migrationsFS, migrationsDir)
	if err != nil {
		return fmt.Errorf("failed to parse migrations: %w", err)
	}

	// Validate checksums of already-applied migrations
	// SHA256 hash detects unauthorized modification of applied migrations
	if err := validateChecksums(db, migrations); err != nil {
		return fmt.Errorf("migration checksum validation failed: %w", err)
	}

	// Get list of applied migration IDs
	applied, err := getAppliedMigrations(db)
	if err != nil {
		return fmt.Errorf("failed to query applied migrations: %w", err)
	}

	// Apply pending migrations in order
	for _, m := range migrations {
		if applied[m.ID] {
			continue
		}

		start := time.Now()

		// Wrap migration execution and recording in transaction for atomicity
		// If migration succeeds but recording fails, rollback prevents partial state
		tx, err := db.Beginx()
		if err != nil {
			return fmt.Errorf("failed to begin transaction for migration %s: %w", m.ID, err)
		}

		if err := applyMigration(tx, m); err != nil {
			tx.Rollback()
			return fmt.Errorf("failed to apply migration %s: %w", m.ID, err)
		}

		duration := time.Since(start)

		// Record migration metadata for audit trail
		if err := recordMigration(tx, m.ID, m.Checksum, duration); err != nil {
			tx.Rollback()
			return fmt.Errorf("failed to record migration %s: %w", m.ID, err)
		}

		if err := tx.Commit(); err != nil {
			return fmt.Errorf("failed to commit migration %s: %w", m.ID, err)
		}
	}

	return nil
}

// MigrateStatus returns the status of all migrations (applied and pending).
func MigrateStatus(db *sqlx.DB) ([]MigrationStatus, error) {
	driver := db.DriverName()

	var migrationsFS embed.FS
	var migrationsDir string

	switch driver {
	case "sqlite3":
		migrationsFS = embeddedmigrations.SqliteMigrations
		migrationsDir = "sqlite"
	case "postgres":
		migrationsFS = embeddedmigrations.PostgresMigrations
		migrationsDir = "postgres"
	default:
		return nil, fmt.Errorf("unsupported database driver: %s", driver)
	}

	// Create migrations tracking table if not exists
	if err := createMigrationsTable(db); err != nil {
		return nil, fmt.Errorf("failed to create migrations table: %w", err)
	}

	migrations, err := parseMigrationFiles(migrationsFS, migrationsDir)
	if err != nil {
		return nil, fmt.Errorf("failed to parse migrations: %w", err)
	}

	// Query applied migrations
	rows, err := db.Queryx("SELECT migration_id, checksum, applied_at, execution_ms FROM migrations")
	if err != nil {
		return nil, fmt.Errorf("failed to query migrations: %w", err)
	}
	defer rows.Close()

	applied := make(map[string]MigrationStatus)
	for rows.Next() {
		var status MigrationStatus
		if err := rows.Scan(&status.ID, &status.Checksum, &status.AppliedAt, &status.ExecutionMs); err != nil {
			return nil, err
		}
		status.Applied = true
		applied[status.ID] = status
	}

	// Build status list
	var statuses []MigrationStatus
	for _, m := range migrations {
		if s, ok := applied[m.ID]; ok {
			statuses = append(statuses, s)
		} else {
			statuses = append(statuses, MigrationStatus{
				ID:       m.ID,
				Checksum: m.Checksum,
				Applied:  false,
			})
		}
	}

	return statuses, nil
}

// migration represents a parsed migration file
type migration struct {
	ID       string
	Checksum string
	SQL      string
}

// parseMigrationFiles extracts ordered list of migrations from embed.FS
func parseMigrationFiles(fsys embed.FS, dir string) ([]migration, error) {
	var migrations []migration

	err := fs.WalkDir(fsys, dir, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if d.IsDir() || !strings.HasSuffix(path, ".sql") {
			return nil
		}

		content, err := fsys.ReadFile(path)
		if err != nil {
			return fmt.Errorf("failed to read %s: %w", path, err)
		}

		// SHA256 checksum for tamper detection
		hash := sha256.Sum256(content)
		checksum := fmt.Sprintf("%x", hash)

		migrations = append(migrations, migration{
			ID:       filepath.Base(path),
			Checksum: checksum,
			SQL:      string(content),
		})

		return nil
	})

	if err != nil {
		return nil, err
	}

	// Sort by filename for deterministic ordering
	sort.Slice(migrations, func(i, j int) bool {
		return migrations[i].ID < migrations[j].ID
	})

	return migrations, nil
}

// createMigrationsTable ensures migrations tracking table exists
// IMPORTANT: Schema must match migrations table definition in 001_initial_schema.sql
// If migration schema changes, update both locations
func createMigrationsTable(db *sqlx.DB) error {
	var createSQL string

	if db.DriverName() == "sqlite3" {
		createSQL = `
			CREATE TABLE IF NOT EXISTS migrations (
				migration_id TEXT PRIMARY KEY,
				checksum TEXT NOT NULL,
				applied_at TEXT NOT NULL,
				execution_ms INTEGER NOT NULL,
				CHECK (applied_at LIKE '____-__-__T__:__:__Z')
			)
		`
	} else {
		createSQL = `
			CREATE TABLE IF NOT EXISTS migrations (
				migration_id TEXT PRIMARY KEY,
				checksum TEXT NOT NULL,
				applied_at TIMESTAMP WITHOUT TIME ZONE NOT NULL,
				execution_ms INTEGER NOT NULL
			)
		`
	}

	_, err := db.Exec(createSQL)
	return err
}

// getAppliedMigrations returns a set of applied migration IDs
func getAppliedMigrations(db *sqlx.DB) (map[string]bool, error) {
	rows, err := db.Queryx("SELECT migration_id FROM migrations")
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	applied := make(map[string]bool)
	for rows.Next() {
		var id string
		if err := rows.Scan(&id); err != nil {
			return nil, err
		}
		applied[id] = true
	}

	return applied, nil
}

// validateChecksums verifies all applied migrations match embedded checksums
func validateChecksums(db *sqlx.DB, migrations []migration) error {
	rows, err := db.Queryx("SELECT migration_id, checksum FROM migrations")
	if err != nil {
		return err
	}
	defer rows.Close()

	checksumMap := make(map[string]string)
	for _, m := range migrations {
		checksumMap[m.ID] = m.Checksum
	}

	for rows.Next() {
		var id, dbChecksum string
		if err := rows.Scan(&id, &dbChecksum); err != nil {
			return err
		}

		expectedChecksum, ok := checksumMap[id]
		if !ok {
			return fmt.Errorf("migration %s exists in database but not in embedded files", id)
		}
		if dbChecksum != expectedChecksum {
			return fmt.Errorf("checksum mismatch for migration %s: expected %s, got %s", id, expectedChecksum, dbChecksum)
		}
	}

	return nil
}

// applyMigration executes a single migration SQL within a transaction
func applyMigration(tx *sqlx.Tx, m migration) error {
	// Split on semicolons for PostgreSQL compatibility
	// lib/pq doesn't support multiple statements in single Exec
	statements := strings.Split(m.SQL, ";")
	for _, stmt := range statements {
		stmt = strings.TrimSpace(stmt)
		if stmt == "" || strings.HasPrefix(stmt, "--") {
			continue
		}
		if _, err := tx.Exec(stmt); err != nil {
			return fmt.Errorf("statement failed: %w", err)
		}
	}
	return nil
}

// recordMigration stores migration metadata for audit trail within a transaction
func recordMigration(tx *sqlx.Tx, id, checksum string, duration time.Duration) error {
	now := time.Now().UTC()
	executionMs := duration.Milliseconds()

	if tx.DriverName() == "sqlite3" {
		_, err := tx.Exec(
			"INSERT INTO migrations (migration_id, checksum, applied_at, execution_ms) VALUES (?, ?, ?, ?)",
			id, checksum, now.Format(time.RFC3339), executionMs,
		)
		return err
	}

	_, err := tx.Exec(
		"INSERT INTO migrations (migration_id, checksum, applied_at, execution_ms) VALUES ($1, $2, $3, $4)",
		id, checksum, now, executionMs,
	)
	return err
}
