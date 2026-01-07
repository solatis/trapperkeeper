// Package db provides database connection management and migration support.
//
// Supports SQLite (development) and PostgreSQL (production) via sqlx for
// connection pooling and query helpers. Migration execution handled by
// custom migration runner using embedded SQL files (embed.FS).
package db

import (
	"fmt"
	"net/url"
	"time"

	"github.com/jmoiron/sqlx"
	_ "github.com/lib/pq"
	_ "github.com/mattn/go-sqlite3"
)

// Connection pool limits based on PostgreSQL defaults and expected instances
// 16 max open connections per instance (100 server max / ~6 instances)
// 4 idle connections balance resource usage vs reconnection latency
const (
	maxOpenConns    = 16
	maxIdleConns    = 4
	connMaxIdleTime = 5 * time.Minute
	connMaxLifetime = 30 * time.Minute
)

// Open establishes a database connection from a URL and configures connection pooling.
// Supported URL schemes: sqlite://, postgres://
// SQLite URLs: sqlite://path/to/file.db or sqlite:///absolute/path
// PostgreSQL URLs: postgres://user:pass@host:port/dbname?sslmode=disable
func Open(dbURL string) (*sqlx.DB, error) {
	u, err := url.Parse(dbURL)
	if err != nil {
		return nil, fmt.Errorf("invalid database URL: %w", err)
	}

	var driverName string
	var dataSource string

	switch u.Scheme {
	case "sqlite":
		driverName = "sqlite3"
		// Extract path from URL: sqlite://file.db uses host+path (relative),
		// sqlite:///absolute/path uses path-only (absolute with empty host)
		if u.Host != "" {
			dataSource = u.Host + u.Path
		} else {
			dataSource = u.Path
		}
	case "postgres":
		driverName = "postgres"
		dataSource = dbURL
	default:
		return nil, fmt.Errorf("unsupported database scheme: %s (expected sqlite or postgres)", u.Scheme)
	}

	db, err := sqlx.Open(driverName, dataSource)
	if err != nil {
		return nil, fmt.Errorf("failed to open database: %w", err)
	}

	// Connection pool configuration prevents resource exhaustion
	// 16 max open connections based on PostgreSQL max_connections (100) / expected instances
	// 5-minute idle timeout releases resources during quiet periods
	// 30-minute max lifetime prevents stale connections
	db.SetMaxOpenConns(maxOpenConns)
	db.SetMaxIdleConns(maxIdleConns)
	db.SetConnMaxIdleTime(connMaxIdleTime)
	db.SetConnMaxLifetime(connMaxLifetime)

	if err := db.Ping(); err != nil {
		db.Close()
		return nil, fmt.Errorf("failed to ping database: %w", err)
	}

	return db, nil
}
