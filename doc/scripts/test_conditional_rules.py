#!/usr/bin/env python3
"""
Tests for conditional logic in rule_evaluator.py - Milestone 5.

Test coverage:
- Conditions block properly parsed and evaluated
- require_if skips rules when condition false
- require_if applies rules when condition true
- forbid_if inverts logic correctly
- All section rule types implemented and tested
- File rules correctly detect missing entries
- 15+ conditional logic scenarios
"""

import unittest
import tempfile
from pathlib import Path
from rule_evaluator import RuleEvaluator, ValidationError
from predicates import PredicateContext


class TestConditionEvaluation(unittest.TestCase):
    """Test condition evaluation from validation schema."""

    def test_conditions_parsed_and_evaluated(self):
        """Test conditions block is properly parsed and evaluated."""
        # Create a temporary directory with test files
        with tempfile.TemporaryDirectory() as tmpdir:
            tmpdir = Path(tmpdir)

            # Create README.md
            readme = tmpdir / "README.md"
            readme.write_text("# README\n\nProject readme.")

            # Create test document
            doc_path = tmpdir / "CLAUDE.md"
            doc_content = "# CLAUDE.md Guide\n\nContent here."

            schema = {
                "schema_version": 1,
                "conditions": {
                    "readme_exists": 'file_exists("README.md")',
                }
            }

            ctx = PredicateContext(doc_path)
            evaluator = RuleEvaluator(schema, predicate_context=ctx)
            errors = evaluator.evaluate(doc_path, doc_content)

            # Should have no errors
            self.assertEqual(len(errors), 0)

            # Condition should be evaluated and stored
            self.assertIn("readme_exists", evaluator.condition_results)
            self.assertTrue(evaluator.condition_results["readme_exists"])

    def test_multiple_conditions_evaluated(self):
        """Test multiple conditions are all evaluated."""
        with tempfile.TemporaryDirectory() as tmpdir:
            tmpdir = Path(tmpdir)

            # Create test files
            (tmpdir / "README.md").write_text("# README")
            (tmpdir / "doc1.md").write_text("# Doc 1")
            (tmpdir / "doc2.md").write_text("# Doc 2")

            doc_path = tmpdir / "CLAUDE.md"
            doc_content = "# CLAUDE.md Guide\n\nContent."

            schema = {
                "schema_version": 1,
                "conditions": {
                    "readme_exists": 'file_exists("README.md")',
                    "has_md_files": 'md_files_exist(exclude=["README.md", "CLAUDE.md"])',
                }
            }

            ctx = PredicateContext(doc_path)
            evaluator = RuleEvaluator(schema, predicate_context=ctx)
            errors = evaluator.evaluate(doc_path, doc_content)

            self.assertEqual(len(errors), 0)
            self.assertTrue(evaluator.condition_results["readme_exists"])
            self.assertTrue(evaluator.condition_results["has_md_files"])

    def test_condition_evaluation_error_handling(self):
        """Test condition evaluation handles errors gracefully."""
        with tempfile.TemporaryDirectory() as tmpdir:
            tmpdir = Path(tmpdir)

            doc_path = tmpdir / "test.md"
            doc_content = "# Test\n\nContent."

            schema = {
                "schema_version": 1,
                "conditions": {
                    "invalid": 'unknown_predicate("test")',
                }
            }

            ctx = PredicateContext(doc_path)
            evaluator = RuleEvaluator(schema, predicate_context=ctx)
            errors = evaluator.evaluate(doc_path, doc_content)

            # Should have an error about invalid predicate
            self.assertEqual(len(errors), 1)
            self.assertEqual(errors[0].rule_violated, "conditions")
            self.assertIn("invalid", errors[0].detail)


