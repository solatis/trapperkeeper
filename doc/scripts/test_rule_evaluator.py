#!/usr/bin/env python3
"""
Tests for rule_evaluator.py - Generic rule interpreter for template-driven validation.

Test coverage:
- ValidationError dataclass and formatting
- RuleEvaluator initialization and basic operation
- title_pattern rule with 5+ test cases
- max_lines rule with 3+ test cases
- forbidden rule with pattern detection and line numbers
- filename_pattern rule
- Integration with template frontmatter
- Error message format validation
"""

import unittest
import tempfile
from pathlib import Path
from rule_evaluator import RuleEvaluator, ValidationError, validate_document


class TestValidationError(unittest.TestCase):
    """Test ValidationError dataclass and formatting."""

    def test_validation_error_creation(self):
        """Test ValidationError can be created with all required fields."""
        error = ValidationError(
            file_path="test.md",
            line_number=5,
            rule_violated="title_pattern",
            detail="Title does not match",
            expected="Title ending with Guide",
            found="# Welcome",
            severity="error"
        )

        self.assertEqual(error.file_path, "test.md")
        self.assertEqual(error.line_number, 5)
        self.assertEqual(error.rule_violated, "title_pattern")
        self.assertEqual(error.detail, "Title does not match")
        self.assertEqual(error.expected, "Title ending with Guide")
        self.assertEqual(error.found, "# Welcome")
        self.assertEqual(error.severity, "error")

    def test_validation_error_default_severity(self):
        """Test ValidationError defaults to error severity."""
        error = ValidationError(
            file_path="test.md",
            line_number=1,
            rule_violated="max_lines",
            detail="Too many lines",
            expected="50 lines",
            found="100 lines"
        )

        self.assertEqual(error.severity, "error")

    def test_format_error_with_line_number(self):
        """Test error formatting includes line number."""
        error = ValidationError(
            file_path="test.md",
            line_number=5,
            rule_violated="title_pattern",
            detail="Title does not match required pattern",
            expected="Title ending with Guide",
            found="# Welcome",
            severity="error"
        )

        formatted = error.format_error()

        self.assertIn("[ERROR]", formatted)
        self.assertIn("test.md:5", formatted)
        self.assertIn("title_pattern", formatted)
        self.assertIn("Title does not match required pattern", formatted)
        self.assertIn("Title ending with Guide", formatted)
        self.assertIn("# Welcome", formatted)

    def test_format_error_without_line_number(self):
        """Test error formatting for file-level errors."""
        error = ValidationError(
            file_path="test.md",
            line_number=0,
            rule_violated="max_lines",
            detail="Document exceeds maximum line limit",
            expected="Maximum 50 lines",
            found="100 lines",
            severity="error"
        )

        formatted = error.format_error()

        self.assertIn("[ERROR]", formatted)
        self.assertIn("test.md", formatted)
        self.assertNotIn("test.md:0", formatted)
        self.assertIn("max_lines", formatted)

    def test_format_warning(self):
        """Test warning severity formatting."""
        error = ValidationError(
            file_path="test.md",
            line_number=10,
            rule_violated="forbidden",
            detail="Contains deprecated pattern",
            expected="",
            found="legacy code",
            severity="warn"
        )

        formatted = error.format_error()
        self.assertIn("[WARN]", formatted)


