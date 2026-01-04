// internal/rules/evaluate_test.go
package rules

import (
	"encoding/json"
	"testing"

	"github.com/solatis/trapperkeeper/internal/types"
)

func TestEvaluate_SimpleMatch(t *testing.T) {
	rule := &types.Rule{
		RuleID:     "rule-001",
		Name:       "simple-match",
		SampleRate: 1.0,
		Action:     int(ActionObserve),
		OrGroups: []types.OrGroup{
			{
				Conditions: []types.Condition{
					{
						FieldPath:      []types.PathSegment{{Key: "status"}},
						Operator:       int(OpEq),
						FieldType:      int(FieldTypeText),
						Value:          "active",
						OnMissingField: int(OnMissingSkip),
						OnCoercionFail: int(OnCoercionSkip),
					},
				},
			},
		},
	}

	compiled, err := Compile(rule)
	if err != nil {
		t.Fatalf("Compile() error = %v, want nil", err)
	}

	payload := json.RawMessage(`{"status": "active"}`)
	result, err := Evaluate(compiled, payload)
	if err != nil {
		t.Fatalf("Evaluate() error = %v, want nil", err)
	}

	if !result.Matched {
		t.Errorf("Matched = false, want true")
	}
	if result.Action != ActionObserve {
		t.Errorf("Action = %v, want ActionObserve", result.Action)
	}
	if result.RuleID != "rule-001" {
		t.Errorf("RuleID = %v, want rule-001", result.RuleID)
	}
}

func TestEvaluate_MultiConditionAND(t *testing.T) {
	rule := &types.Rule{
		RuleID:     "rule-002",
		Name:       "multi-condition-and",
		SampleRate: 1.0,
		Action:     int(ActionObserve),
		OrGroups: []types.OrGroup{
			{
				Conditions: []types.Condition{
					{
						FieldPath:      []types.PathSegment{{Key: "status"}},
						Operator:       int(OpEq),
						FieldType:      int(FieldTypeText),
						Value:          "active",
						OnMissingField: int(OnMissingSkip),
						OnCoercionFail: int(OnCoercionSkip),
					},
					{
						FieldPath:      []types.PathSegment{{Key: "priority"}},
						Operator:       int(OpGt),
						FieldType:      int(FieldTypeNumeric),
						Value:          float64(5),
						OnMissingField: int(OnMissingSkip),
						OnCoercionFail: int(OnCoercionSkip),
					},
				},
			},
		},
	}

	compiled, err := Compile(rule)
	if err != nil {
		t.Fatalf("Compile() error = %v, want nil", err)
	}

	payload := json.RawMessage(`{"status": "active", "priority": 10}`)
	result, err := Evaluate(compiled, payload)
	if err != nil {
		t.Fatalf("Evaluate() error = %v, want nil", err)
	}

	if !result.Matched {
		t.Errorf("Matched = false, want true")
	}
}

func TestEvaluate_MultiConditionAND_ShortCircuit(t *testing.T) {
	rule := &types.Rule{
		RuleID:     "rule-003",
		Name:       "multi-condition-and-short-circuit",
		SampleRate: 1.0,
		Action:     int(ActionObserve),
		OrGroups: []types.OrGroup{
			{
				Conditions: []types.Condition{
					{
						FieldPath:      []types.PathSegment{{Key: "status"}},
						Operator:       int(OpEq),
						FieldType:      int(FieldTypeText),
						Value:          "inactive",
						OnMissingField: int(OnMissingSkip),
						OnCoercionFail: int(OnCoercionSkip),
					},
					{
						FieldPath:      []types.PathSegment{{Key: "priority"}},
						Operator:       int(OpGt),
						FieldType:      int(FieldTypeNumeric),
						Value:          float64(5),
						OnMissingField: int(OnMissingSkip),
						OnCoercionFail: int(OnCoercionSkip),
					},
				},
			},
		},
	}

	compiled, err := Compile(rule)
	if err != nil {
		t.Fatalf("Compile() error = %v, want nil", err)
	}

	payload := json.RawMessage(`{"status": "active", "priority": 10}`)
	result, err := Evaluate(compiled, payload)
	if err != nil {
		t.Fatalf("Evaluate() error = %v, want nil", err)
	}

	if result.Matched {
		t.Errorf("Matched = true, want false (first condition fails)")
	}
}