class TestRequireIfLogic(unittest.TestCase):
    """Test require_if conditional logic."""

    def test_require_if_condition_true_section_exists(self):
        """Scenario 3: Section required conditionally - condition true, exists -> pass."""
        with tempfile.TemporaryDirectory() as tmpdir:
            tmpdir = Path(tmpdir)

            # Create README.md to make condition true
            (tmpdir / "README.md").write_text("# README")

            doc_path = tmpdir / "CLAUDE.md"
            doc_content = """# CLAUDE.md Guide

## Hub

**`README.md`** - Read when you need project overview
"""

            schema = {
                "schema_version": 1,
                "conditions": {
                    "readme_exists": 'file_exists("README.md")',
                },
                "required_sections": [
                    {
                        "name": "Hub",
                        "require_if": "readme_exists",
                    }
                ]
            }

            ctx = PredicateContext(doc_path)
            evaluator = RuleEvaluator(schema, predicate_context=ctx)
            errors = evaluator.evaluate(doc_path, doc_content)

            # Should pass - condition true and section exists
            self.assertEqual(len(errors), 0)

    def test_require_if_condition_true_section_missing(self):
        """Scenario 4: Section required conditionally - condition true, missing -> error."""
        with tempfile.TemporaryDirectory() as tmpdir:
            tmpdir = Path(tmpdir)

            # Create README.md to make condition true
            (tmpdir / "README.md").write_text("# README")

            doc_path = tmpdir / "CLAUDE.md"
            doc_content = """# CLAUDE.md Guide

## Purpose

This is the purpose section.
"""

            schema = {
                "schema_version": 1,
                "conditions": {
                    "readme_exists": 'file_exists("README.md")',
                },
                "required_sections": [
                    {
                        "name": "Hub",
                        "must_exist": True,
                        "require_if": "readme_exists",
                    }
                ]
            }

            ctx = PredicateContext(doc_path)
            evaluator = RuleEvaluator(schema, predicate_context=ctx)
            errors = evaluator.evaluate(doc_path, doc_content)

            # Should fail - condition true but section missing
            self.assertEqual(len(errors), 1)
            self.assertEqual(errors[0].rule_violated, "required_sections")
            self.assertIn("Hub", errors[0].detail)

    def test_require_if_condition_false_section_missing(self):
        """Scenario 5: Section required conditionally - condition false, missing -> pass."""
        with tempfile.TemporaryDirectory() as tmpdir:
            tmpdir = Path(tmpdir)

            # No README.md, condition is false

            doc_path = tmpdir / "CLAUDE.md"
            doc_content = """# CLAUDE.md Guide

## Purpose

This is the purpose section.
"""

            schema = {
                "schema_version": 1,
                "conditions": {
                    "readme_exists": 'file_exists("README.md")',
                },
                "required_sections": [
                    {
                        "name": "Hub",
                        "must_exist": True,
                        "require_if": "readme_exists",
                    }
                ]
            }

            ctx = PredicateContext(doc_path)
            evaluator = RuleEvaluator(schema, predicate_context=ctx)
            errors = evaluator.evaluate(doc_path, doc_content)

            # Should pass - condition false, rule skipped
            self.assertEqual(len(errors), 0)