class TestTitlePatternRule(unittest.TestCase):
    """Test title_pattern rule with 5+ test cases."""

    def test_title_pattern_match(self):
        """Test title_pattern passes when title matches."""
        content = "# Documentation Guide\n\nContent here."
        schema = {
            "schema_version": 1,
            "title_pattern": "^# .+ Guide$"
        }

        evaluator = RuleEvaluator(schema)
        errors = evaluator.evaluate(Path("test.md"), content)

        self.assertEqual(len(errors), 0)

    def test_title_pattern_no_match(self):
        """Test title_pattern fails when title doesn't match."""
        content = "# Welcome\n\nContent here."
        schema = {
            "schema_version": 1,
            "title_pattern": "^# .+ Guide$"
        }

        evaluator = RuleEvaluator(schema)
        errors = evaluator.evaluate(Path("test.md"), content)

        self.assertEqual(len(errors), 1)
        self.assertEqual(errors[0].rule_violated, "title_pattern")
        self.assertEqual(errors[0].line_number, 1)
        self.assertIn("# Welcome", errors[0].found)

    def test_title_pattern_missing_title(self):
        """Test title_pattern fails when document has no H1."""
        content = "## Section\n\nContent here."
        schema = {
            "schema_version": 1,
            "title_pattern": "^# .+ Guide$"
        }

        evaluator = RuleEvaluator(schema)
        errors = evaluator.evaluate(Path("test.md"), content)

        self.assertEqual(len(errors), 1)
        self.assertEqual(errors[0].rule_violated, "title_pattern")
        self.assertIn("No H1 heading", errors[0].found)

    def test_title_pattern_case_sensitive(self):
        """Test title_pattern is case-sensitive by default."""
        content = "# documentation guide\n\nContent here."
        schema = {
            "schema_version": 1,
            "title_pattern": "^# Documentation Guide$"
        }

        evaluator = RuleEvaluator(schema)
        errors = evaluator.evaluate(Path("test.md"), content)

        self.assertEqual(len(errors), 1)
        self.assertEqual(errors[0].rule_violated, "title_pattern")

    def test_title_pattern_with_special_characters(self):
        """Test title_pattern with special regex characters."""
        content = "# CLAUDE.md Guide\n\nContent here."
        schema = {
            "schema_version": 1,
            "title_pattern": "^# CLAUDE\\.md Guide$"
        }

        evaluator = RuleEvaluator(schema)
        errors = evaluator.evaluate(Path("test.md"), content)

        self.assertEqual(len(errors), 0)

    def test_title_pattern_multiple_h1_uses_first(self):
        """Test title_pattern uses first H1 when multiple exist."""
        content = "# First Title\n\nContent.\n\n# Second Title\n\nMore content."
        schema = {
            "schema_version": 1,
            "title_pattern": "^# First Title$"
        }

        evaluator = RuleEvaluator(schema)
        errors = evaluator.evaluate(Path("test.md"), content)

        self.assertEqual(len(errors), 0)

    def test_title_pattern_invalid_regex(self):
        """Test title_pattern handles invalid regex gracefully."""
        content = "# Valid Title\n\nContent here."
        schema = {
            "schema_version": 1,
            "title_pattern": "^# [Invalid(Regex$"
        }

        evaluator = RuleEvaluator(schema)
        errors = evaluator.evaluate(Path("test.md"), content)

        self.assertEqual(len(errors), 1)
        self.assertEqual(errors[0].rule_violated, "title_pattern")
        self.assertIn("Invalid regex pattern", errors[0].detail)


class TestMaxLinesRule(unittest.TestCase):
    """Test max_lines rule with 3+ test cases."""

    def test_max_lines_within_limit(self):
        """Test max_lines passes when document is within limit."""
        content = "\n".join([f"Line {i}" for i in range(1, 26)])  # 25 lines
        schema = {
            "schema_version": 1,
            "max_lines": 50
        }

        evaluator = RuleEvaluator(schema)
        errors = evaluator.evaluate(Path("test.md"), content)

        self.assertEqual(len(errors), 0)

    def test_max_lines_exceeds_limit(self):
        """Test max_lines fails when document exceeds limit."""
        content = "\n".join([f"Line {i}" for i in range(1, 101)])  # 100 lines
        schema = {
            "schema_version": 1,
            "max_lines": 50
        }

        evaluator = RuleEvaluator(schema)
        errors = evaluator.evaluate(Path("test.md"), content)

        self.assertEqual(len(errors), 1)
        self.assertEqual(errors[0].rule_violated, "max_lines")
        self.assertEqual(errors[0].line_number, 0)
        self.assertIn("100 lines", errors[0].found)
        self.assertIn("Maximum 50 lines", errors[0].expected)

    def test_max_lines_exactly_at_limit(self):
        """Test max_lines passes when exactly at limit."""
        content = "\n".join([f"Line {i}" for i in range(1, 51)])  # 50 lines
        schema = {
            "schema_version": 1,
            "max_lines": 50
        }

        evaluator = RuleEvaluator(schema)
        errors = evaluator.evaluate(Path("test.md"), content)

        self.assertEqual(len(errors), 0)

    def test_max_lines_empty_document(self):
        """Test max_lines handles empty document."""
        content = ""
        schema = {
            "schema_version": 1,
            "max_lines": 10
        }

        evaluator = RuleEvaluator(schema)
        errors = evaluator.evaluate(Path("test.md"), content)

        self.assertEqual(len(errors), 0)


