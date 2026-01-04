// internal/rules/compile_test.go
package rules

import (
	"testing"

	"github.com/solatis/trapperkeeper/internal/types"
)

func TestCompile_SimpleRule(t *testing.T) {
	rule := &types.Rule{
		RuleID:     "rule-001",
		Name:       "simple-rule",
		SampleRate: 1.0,
		Action:     int(ActionObserve),
		OrGroups: []types.OrGroup{
			{
				Conditions: []types.Condition{
					{
						FieldPath: []types.PathSegment{{Key: "user"}},
						Operator:  int(OpExists),
						FieldType: int(FieldTypeAny),
					},
				},
			},
		},
	}

	compiled, err := Compile(rule)
	if err != nil {
		t.Fatalf("Compile() error = %v, want nil", err)
	}

	if compiled.RuleID != "rule-001" {
		t.Errorf("RuleID = %v, want %v", compiled.RuleID, "rule-001")
	}
	if compiled.Name != "simple-rule" {
		t.Errorf("Name = %v, want %v", compiled.Name, "simple-rule")
	}
	if len(compiled.OrGroups) != 1 {
		t.Fatalf("len(OrGroups) = %v, want 1", len(compiled.OrGroups))
	}
	if len(compiled.OrGroups[0].Conditions) != 1 {
		t.Fatalf("len(Conditions) = %v, want 1", len(compiled.OrGroups[0].Conditions))
	}
}

func TestCompile_MultiGroupDNF(t *testing.T) {
	rule := &types.Rule{
		RuleID:     "rule-002",
		Name:       "multi-group-dnf",
		SampleRate: 1.0,
		Action:     int(ActionObserve),
		OrGroups: []types.OrGroup{
			{
				Conditions: []types.Condition{
					{
						FieldPath: []types.PathSegment{{Key: "status"}},
						Operator:  int(OpEq),
						FieldType: int(FieldTypeText),
						Value:     "active",
					},
				},
			},
			{
				Conditions: []types.Condition{
					{
						FieldPath: []types.PathSegment{{Key: "priority"}},
						Operator:  int(OpGt),
						FieldType: int(FieldTypeNumeric),
						Value:     float64(5),
					},
				},
			},
		},
	}

	compiled, err := Compile(rule)
	if err != nil {
		t.Fatalf("Compile() error = %v, want nil", err)
	}

	if len(compiled.OrGroups) != 2 {
		t.Fatalf("len(OrGroups) = %v, want 2", len(compiled.OrGroups))
	}
}

func TestCompile_ConditionsOrderedByCost(t *testing.T) {
	rule := &types.Rule{
		RuleID:     "rule-003",
		Name:       "cost-ordering",
		SampleRate: 1.0,
		Action:     int(ActionObserve),
		OrGroups: []types.OrGroup{
			{
				Conditions: []types.Condition{
					{
						FieldPath: []types.PathSegment{{Key: "name"}},
						Operator:  int(OpPrefix),
						FieldType: int(FieldTypeText),
						Value:     "test",
					},
					{
						FieldPath: []types.PathSegment{{Key: "user"}},
						Operator:  int(OpExists),
						FieldType: int(FieldTypeAny),
					},
				},
			},
		},
	}

	compiled, err := Compile(rule)
	if err != nil {
		t.Fatalf("Compile() error = %v, want nil", err)
	}

	conditions := compiled.OrGroups[0].Conditions
	if len(conditions) != 2 {
		t.Fatalf("len(Conditions) = %v, want 2", len(conditions))
	}

	// exists (cost 1) should be ordered before prefix (cost 10)
	if conditions[0].Operator != OpExists {
		t.Errorf("First condition operator = %v, want OpExists (cost 1)", conditions[0].Operator)
	}
	if conditions[1].Operator != OpPrefix {
		t.Errorf("Second condition operator = %v, want OpPrefix (cost 10)", conditions[1].Operator)
	}

	// Verify costs
	if conditions[0].Cost >= conditions[1].Cost {
		t.Errorf("Conditions not ordered by cost: %v >= %v", conditions[0].Cost, conditions[1].Cost)
	}
}

func TestCompile_MaximumWildcardsAllowed(t *testing.T) {
	rule := &types.Rule{
		RuleID:     "rule-004",
		Name:       "max-wildcards",
		SampleRate: 1.0,
		Action:     int(ActionObserve),
		OrGroups: []types.OrGroup{
			{
				Conditions: []types.Condition{
					{
						FieldPath: []types.PathSegment{
							{Key: "orders"},
							{Wildcard: true},
							{Key: "items"},
							{Wildcard: true},
							{Key: "price"},
						},
						Operator:  int(OpGt),
						FieldType: int(FieldTypeNumeric),
						Value:     float64(100),
					},
				},
			},
		},
	}

	compiled, err := Compile(rule)
	if err != nil {
		t.Fatalf("Compile() error = %v, want nil (2 wildcards should be allowed)", err)
	}

	if len(compiled.OrGroups) != 1 {
		t.Fatalf("len(OrGroups) = %v, want 1", len(compiled.OrGroups))
	}
}