class TestForbidIfLogic(unittest.TestCase):
    """Test forbid_if conditional logic."""

    def test_forbid_if_condition_true_section_exists(self):
        """Scenario 6: Section forbidden conditionally - condition true, exists -> error."""
        with tempfile.TemporaryDirectory() as tmpdir:
            tmpdir = Path(tmpdir)

            # Create condition that makes section forbidden
            (tmpdir / "README.md").write_text("# README")

            doc_path = tmpdir / "CLAUDE.md"
            doc_content = """# CLAUDE.md Guide

## Hub

This section should not exist.
"""

            schema = {
                "schema_version": 1,
                "conditions": {
                    "readme_exists": 'file_exists("README.md")',
                },
                "required_sections": [
                    {
                        "name": "Hub",
                        "must_not_exist": True,
                        "forbid_if": "readme_exists",
                    }
                ]
            }

            ctx = PredicateContext(doc_path)
            evaluator = RuleEvaluator(schema, predicate_context=ctx)
            errors = evaluator.evaluate(doc_path, doc_content)

            # Should fail - condition true and forbidden section exists
            self.assertEqual(len(errors), 1)
            self.assertIn("Hub", errors[0].detail)
            self.assertIn("forbidden", errors[0].detail.lower())

    def test_forbid_if_condition_false_section_exists(self):
        """Scenario 7: Section forbidden conditionally - condition false, exists -> pass."""
        with tempfile.TemporaryDirectory() as tmpdir:
            tmpdir = Path(tmpdir)

            # No README.md, condition is false

            doc_path = tmpdir / "CLAUDE.md"
            doc_content = """# CLAUDE.md Guide

## Hub

This section is allowed when condition is false.
"""

            schema = {
                "schema_version": 1,
                "conditions": {
                    "readme_exists": 'file_exists("README.md")',
                },
                "required_sections": [
                    {
                        "name": "Hub",
                        "forbid_if": "readme_exists",
                    }
                ]
            }

            ctx = PredicateContext(doc_path)
            evaluator = RuleEvaluator(schema, predicate_context=ctx)
            errors = evaluator.evaluate(doc_path, doc_content)

            # Should pass - condition false, rule not applied
            self.assertEqual(len(errors), 0)


class TestUnconditionalSectionRules(unittest.TestCase):
    """Test unconditional section requirements."""

    def test_section_required_unconditionally_exists(self):
        """Scenario 1: Section required unconditionally - exists -> pass."""
        with tempfile.TemporaryDirectory() as tmpdir:
            tmpdir = Path(tmpdir)

            doc_path = tmpdir / "test.md"
            doc_content = """# Test Document

## Purpose

This is the purpose section.
"""

            schema = {
                "schema_version": 1,
                "required_sections": [
                    {
                        "name": "Purpose",
                        "must_exist": True,
                    }
                ]
            }

            ctx = PredicateContext(doc_path)
            evaluator = RuleEvaluator(schema, predicate_context=ctx)
            errors = evaluator.evaluate(doc_path, doc_content)

            self.assertEqual(len(errors), 0)

    def test_section_required_unconditionally_missing(self):
        """Scenario 2: Section required unconditionally - missing -> error."""
        with tempfile.TemporaryDirectory() as tmpdir:
            tmpdir = Path(tmpdir)

            doc_path = tmpdir / "test.md"
            doc_content = """# Test Document

## Other Section

Some content.
"""

            schema = {
                "schema_version": 1,
                "required_sections": [
                    {
                        "name": "Purpose",
                        "must_exist": True,
                    }
                ]
            }

            ctx = PredicateContext(doc_path)
            evaluator = RuleEvaluator(schema, predicate_context=ctx)
            errors = evaluator.evaluate(doc_path, doc_content)

            self.assertEqual(len(errors), 1)
            self.assertEqual(errors[0].rule_violated, "required_sections")
            self.assertIn("Purpose", errors[0].detail)