class TestForbiddenRule(unittest.TestCase):
    """Test forbidden rule with pattern detection and line numbers."""

    def test_forbidden_no_matches(self):
        """Test forbidden passes when no patterns match."""
        content = "# Title\n\nThis is clean content."
        schema = {
            "schema_version": 1,
            "forbidden": [
                {"pattern": "(?i)how to", "reason": "how-to instructions", "severity": "error"}
            ]
        }

        evaluator = RuleEvaluator(schema)
        errors = evaluator.evaluate(Path("test.md"), content)

        self.assertEqual(len(errors), 0)

    def test_forbidden_single_match(self):
        """Test forbidden detects single pattern match."""
        content = "# Title\n\nThis shows how to do something."
        schema = {
            "schema_version": 1,
            "forbidden": [
                {"pattern": "(?i)how to", "reason": "how-to instructions", "severity": "error"}
            ]
        }

        evaluator = RuleEvaluator(schema)
        errors = evaluator.evaluate(Path("test.md"), content)

        self.assertEqual(len(errors), 1)
        self.assertEqual(errors[0].rule_violated, "forbidden")
        self.assertEqual(errors[0].line_number, 3)
        self.assertIn("how-to instructions", errors[0].detail)
        self.assertIn("how to", errors[0].found.lower())

    def test_forbidden_multiple_matches_same_pattern(self):
        """Test forbidden detects multiple matches of same pattern."""
        content = "# Title\n\nHow to step 1.\n\nHow to step 2."
        schema = {
            "schema_version": 1,
            "forbidden": [
                {"pattern": "(?i)how to", "reason": "how-to instructions", "severity": "error"}
            ]
        }

        evaluator = RuleEvaluator(schema)
        errors = evaluator.evaluate(Path("test.md"), content)

        self.assertEqual(len(errors), 2)
        self.assertEqual(errors[0].line_number, 3)
        self.assertEqual(errors[1].line_number, 5)

    def test_forbidden_multiple_patterns(self):
        """Test forbidden with multiple forbidden patterns."""
        content = "# Title\n\nThis describes the feature.\n\nThis contains information."
        schema = {
            "schema_version": 1,
            "forbidden": [
                {"pattern": "(?i)describes", "reason": "explanatory content", "severity": "error"},
                {"pattern": "(?i)contains information", "reason": "vague language", "severity": "warn"}
            ]
        }

        evaluator = RuleEvaluator(schema)
        errors = evaluator.evaluate(Path("test.md"), content)

        self.assertEqual(len(errors), 2)
        self.assertEqual(errors[0].rule_violated, "forbidden")
        self.assertEqual(errors[0].severity, "error")
        self.assertEqual(errors[1].severity, "warn")

    def test_forbidden_line_numbers_accurate(self):
        """Test forbidden reports accurate line numbers."""
        content = "Line 1\nLine 2\nLine 3 has forbidden pattern\nLine 4"
        schema = {
            "schema_version": 1,
            "forbidden": [
                {"pattern": "forbidden", "reason": "test pattern", "severity": "error"}
            ]
        }

        evaluator = RuleEvaluator(schema)
        errors = evaluator.evaluate(Path("test.md"), content)

        self.assertEqual(len(errors), 1)
        self.assertEqual(errors[0].line_number, 3)

    def test_forbidden_case_insensitive(self):
        """Test forbidden pattern is case-insensitive."""
        content = "# Title\n\nHOW TO do something."
        schema = {
            "schema_version": 1,
            "forbidden": [
                {"pattern": "(?i)how to", "reason": "how-to instructions", "severity": "error"}
            ]
        }

        evaluator = RuleEvaluator(schema)
        errors = evaluator.evaluate(Path("test.md"), content)

        self.assertEqual(len(errors), 1)

    def test_forbidden_invalid_regex(self):
        """Test forbidden handles invalid regex gracefully."""
        content = "# Title\n\nValid content."
        schema = {
            "schema_version": 1,
            "forbidden": [
                {"pattern": "[Invalid(Regex", "reason": "test", "severity": "error"}
            ]
        }

        evaluator = RuleEvaluator(schema)
        errors = evaluator.evaluate(Path("test.md"), content)

        self.assertEqual(len(errors), 1)
        self.assertEqual(errors[0].rule_violated, "forbidden")
        self.assertIn("Invalid regex pattern", errors[0].detail)


class TestFilenamePatternRule(unittest.TestCase):
    """Test filename_pattern rule."""

    def test_filename_pattern_match(self):
        """Test filename_pattern passes when filename matches."""
        content = "# Title\n\nContent."
        schema = {
            "schema_version": 1,
            "filename_pattern": "^[a-z-]+\\.md$"
        }

        evaluator = RuleEvaluator(schema)
        errors = evaluator.evaluate(Path("test-file.md"), content)

        self.assertEqual(len(errors), 0)

    def test_filename_pattern_no_match(self):
        """Test filename_pattern fails when filename doesn't match."""
        content = "# Title\n\nContent."
        schema = {
            "schema_version": 1,
            "filename_pattern": "^[a-z-]+\\.md$"
        }

        evaluator = RuleEvaluator(schema)
        errors = evaluator.evaluate(Path("TestFile.md"), content)

        self.assertEqual(len(errors), 1)
        self.assertEqual(errors[0].rule_violated, "filename_pattern")
        self.assertIn("TestFile.md", errors[0].found)

    def test_filename_pattern_invalid_regex(self):
        """Test filename_pattern handles invalid regex gracefully."""
        content = "# Title\n\nContent."
        schema = {
            "schema_version": 1,
            "filename_pattern": "[Invalid(Regex"
        }

        evaluator = RuleEvaluator(schema)
        errors = evaluator.evaluate(Path("test.md"), content)

        self.assertEqual(len(errors), 1)
        self.assertEqual(errors[0].rule_violated, "filename_pattern")
        self.assertIn("Invalid regex pattern", errors[0].detail)