func TestEvaluate_MultiGroupOR(t *testing.T) {
	rule := &types.Rule{
		RuleID:     "rule-004",
		Name:       "multi-group-or",
		SampleRate: 1.0,
		Action:     int(ActionObserve),
		OrGroups: []types.OrGroup{
			{
				Conditions: []types.Condition{
					{
						FieldPath:      []types.PathSegment{{Key: "status"}},
						Operator:       int(OpEq),
						FieldType:      int(FieldTypeText),
						Value:          "critical",
						OnMissingField: int(OnMissingSkip),
						OnCoercionFail: int(OnCoercionSkip),
					},
				},
			},
			{
				Conditions: []types.Condition{
					{
						FieldPath:      []types.PathSegment{{Key: "priority"}},
						Operator:       int(OpGt),
						FieldType:      int(FieldTypeNumeric),
						Value:          float64(8),
						OnMissingField: int(OnMissingSkip),
						OnCoercionFail: int(OnCoercionSkip),
					},
				},
			},
		},
	}

	compiled, err := Compile(rule)
	if err != nil {
		t.Fatalf("Compile() error = %v, want nil", err)
	}

	payload := json.RawMessage(`{"status": "active", "priority": 10}`)
	result, err := Evaluate(compiled, payload)
	if err != nil {
		t.Fatalf("Evaluate() error = %v, want nil", err)
	}

	if !result.Matched {
		t.Errorf("Matched = false, want true (second group matches)")
	}
	if len(result.MatchedCondition) != 3 {
		t.Fatalf("len(MatchedCondition) = %v, want 3", len(result.MatchedCondition))
	}
	if result.MatchedCondition[0] != "any" {
		t.Errorf("MatchedCondition[0] = %v, want 'any'", result.MatchedCondition[0])
	}
	if result.MatchedCondition[1] != 1 {
		t.Errorf("MatchedCondition[1] = %v, want 1 (second group)", result.MatchedCondition[1])
	}
	if result.MatchedCondition[2] != "all" {
		t.Errorf("MatchedCondition[2] = %v, want 'all'", result.MatchedCondition[2])
	}
}

func TestEvaluate_MultiGroupOR_ShortCircuit(t *testing.T) {
	rule := &types.Rule{
		RuleID:     "rule-005",
		Name:       "multi-group-or-short-circuit",
		SampleRate: 1.0,
		Action:     int(ActionObserve),
		OrGroups: []types.OrGroup{
			{
				Conditions: []types.Condition{
					{
						FieldPath:      []types.PathSegment{{Key: "status"}},
						Operator:       int(OpEq),
						FieldType:      int(FieldTypeText),
						Value:          "active",
						OnMissingField: int(OnMissingSkip),
						OnCoercionFail: int(OnCoercionSkip),
					},
				},
			},
			{
				Conditions: []types.Condition{
					{
						FieldPath:      []types.PathSegment{{Key: "priority"}},
						Operator:       int(OpGt),
						FieldType:      int(FieldTypeNumeric),
						Value:          float64(8),
						OnMissingField: int(OnMissingSkip),
						OnCoercionFail: int(OnCoercionSkip),
					},
				},
			},
		},
	}

	compiled, err := Compile(rule)
	if err != nil {
		t.Fatalf("Compile() error = %v, want nil", err)
	}

	payload := json.RawMessage(`{"status": "active", "priority": 10}`)
	result, err := Evaluate(compiled, payload)
	if err != nil {
		t.Fatalf("Evaluate() error = %v, want nil", err)
	}

	if !result.Matched {
		t.Errorf("Matched = false, want true")
	}
	if result.MatchedCondition[1] != 0 {
		t.Errorf("MatchedCondition[1] = %v, want 0 (first group matches, short-circuit)", result.MatchedCondition[1])
	}
}

func TestEvaluate_SampleRateZero(t *testing.T) {
	rule := &types.Rule{
		RuleID:     "rule-006",
		Name:       "sample-rate-zero",
		SampleRate: 0.0,
		Action:     int(ActionObserve),
		OrGroups: []types.OrGroup{
			{
				Conditions: []types.Condition{
					{
						FieldPath:      []types.PathSegment{{Key: "status"}},
						Operator:       int(OpEq),
						FieldType:      int(FieldTypeText),
						Value:          "active",
						OnMissingField: int(OnMissingSkip),
						OnCoercionFail: int(OnCoercionSkip),
					},
				},
			},
		},
	}

	compiled, err := Compile(rule)
	if err != nil {
		t.Fatalf("Compile() error = %v, want nil", err)
	}

	payload := json.RawMessage(`{"status": "active"}`)
	result, err := Evaluate(compiled, payload)
	if err != nil {
		t.Fatalf("Evaluate() error = %v, want nil", err)
	}

	if result.Matched {
		t.Errorf("Matched = true, want false (sample_rate=0.0 never matches)")
	}
}