class TestSectionContentRules(unittest.TestCase):
    """Test section content validation rules."""

    def test_max_paragraphs_within_limit(self):
        """Scenario 8: Max paragraphs - within limit -> pass."""
        with tempfile.TemporaryDirectory() as tmpdir:
            tmpdir = Path(tmpdir)

            doc_path = tmpdir / "test.md"
            doc_content = """# Test Document

## Purpose

First paragraph.

Second paragraph.
"""

            schema = {
                "schema_version": 1,
                "required_sections": [
                    {
                        "name": "Purpose",
                        "max_paragraphs": 3,
                    }
                ]
            }

            ctx = PredicateContext(doc_path)
            evaluator = RuleEvaluator(schema, predicate_context=ctx)
            errors = evaluator.evaluate(doc_path, doc_content)

            self.assertEqual(len(errors), 0)

    def test_max_paragraphs_exceeds_limit(self):
        """Scenario 9: Max paragraphs - exceeds limit -> error."""
        with tempfile.TemporaryDirectory() as tmpdir:
            tmpdir = Path(tmpdir)

            doc_path = tmpdir / "test.md"
            doc_content = """# Test Document

## Purpose

First paragraph.

Second paragraph.

Third paragraph.

Fourth paragraph.
"""

            schema = {
                "schema_version": 1,
                "required_sections": [
                    {
                        "name": "Purpose",
                        "max_paragraphs": 2,
                    }
                ]
            }

            ctx = PredicateContext(doc_path)
            evaluator = RuleEvaluator(schema, predicate_context=ctx)
            errors = evaluator.evaluate(doc_path, doc_content)

            self.assertEqual(len(errors), 1)
            self.assertIn("paragraph limit", errors[0].detail)

    def test_content_pattern_matches(self):
        """Scenario 10: Content pattern - matches -> pass."""
        with tempfile.TemporaryDirectory() as tmpdir:
            tmpdir = Path(tmpdir)

            doc_path = tmpdir / "CLAUDE.md"
            doc_content = """# CLAUDE.md Guide

## Hub

**`README.md`** - Read when you need project overview
"""

            schema = {
                "schema_version": 1,
                "required_sections": [
                    {
                        "name": "Hub",
                        "content_pattern": r'\*\*`README\.md`\*\* - Read when',
                    }
                ]
            }

            ctx = PredicateContext(doc_path)
            evaluator = RuleEvaluator(schema, predicate_context=ctx)
            errors = evaluator.evaluate(doc_path, doc_content)

            self.assertEqual(len(errors), 0)

    def test_content_pattern_no_match(self):
        """Scenario 11: Content pattern - no match -> error."""
        with tempfile.TemporaryDirectory() as tmpdir:
            tmpdir = Path(tmpdir)

            doc_path = tmpdir / "CLAUDE.md"
            doc_content = """# CLAUDE.md Guide

## Hub

Some other content without the pattern.
"""

            schema = {
                "schema_version": 1,
                "required_sections": [
                    {
                        "name": "Hub",
                        "content_pattern": r'\*\*`README\.md`\*\* - Read when',
                    }
                ]
            }

            ctx = PredicateContext(doc_path)
            evaluator = RuleEvaluator(schema, predicate_context=ctx)
            errors = evaluator.evaluate(doc_path, doc_content)

            self.assertEqual(len(errors), 1)
            self.assertIn("does not match required pattern", errors[0].detail)