func TestCompile_MaximumINValues(t *testing.T) {
	values := make([]any, types.MaxInOperatorValues)
	for i := 0; i < types.MaxInOperatorValues; i++ {
		values[i] = i
	}

	rule := &types.Rule{
		RuleID:     "rule-005",
		Name:       "max-in-values",
		SampleRate: 1.0,
		Action:     int(ActionObserve),
		OrGroups: []types.OrGroup{
			{
				Conditions: []types.Condition{
					{
						FieldPath: []types.PathSegment{{Key: "status"}},
						Operator:  int(OpIn),
						FieldType: int(FieldTypeNumeric),
						Values:    values,
					},
				},
			},
		},
	}

	compiled, err := Compile(rule)
	if err != nil {
		t.Fatalf("Compile() error = %v, want nil (64 IN values should be allowed)", err)
	}

	if len(compiled.OrGroups[0].Conditions[0].Values) != types.MaxInOperatorValues {
		t.Errorf("len(Values) = %v, want %v", len(compiled.OrGroups[0].Conditions[0].Values), types.MaxInOperatorValues)
	}
}

func TestCompile_ErrorPathTooDeep(t *testing.T) {
	// Create path with 17 segments (exceeds MaxPathDepth of 16)
	path := make([]types.PathSegment, types.MaxPathDepth+1)
	for i := range path {
		path[i] = types.PathSegment{Key: "level"}
	}

	rule := &types.Rule{
		RuleID:     "rule-006",
		Name:       "path-too-deep",
		SampleRate: 1.0,
		Action:     int(ActionObserve),
		OrGroups: []types.OrGroup{
			{
				Conditions: []types.Condition{
					{
						FieldPath: path,
						Operator:  int(OpExists),
						FieldType: int(FieldTypeAny),
					},
				},
			},
		},
	}

	_, err := Compile(rule)
	if err != types.ErrPathTooDeep {
		t.Errorf("Compile() error = %v, want ErrPathTooDeep", err)
	}
}

func TestCompile_ErrorTooManyWildcards(t *testing.T) {
	rule := &types.Rule{
		RuleID:     "rule-007",
		Name:       "too-many-wildcards",
		SampleRate: 1.0,
		Action:     int(ActionObserve),
		OrGroups: []types.OrGroup{
			{
				Conditions: []types.Condition{
					{
						FieldPath: []types.PathSegment{
							{Key: "level1"},
							{Wildcard: true},
							{Key: "level2"},
							{Wildcard: true},
							{Key: "level3"},
							{Wildcard: true},
						},
						Operator:  int(OpExists),
						FieldType: int(FieldTypeAny),
					},
				},
			},
		},
	}

	_, err := Compile(rule)
	if err != types.ErrTooManyWildcards {
		t.Errorf("Compile() error = %v, want ErrTooManyWildcards", err)
	}
}

func TestCompile_ErrorINOperatorTooManyValues(t *testing.T) {
	values := make([]any, types.MaxInOperatorValues+1)
	for i := range values {
		values[i] = i
	}

	rule := &types.Rule{
		RuleID:     "rule-008",
		Name:       "too-many-in-values",
		SampleRate: 1.0,
		Action:     int(ActionObserve),
		OrGroups: []types.OrGroup{
			{
				Conditions: []types.Condition{
					{
						FieldPath: []types.PathSegment{{Key: "status"}},
						Operator:  int(OpIn),
						FieldType: int(FieldTypeNumeric),
						Values:    values,
					},
				},
			},
		},
	}

	_, err := Compile(rule)
	if err != types.ErrTooManyInValues {
		t.Errorf("Compile() error = %v, want ErrTooManyInValues", err)
	}
}

func TestCompile_ErrorFieldRefWithWildcard(t *testing.T) {
	rule := &types.Rule{
		RuleID:     "rule-009",
		Name:       "field-ref-with-wildcard",
		SampleRate: 1.0,
		Action:     int(ActionObserve),
		OrGroups: []types.OrGroup{
			{
				Conditions: []types.Condition{
					{
						FieldPath: []types.PathSegment{{Key: "value"}},
						FieldRef: []types.PathSegment{
							{Key: "items"},
							{Wildcard: true},
							{Key: "threshold"},
						},
						Operator:  int(OpGt),
						FieldType: int(FieldTypeNumeric),
					},
				},
			},
		},
	}

	_, err := Compile(rule)
	if err != types.ErrWildcardInFieldRef {
		t.Errorf("Compile() error = %v, want ErrWildcardInFieldRef", err)
	}
}

func TestCompile_PriorityCalculation(t *testing.T) {
	rule := &types.Rule{
		RuleID:     "rule-010",
		Name:       "priority-calculation",
		SampleRate: 0.5,
		Action:     int(ActionObserve),
		OrGroups: []types.OrGroup{
			{
				Conditions: []types.Condition{
					{
						FieldPath: []types.PathSegment{{Key: "user"}},
						Operator:  int(OpExists),
						FieldType: int(FieldTypeAny),
					},
				},
			},
		},
	}

	compiled, err := Compile(rule)
	if err != nil {
		t.Fatalf("Compile() error = %v, want nil", err)
	}

	// Priority = BasePriority + totalCost + orPenalty + samplePenalty
	// BasePriority = 1000
	// totalCost = lookup_cost + (operator_cost * type_multiplier * 8^wildcards)
	//           = 128 + (1 * 128 * 1) = 256
	// orPenalty = 1 * 10 = 10
	// samplePenalty = (1.0 - 0.5) * 50 = 25
	// Priority = 1000 + 256 + 10 + 25 = 1291

	expectedPriority := 1291
	if compiled.Priority != expectedPriority {
		t.Errorf("Priority = %v, want %v", compiled.Priority, expectedPriority)
	}
}