func TestEvaluate_SampleRateOne(t *testing.T) {
	rule := &types.Rule{
		RuleID:     "rule-007",
		Name:       "sample-rate-one",
		SampleRate: 1.0,
		Action:     int(ActionObserve),
		OrGroups: []types.OrGroup{
			{
				Conditions: []types.Condition{
					{
						FieldPath:      []types.PathSegment{{Key: "status"}},
						Operator:       int(OpEq),
						FieldType:      int(FieldTypeText),
						Value:          "active",
						OnMissingField: int(OnMissingSkip),
						OnCoercionFail: int(OnCoercionSkip),
					},
				},
			},
		},
	}

	compiled, err := Compile(rule)
	if err != nil {
		t.Fatalf("Compile() error = %v, want nil", err)
	}

	payload := json.RawMessage(`{"status": "active"}`)
	result, err := Evaluate(compiled, payload)
	if err != nil {
		t.Fatalf("Evaluate() error = %v, want nil", err)
	}

	if !result.Matched {
		t.Errorf("Matched = false, want true (sample_rate=1.0 always evaluates)")
	}
}

func TestEvaluate_EmptyPayload(t *testing.T) {
	rule := &types.Rule{
		RuleID:     "rule-008",
		Name:       "empty-payload",
		SampleRate: 1.0,
		Action:     int(ActionObserve),
		OrGroups: []types.OrGroup{
			{
				Conditions: []types.Condition{
					{
						FieldPath:      []types.PathSegment{{Key: "status"}},
						Operator:       int(OpEq),
						FieldType:      int(FieldTypeText),
						Value:          "active",
						OnMissingField: int(OnMissingSkip),
						OnCoercionFail: int(OnCoercionSkip),
					},
				},
			},
		},
	}

	compiled, err := Compile(rule)
	if err != nil {
		t.Fatalf("Compile() error = %v, want nil", err)
	}

	payload := json.RawMessage(`{}`)
	result, err := Evaluate(compiled, payload)
	if err != nil {
		t.Fatalf("Evaluate() error = %v, want nil", err)
	}

	if result.Matched {
		t.Errorf("Matched = true, want false (field missing, OnMissingSkip)")
	}
}

func TestEvaluate_AllConditionsSkip(t *testing.T) {
	rule := &types.Rule{
		RuleID:     "rule-009",
		Name:       "all-conditions-skip",
		SampleRate: 1.0,
		Action:     int(ActionObserve),
		OrGroups: []types.OrGroup{
			{
				Conditions: []types.Condition{
					{
						FieldPath:      []types.PathSegment{{Key: "missing1"}},
						Operator:       int(OpEq),
						FieldType:      int(FieldTypeText),
						Value:          "value",
						OnMissingField: int(OnMissingSkip),
						OnCoercionFail: int(OnCoercionSkip),
					},
					{
						FieldPath:      []types.PathSegment{{Key: "missing2"}},
						Operator:       int(OpEq),
						FieldType:      int(FieldTypeText),
						Value:          "value",
						OnMissingField: int(OnMissingSkip),
						OnCoercionFail: int(OnCoercionSkip),
					},
				},
			},
		},
	}

	compiled, err := Compile(rule)
	if err != nil {
		t.Fatalf("Compile() error = %v, want nil", err)
	}

	payload := json.RawMessage(`{}`)
	result, err := Evaluate(compiled, payload)
	if err != nil {
		t.Fatalf("Evaluate() error = %v, want nil", err)
	}

	if result.Matched {
		t.Errorf("Matched = true, want false (all conditions skip)")
	}
}

func TestEvaluate_OnMissingMatch(t *testing.T) {
	rule := &types.Rule{
		RuleID:     "rule-010",
		Name:       "on-missing-match",
		SampleRate: 1.0,
		Action:     int(ActionObserve),
		OrGroups: []types.OrGroup{
			{
				Conditions: []types.Condition{
					{
						FieldPath:      []types.PathSegment{{Key: "missing"}},
						Operator:       int(OpEq),
						FieldType:      int(FieldTypeText),
						Value:          "value",
						OnMissingField: int(OnMissingMatch),
						OnCoercionFail: int(OnCoercionSkip),
					},
				},
			},
		},
	}

	compiled, err := Compile(rule)
	if err != nil {
		t.Fatalf("Compile() error = %v, want nil", err)
	}

	payload := json.RawMessage(`{}`)
	result, err := Evaluate(compiled, payload)
	if err != nil {
		t.Fatalf("Evaluate() error = %v, want nil", err)
	}

	if !result.Matched {
		t.Errorf("Matched = false, want true (OnMissingMatch)")
	}
}