class TestFilesRules(unittest.TestCase):
    """Test files_rules validation."""

    def test_files_rules_all_files_listed(self):
        """Scenario 12: Files rules - all files listed -> pass."""
        with tempfile.TemporaryDirectory() as tmpdir:
            tmpdir = Path(tmpdir)

            # Create test markdown files
            (tmpdir / "README.md").write_text("# README")
            (tmpdir / "doc1.md").write_text("# Doc 1")
            (tmpdir / "doc2.md").write_text("# Doc 2")

            doc_path = tmpdir / "CLAUDE.md"
            doc_content = """# CLAUDE.md Guide

## Files

- **`doc1.md`** - Documentation file 1
- **`doc2.md`** - Documentation file 2
"""

            schema = {
                "schema_version": 1,
                "required_sections": [
                    {
                        "name": "Files",
                        "files_rules": {
                            "must_list_all_md": True,
                            "exclude_globs": ["README.md", "CLAUDE.md"],
                        }
                    }
                ]
            }

            ctx = PredicateContext(doc_path)
            evaluator = RuleEvaluator(schema, predicate_context=ctx)
            errors = evaluator.evaluate(doc_path, doc_content)

            self.assertEqual(len(errors), 0)

    def test_files_rules_missing_file(self):
        """Scenario 13: Files rules - missing file -> error."""
        with tempfile.TemporaryDirectory() as tmpdir:
            tmpdir = Path(tmpdir)

            # Create test markdown files
            (tmpdir / "README.md").write_text("# README")
            (tmpdir / "doc1.md").write_text("# Doc 1")
            (tmpdir / "doc2.md").write_text("# Doc 2")

            doc_path = tmpdir / "CLAUDE.md"
            doc_content = """# CLAUDE.md Guide

## Files

- **`doc1.md`** - Documentation file 1
"""

            schema = {
                "schema_version": 1,
                "required_sections": [
                    {
                        "name": "Files",
                        "files_rules": {
                            "must_list_all_md": True,
                            "exclude_globs": ["README.md", "CLAUDE.md"],
                        }
                    }
                ]
            }

            ctx = PredicateContext(doc_path)
            evaluator = RuleEvaluator(schema, predicate_context=ctx)
            errors = evaluator.evaluate(doc_path, doc_content)

            # Should have error for missing doc2.md
            self.assertEqual(len(errors), 1)
            self.assertIn("doc2.md", errors[0].expected)

    def test_files_rules_exclude_patterns_work(self):
        """Scenario 14: Files rules - exclude patterns work -> pass."""
        with tempfile.TemporaryDirectory() as tmpdir:
            tmpdir = Path(tmpdir)

            # Create test markdown files
            (tmpdir / "README.md").write_text("# README")
            (tmpdir / "CLAUDE.md").write_text("# CLAUDE")
            (tmpdir / "doc1.md").write_text("# Doc 1")

            doc_path = tmpdir / "CLAUDE.md"
            doc_content = """# CLAUDE.md Guide

## Files

- **`doc1.md`** - Documentation file 1
"""

            schema = {
                "schema_version": 1,
                "required_sections": [
                    {
                        "name": "Files",
                        "files_rules": {
                            "must_list_all_md": True,
                            "exclude_globs": ["README.md", "CLAUDE.md"],
                        }
                    }
                ]
            }

            ctx = PredicateContext(doc_path)
            evaluator = RuleEvaluator(schema, predicate_context=ctx)
            errors = evaluator.evaluate(doc_path, doc_content)

            # Should pass - README.md and CLAUDE.md are excluded
            self.assertEqual(len(errors), 0)

    def test_entry_pattern_validation(self):
        """Test entry_pattern validates list item format."""
        with tempfile.TemporaryDirectory() as tmpdir:
            tmpdir = Path(tmpdir)

            (tmpdir / "doc1.md").write_text("# Doc 1")

            doc_path = tmpdir / "CLAUDE.md"
            doc_content = """# CLAUDE.md Guide

## Files

- **`doc1.md`** - Read when you need doc1
"""

            schema = {
                "schema_version": 1,
                "required_sections": [
                    {
                        "name": "Files",
                        "files_rules": {
                            "entry_pattern": r'^\*\*`.+\.md`\*\* - Read when',
                        }
                    }
                ]
            }

            ctx = PredicateContext(doc_path)
            evaluator = RuleEvaluator(schema, predicate_context=ctx)
            errors = evaluator.evaluate(doc_path, doc_content)

            # Should pass - entry matches pattern
            self.assertEqual(len(errors), 0)

    def test_entry_pattern_validation_fails(self):
        """Test entry_pattern detects incorrectly formatted entries."""
        with tempfile.TemporaryDirectory() as tmpdir:
            tmpdir = Path(tmpdir)

            (tmpdir / "doc1.md").write_text("# Doc 1")

            doc_path = tmpdir / "CLAUDE.md"
            doc_content = """# CLAUDE.md Guide

## Files

- doc1.md - Wrong format
"""

            schema = {
                "schema_version": 1,
                "required_sections": [
                    {
                        "name": "Files",
                        "files_rules": {
                            "entry_pattern": r'^\*\*`.+\.md`\*\* - Read when',
                        }
                    }
                ]
            }

            ctx = PredicateContext(doc_path)
            evaluator = RuleEvaluator(schema, predicate_context=ctx)
            errors = evaluator.evaluate(doc_path, doc_content)

            # Should fail - entry doesn't match pattern
            self.assertEqual(len(errors), 1)
            self.assertIn("incorrectly formatted entry", errors[0].detail)


