// Package api provides gRPC service implementations for TrapperKeeper sensor API.
package api

import (
	"fmt"
	"os"
	"path/filepath"
	"sync"

	"github.com/jmoiron/sqlx"
	"github.com/solatis/trapperkeeper/internal/core/config"
	pb "github.com/solatis/trapperkeeper/internal/protobuf/trapperkeeper/sensor/v1"
	"github.com/solatis/trapperkeeper/internal/rules"
)

// SensorAPIService implements the gRPC SensorAPIServer interface.
// Thin orchestration layer delegating to auth, rules, and database packages.
type SensorAPIService struct {
	pb.UnimplementedSensorAPIServer
	db           *sqlx.DB
	rulesEngine  *rules.Engine
	cfg          *config.SensorAPIConfig
	jsonlMutexes map[string]*sync.Mutex
	mutexLock    sync.Mutex
}

// NewSensorAPIService creates service instance with dependencies.
// Auto-creates events directory if not exists.
func NewSensorAPIService(db *sqlx.DB, rulesEngine *rules.Engine, cfg *config.SensorAPIConfig) (*SensorAPIService, error) {
	if db == nil {
		return nil, fmt.Errorf("db cannot be nil")
	}
	if rulesEngine == nil {
		return nil, fmt.Errorf("rulesEngine cannot be nil")
	}
	if cfg == nil {
		return nil, fmt.Errorf("cfg cannot be nil")
	}

	eventsDir := filepath.Join(cfg.DataDir, "events")
	if err := os.MkdirAll(eventsDir, 0755); err != nil {
		return nil, err
	}

	return &SensorAPIService{
		db:           db,
		rulesEngine:  rulesEngine,
		cfg:          cfg,
		jsonlMutexes: make(map[string]*sync.Mutex),
	}, nil
}

// getJSONLMutex returns mutex for given filename, creating if not exists.
// Per-file mutex protects concurrent writes to same daily JSONL file.
// Mutex map grows by ~1 entry/day (acceptable memory footprint for annual lifecycle).
func (s *SensorAPIService) getJSONLMutex(filename string) *sync.Mutex {
	s.mutexLock.Lock()
	defer s.mutexLock.Unlock()

	if _, ok := s.jsonlMutexes[filename]; !ok {
		s.jsonlMutexes[filename] = &sync.Mutex{}
	}
	return s.jsonlMutexes[filename]
}