func TestEvaluate_CoercionFailureSkip(t *testing.T) {
	rule := &types.Rule{
		RuleID:     "rule-011",
		Name:       "coercion-failure-skip",
		SampleRate: 1.0,
		Action:     int(ActionObserve),
		OrGroups: []types.OrGroup{
			{
				Conditions: []types.Condition{
					{
						FieldPath:      []types.PathSegment{{Key: "value"}},
						Operator:       int(OpGt),
						FieldType:      int(FieldTypeNumeric),
						Value:          float64(5),
						OnMissingField: int(OnMissingSkip),
						OnCoercionFail: int(OnCoercionSkip),
					},
				},
			},
		},
	}

	compiled, err := Compile(rule)
	if err != nil {
		t.Fatalf("Compile() error = %v, want nil", err)
	}

	payload := json.RawMessage(`{"value": "not-a-number"}`)
	result, err := Evaluate(compiled, payload)
	if err != nil {
		t.Fatalf("Evaluate() error = %v, want nil", err)
	}

	if result.Matched {
		t.Errorf("Matched = true, want false (coercion fails, OnCoercionSkip)")
	}
}

func TestEvaluate_CoercionFailureMatch(t *testing.T) {
	rule := &types.Rule{
		RuleID:     "rule-012",
		Name:       "coercion-failure-match",
		SampleRate: 1.0,
		Action:     int(ActionObserve),
		OrGroups: []types.OrGroup{
			{
				Conditions: []types.Condition{
					{
						FieldPath:      []types.PathSegment{{Key: "value"}},
						Operator:       int(OpGt),
						FieldType:      int(FieldTypeNumeric),
						Value:          float64(5),
						OnMissingField: int(OnMissingSkip),
						OnCoercionFail: int(OnCoercionMatch),
					},
				},
			},
		},
	}

	compiled, err := Compile(rule)
	if err != nil {
		t.Fatalf("Compile() error = %v, want nil", err)
	}

	payload := json.RawMessage(`{"value": "not-a-number"}`)
	result, err := Evaluate(compiled, payload)
	if err != nil {
		t.Fatalf("Evaluate() error = %v, want nil", err)
	}

	if !result.Matched {
		t.Errorf("Matched = false, want true (coercion fails, OnCoercionMatch)")
	}
}

func TestEvaluate_AllOperators(t *testing.T) {
	tests := []struct {
		name     string
		operator Operator
		value    any
		target   any
		want     bool
	}{
		{"exists_true", OpExists, "value", nil, true},
		{"exists_false", OpExists, nil, nil, false},
		{"is_null_true", OpIsNull, nil, nil, true},
		{"is_null_false", OpIsNull, "value", nil, false},
		{"eq_true", OpEq, float64(5), float64(5), true},
		{"eq_false", OpEq, float64(5), float64(6), false},
		{"neq_true", OpNeq, float64(5), float64(6), true},
		{"neq_false", OpNeq, float64(5), float64(5), false},
		{"lt_true", OpLt, float64(5), float64(6), true},
		{"lt_false", OpLt, float64(6), float64(5), false},
		{"lte_true", OpLte, float64(5), float64(5), true},
		{"lte_false", OpLte, float64(6), float64(5), false},
		{"gt_true", OpGt, float64(6), float64(5), true},
		{"gt_false", OpGt, float64(5), float64(6), false},
		{"gte_true", OpGte, float64(5), float64(5), true},
		{"gte_false", OpGte, float64(4), float64(5), false},
		{"prefix_true", OpPrefix, "hello world", "hello", true},
		{"prefix_false", OpPrefix, "hello world", "world", false},
		{"suffix_true", OpSuffix, "hello world", "world", true},
		{"suffix_false", OpSuffix, "hello world", "hello", false},
		{"in_true", OpIn, float64(5), []any{float64(3), float64(5), float64(7)}, true},
		{"in_false", OpIn, float64(6), []any{float64(3), float64(5), float64(7)}, false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := Compare(tt.operator, tt.value, tt.target)
			if got != tt.want {
				t.Errorf("Compare(%v, %v, %v) = %v, want %v",
					tt.operator, tt.value, tt.target, got, tt.want)
			}
		})
	}
}