class TestMultipleConditionsCombined(unittest.TestCase):
    """Test multiple conditions combined in complex scenarios."""

    def test_multiple_conditions_combined(self):
        """Scenario 15: Multiple conditions combined -> correct evaluation."""
        with tempfile.TemporaryDirectory() as tmpdir:
            tmpdir = Path(tmpdir)

            # Create files for conditions
            (tmpdir / "README.md").write_text("# README")
            (tmpdir / "doc1.md").write_text("# Doc 1")

            doc_path = tmpdir / "CLAUDE.md"
            doc_content = """# CLAUDE.md Guide

## Purpose

Project overview.

## Hub

**`README.md`** - Read when you need overview

## Files

- **`doc1.md`** - Documentation file
"""

            schema = {
                "schema_version": 1,
                "conditions": {
                    "readme_exists": 'file_exists("README.md")',
                    "has_md_files": 'md_files_exist(exclude=["README.md", "CLAUDE.md"])',
                },
                "required_sections": [
                    {
                        "name": "Purpose",
                        "must_exist": True,
                        "max_paragraphs": 3,
                    },
                    {
                        "name": "Hub",
                        "require_if": "readme_exists",
                        "content_pattern": r'\*\*`README\.md`\*\*',
                    },
                    {
                        "name": "Files",
                        "require_if": "has_md_files",
                        "files_rules": {
                            "must_list_all_md": True,
                            "exclude_globs": ["README.md", "CLAUDE.md"],
                            "entry_pattern": r'^\*\*`.+\.md`\*\*',
                        }
                    }
                ]
            }

            ctx = PredicateContext(doc_path)
            evaluator = RuleEvaluator(schema, predicate_context=ctx)
            errors = evaluator.evaluate(doc_path, doc_content)

            # All conditions true, all rules pass
            self.assertEqual(len(errors), 0)
            self.assertTrue(evaluator.condition_results["readme_exists"])
            self.assertTrue(evaluator.condition_results["has_md_files"])


class TestMaxSentences(unittest.TestCase):
    """Test max_sentences section rule."""

    def test_max_sentences_within_limit(self):
        """Test max_sentences passes when within limit."""
        with tempfile.TemporaryDirectory() as tmpdir:
            tmpdir = Path(tmpdir)

            doc_path = tmpdir / "test.md"
            doc_content = """# Test Document

## Purpose

First sentence. Second sentence.
"""

            schema = {
                "schema_version": 1,
                "required_sections": [
                    {
                        "name": "Purpose",
                        "max_sentences": 3,
                    }
                ]
            }

            ctx = PredicateContext(doc_path)
            evaluator = RuleEvaluator(schema, predicate_context=ctx)
            errors = evaluator.evaluate(doc_path, doc_content)

            self.assertEqual(len(errors), 0)

    def test_max_sentences_exceeds_limit(self):
        """Test max_sentences fails when exceeds limit."""
        with tempfile.TemporaryDirectory() as tmpdir:
            tmpdir = Path(tmpdir)

            doc_path = tmpdir / "test.md"
            doc_content = """# Test Document

## Purpose

First sentence. Second sentence. Third sentence. Fourth sentence.
"""

            schema = {
                "schema_version": 1,
                "required_sections": [
                    {
                        "name": "Purpose",
                        "max_sentences": 2,
                    }
                ]
            }

            ctx = PredicateContext(doc_path)
            evaluator = RuleEvaluator(schema, predicate_context=ctx)
            errors = evaluator.evaluate(doc_path, doc_content)

            self.assertEqual(len(errors), 1)
            self.assertIn("sentence limit", errors[0].detail)


if __name__ == '__main__':
    unittest.main()
