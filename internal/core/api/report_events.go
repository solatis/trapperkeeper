package api

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sync"
	"time"

	"github.com/solatis/trapperkeeper/internal/core/auth"
	pb "github.com/solatis/trapperkeeper/internal/protobuf/trapperkeeper/sensor/v1"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
)

// ReportEvents ingests event batch from sensor.
// Per-event transactions enable partial batch success.
// JSONL output is best-effort debugging aid, not authoritative.
func (s *SensorAPIService) ReportEvents(ctx context.Context, req *pb.ReportEventsRequest) (*pb.ReportEventsResponse, error) {
	tenantID := auth.TenantIDFromContext(ctx)
	if tenantID == "" {
		return nil, status.Error(codes.Internal, "missing tenant_id in context")
	}

	// Reject batches exceeding max size
	// Prevents transaction timeouts and memory exhaustion
	if req.Events == nil || len(req.Events) > s.cfg.MaxBatchSize {
		return nil, status.Error(codes.InvalidArgument, fmt.Sprintf("batch size exceeds maximum of %d events", s.cfg.MaxBatchSize))
	}

	// Determine JSONL filename at request start
	// All events in batch written to same file even if processing spans midnight
	now := time.Now().UTC()
	jsonlFilename := filepath.Join(s.cfg.DataDir, "events", now.Format("2006-01-02.jsonl"))
	jsonlMutex := s.getJSONLMutex(jsonlFilename)

	results := make([]*pb.EventResult, len(req.Events))
	acceptedCount := int32(0)

	for i, event := range req.Events {
		result := s.processEvent(ctx, tenantID, event, jsonlFilename, jsonlMutex)
		results[i] = result
		if result.Status == pb.EventStatus_EVENT_STATUS_ACCEPTED {
			acceptedCount++
		}
	}

	return &pb.ReportEventsResponse{
		AcceptedCount: acceptedCount,
		Results:       results,
	}, nil
}

// processEvent validates, persists, and logs single event in own transaction.
// Per-event transactions enable partial batch success when some events fail.
func (s *SensorAPIService) processEvent(ctx context.Context, tenantID string, event *pb.Event, jsonlFilename string, jsonlMutex *sync.Mutex) *pb.EventResult {
	// Validate event structure
	if event.EventId == "" {
		return &pb.EventResult{
			EventId:      event.EventId,
			Status:       pb.EventStatus_EVENT_STATUS_REJECTED,
			ErrorMessage: "event_id required",
		}
	}

	// Insert to database (own transaction)
	// JSONL may contain events not in database (if DB insert failed)
	// Database is source of truth, JSONL is debugging aid
	insertQuery := `
		INSERT INTO events (event_id, tenant_id, client_timestamp, server_received_at, file_path, file_offset, payload_hash, matched_rule_count, created_at)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
	`
	serverReceivedAt := time.Now().UTC()

	// Convert proto timestamp to RFC3339 string for database
	var clientTimestamp string
	if event.ClientTimestamp != nil {
		clientTimestamp = event.ClientTimestamp.AsTime().UTC().Format(time.RFC3339)
	} else {
		clientTimestamp = serverReceivedAt.Format(time.RFC3339)
	}

	_, err := s.db.ExecContext(ctx, s.db.Rebind(insertQuery),
		event.EventId,
		tenantID,
		clientTimestamp,
		serverReceivedAt.Format(time.RFC3339),
		jsonlFilename,
		0, // file_offset computed during JSONL write
		"", // payload_hash: empty (content-addressable indexing not implemented)
		0,  // matched_rule_count: computed during evaluation
		serverReceivedAt.Format(time.RFC3339),
	)
	if err != nil {
		return &pb.EventResult{
			EventId:      event.EventId,
			Status:       pb.EventStatus_EVENT_STATUS_ERROR,
			ErrorMessage: fmt.Sprintf("database error: %v", err),
		}
	}

	// Write to JSONL (best-effort, with mutex protection)
	jsonlMutex.Lock()
	defer jsonlMutex.Unlock()
	f, err := os.OpenFile(jsonlFilename, os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0644)
	if err == nil {
		defer f.Close()
		encoder := json.NewEncoder(f)
		_ = encoder.Encode(event)
	}

	return &pb.EventResult{
		EventId: event.EventId,
		Status:  pb.EventStatus_EVENT_STATUS_ACCEPTED,
	}
}