class TestRuleEvaluatorIntegration(unittest.TestCase):
    """Test RuleEvaluator integration and multiple rules."""

    def test_multiple_rules_all_pass(self):
        """Test document passing all validation rules."""
        content = "# Documentation Guide\n\nClean content here.\n\nMore content."
        schema = {
            "schema_version": 1,
            "title_pattern": "^# .+ Guide$",
            "max_lines": 10,
            "forbidden": [
                {"pattern": "(?i)how to", "reason": "how-to instructions", "severity": "error"}
            ],
            "filename_pattern": "^[a-z-]+\\.md$"
        }

        evaluator = RuleEvaluator(schema)
        errors = evaluator.evaluate(Path("test-doc.md"), content)

        self.assertEqual(len(errors), 0)

    def test_multiple_rules_multiple_failures(self):
        """Test document failing multiple validation rules."""
        content = "\n".join([f"# Wrong Title\n\nHow to do something."] + [f"Line {i}" for i in range(50)])
        schema = {
            "schema_version": 1,
            "title_pattern": "^# .+ Guide$",
            "max_lines": 10,
            "forbidden": [
                {"pattern": "(?i)how to", "reason": "how-to instructions", "severity": "error"}
            ]
        }

        evaluator = RuleEvaluator(schema)
        errors = evaluator.evaluate(Path("test.md"), content)

        self.assertEqual(len(errors), 3)
        rule_names = [e.rule_violated for e in errors]
        self.assertIn("title_pattern", rule_names)
        self.assertIn("max_lines", rule_names)
        self.assertIn("forbidden", rule_names)

    def test_empty_schema(self):
        """Test evaluator with empty schema (no rules)."""
        content = "# Any Title\n\nAny content.\n" * 100
        schema = {
            "schema_version": 1
        }

        evaluator = RuleEvaluator(schema)
        errors = evaluator.evaluate(Path("test.md"), content)

        self.assertEqual(len(errors), 0)

    def test_validate_document_convenience_function(self):
        """Test validate_document convenience function."""
        with tempfile.NamedTemporaryFile(mode='w', suffix='.md', delete=False) as f:
            f.write("# Test Guide\n\nContent.")
            temp_path = Path(f.name)

        try:
            schema = {
                "schema_version": 1,
                "title_pattern": "^# .+ Guide$"
            }

            errors = validate_document(temp_path, schema)
            self.assertEqual(len(errors), 0)
        finally:
            temp_path.unlink()


class TestErrorMessageFormat(unittest.TestCase):
    """Test error message format includes all required information."""

    def test_error_includes_file_path(self):
        """Test error message includes document path."""
        content = "# Wrong Title\n\nContent."
        schema = {
            "schema_version": 1,
            "title_pattern": "^# .+ Guide$"
        }

        evaluator = RuleEvaluator(schema)
        errors = evaluator.evaluate(Path("doc/test/example.md"), content)

        self.assertEqual(len(errors), 1)
        self.assertIn("doc/test/example.md", errors[0].file_path)

    def test_error_includes_line_number(self):
        """Test error message includes line number."""
        content = "Line 1\nLine 2\nLine 3 with how to\nLine 4"
        schema = {
            "schema_version": 1,
            "forbidden": [
                {"pattern": "(?i)how to", "reason": "how-to instructions", "severity": "error"}
            ]
        }

        evaluator = RuleEvaluator(schema)
        errors = evaluator.evaluate(Path("test.md"), content)

        self.assertEqual(len(errors), 1)
        self.assertEqual(errors[0].line_number, 3)

    def test_error_includes_rule_violated(self):
        """Test error message includes rule name."""
        content = "# Wrong Title\n\nContent."
        schema = {
            "schema_version": 1,
            "title_pattern": "^# .+ Guide$"
        }

        evaluator = RuleEvaluator(schema)
        errors = evaluator.evaluate(Path("test.md"), content)

        self.assertEqual(len(errors), 1)
        self.assertEqual(errors[0].rule_violated, "title_pattern")


if __name__ == '__main__':
    unittest.main()
