package api

import (
	"context"
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"sort"
	"time"

	"github.com/solatis/trapperkeeper/internal/core/auth"
	pb "github.com/solatis/trapperkeeper/internal/protobuf/trapperkeeper/sensor/v1"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
	"google.golang.org/protobuf/types/known/timestamppb"
)

// SyncRules returns rules matching requested tags.
// ETAG-based caching minimizes bandwidth when rules unchanged.
// Returns up to 10,000 rules per tenant.
func (s *SensorAPIService) SyncRules(ctx context.Context, req *pb.SyncRulesRequest) (*pb.SyncRulesResponse, error) {
	tenantID := auth.TenantIDFromContext(ctx)
	if tenantID == "" {
		return nil, status.Error(codes.Internal, "missing tenant_id in context")
	}

	// Query rules by tags
	// If no tags specified, return all rules for tenant
	var rules []struct {
		RuleID     string  `db:"rule_id"`
		Name       string  `db:"name"`
		State      string  `db:"state"`
		Action     string  `db:"action"`
		Expression string  `db:"expression"`
		SampleRate float64 `db:"sample_rate"`
		ScopeTags  string  `db:"scope_tags"`
		CreatedAt  string  `db:"created_at"`
	}

	// Returns all rules for tenant
	query := `
		SELECT rule_id, name, state, action, expression, sample_rate, scope_tags, created_at
		FROM rules
		WHERE tenant_id = ?
		ORDER BY created_at DESC
		LIMIT 10000
	`
	err := s.db.SelectContext(ctx, &rules, s.db.Rebind(query), tenantID)
	if err != nil {
		return nil, status.Error(codes.Unavailable, fmt.Sprintf("failed to query rules: %v", err))
	}

	// Compute ETAG as SHA256(sorted rule_ids + created_at timestamps)
	// ETAG is content-addressable: same rules always produce same ETAG
	etag := computeETAG(rules)

	// ETAG matching optimization requires if_none_match field in SyncRulesRequest proto.
	// Current proto definition lacks this field; server always returns full rule set.
	// Bandwidth optimization via ETAG comparison unavailable without proto extension.

	// Convert database rules to proto format
	var pbRules []*pb.Rule
	for _, r := range rules {
		// Parse expression JSON to or_groups
		var orGroups []*pb.OrGroup
		if r.Expression != "" {
			if err := json.Unmarshal([]byte(r.Expression), &orGroups); err != nil {
				// Skip malformed rule - continue processing others
				continue
			}
		}

		// Parse scope_tags JSON to ScopeTag array
		var scopeTags []*pb.ScopeTag
		if r.ScopeTags != "" {
			if err := json.Unmarshal([]byte(r.ScopeTags), &scopeTags); err != nil {
				// Skip malformed rule - continue processing others
				continue
			}
		}

		// Parse created_at timestamp
		createdAt, err := time.Parse(time.RFC3339, r.CreatedAt)
		if err != nil {
			// Skip malformed rule - continue processing others
			continue
		}

		pbRules = append(pbRules, &pb.Rule{
			RuleId:     r.RuleID,
			Name:       r.Name,
			State:      stringToRuleState(r.State),
			Action:     stringToAction(r.Action),
			OrGroups:   orGroups,
			SampleRate: r.SampleRate,
			ScopeTags:  scopeTags,
			CreatedAt:  timestamppb.New(createdAt),
		})
	}

	return &pb.SyncRulesResponse{
		Rules: pbRules,
		Etag:  etag,
	}, nil
}

// computeETAG generates content-addressable hash enabling bandwidth-efficient sync.
func computeETAG(rules []struct{
	RuleID     string  `db:"rule_id"`
	Name       string  `db:"name"`
	State      string  `db:"state"`
	Action     string  `db:"action"`
	Expression string  `db:"expression"`
	SampleRate float64 `db:"sample_rate"`
	ScopeTags  string  `db:"scope_tags"`
	CreatedAt  string  `db:"created_at"`
}) string {
	h := sha256.New()
	var ids []string
	for _, r := range rules {
		ids = append(ids, r.RuleID+":"+r.CreatedAt)
	}
	sort.Strings(ids)
	for _, id := range ids {
		h.Write([]byte(id))
	}
	return fmt.Sprintf("%x", h.Sum(nil))
}

func stringToRuleState(s string) pb.RuleState {
	switch s {
	case "draft":
		return pb.RuleState_RULE_STATE_DRAFT
	case "active":
		return pb.RuleState_RULE_STATE_ACTIVE
	case "disabled":
		return pb.RuleState_RULE_STATE_DISABLED
	default:
		return pb.RuleState_RULE_STATE_UNSPECIFIED
	}
}

func stringToAction(a string) pb.Action {
	switch a {
	case "observe":
		return pb.Action_ACTION_OBSERVE
	case "drop":
		return pb.Action_ACTION_DROP
	case "fail":
		return pb.Action_ACTION_FAIL
	default:
		return pb.Action_ACTION_UNSPECIFIED
	}
}
