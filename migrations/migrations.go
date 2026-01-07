package migrations

import "embed"

// Embedded migration files bundled at compile time
// Single binary deployment without external file dependencies
//
//go:embed sqlite/*.sql
var SqliteMigrations embed.FS

//go:embed postgres/*.sql
var PostgresMigrations embed.FS
