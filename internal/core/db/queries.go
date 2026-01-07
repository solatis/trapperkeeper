package db

import (
	"database/sql"
	"embed"
	"fmt"
	"io/fs"
	"path/filepath"

	"github.com/jmoiron/sqlx"
	"github.com/qustavo/dotsql"
)

//go:embed queries/*.sql
var queriesFS embed.FS

// Queries provides access to named SQL queries loaded from embedded .sql files.
// Uses dotsql for named query management and sqlx for database operations.
type Queries struct {
	dot *dotsql.DotSql
	db  *sqlx.DB
}

// LoadQueries loads all .sql files from embedded filesystem and returns Queries instance.
// Named queries accessible by name (e.g., "get-tenant", "list-rules").
func LoadQueries(db *sqlx.DB) (*Queries, error) {
	var combinedSQL string

	err := fs.WalkDir(queriesFS, "queries", func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if d.IsDir() || filepath.Ext(path) != ".sql" {
			return nil
		}

		content, err := queriesFS.ReadFile(path)
		if err != nil {
			return fmt.Errorf("failed to read %s: %w", path, err)
		}

		combinedSQL += string(content) + "\n"
		return nil
	})

	if err != nil {
		return nil, fmt.Errorf("failed to load query files: %w", err)
	}

	dot, err := dotsql.LoadFromString(combinedSQL)
	if err != nil {
		return nil, fmt.Errorf("failed to parse queries: %w", err)
	}

	return &Queries{dot: dot, db: db}, nil
}

// Exec executes a named query with placeholder conversion for database compatibility.
// Uses sqlx Rebind to convert ? placeholders to $1, $2 for PostgreSQL.
func (q *Queries) Exec(name string, args ...interface{}) (sql.Result, error) {
	query, err := q.dot.Raw(name)
	if err != nil {
		return nil, fmt.Errorf("query not found: %s", name)
	}
	return q.db.Exec(q.db.Rebind(query), args...)
}

// Get retrieves a single row into dest struct using named query.
func (q *Queries) Get(name string, dest interface{}, args ...interface{}) error {
	query, err := q.dot.Raw(name)
	if err != nil {
		return fmt.Errorf("query not found: %s", name)
	}
	return q.db.Get(dest, q.db.Rebind(query), args...)
}

// Select retrieves multiple rows into dest slice using named query.
func (q *Queries) Select(name string, dest interface{}, args ...interface{}) error {
	query, err := q.dot.Raw(name)
	if err != nil {
		return fmt.Errorf("query not found: %s", name)
	}
	return q.db.Select(dest, q.db.Rebind(query), args...)
}
