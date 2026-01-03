#!/usr/bin/env python3
"""
Test suite for validation schema validation.

Tests the JSON Schema that validates validation blocks in template frontmatter.
This is meta-validation: validating the validators themselves.
"""

import json
import pytest
from pathlib import Path
from jsonschema import validate, ValidationError, Draft7Validator


# Load schema once at module level
SCHEMA_PATH = Path(__file__).parent / "validation_schema.json"
with open(SCHEMA_PATH) as f:
    VALIDATION_SCHEMA = json.load(f)


def test_schema_is_valid_json_schema():
    """Validation schema itself must be a valid JSON Schema Draft 7."""
    Draft7Validator.check_schema(VALIDATION_SCHEMA)


def test_minimal_valid_validation_block():
    """Minimal validation block with only schema_version."""
    validation_block = {
        "schema_version": 1
    }
    validate(instance=validation_block, schema=VALIDATION_SCHEMA)


def test_schema_version_required():
    """schema_version field is required."""
    validation_block = {}
    with pytest.raises(ValidationError) as exc_info:
        validate(instance=validation_block, schema=VALIDATION_SCHEMA)
    assert "schema_version" in str(exc_info.value).lower()


def test_schema_version_must_be_integer():
    """schema_version must be an integer."""
    validation_block = {
        "schema_version": "1"
    }
    with pytest.raises(ValidationError):
        validate(instance=validation_block, schema=VALIDATION_SCHEMA)


def test_schema_version_must_be_one():
    """schema_version must be exactly 1."""
    validation_block = {
        "schema_version": 2
    }
    with pytest.raises(ValidationError):
        validate(instance=validation_block, schema=VALIDATION_SCHEMA)


def test_extends_with_valid_templates():
    """extends field accepts array of template filenames."""
    validation_block = {
        "schema_version": 1,
        "extends": ["base-template.md", "common-rules.md"]
    }
    validate(instance=validation_block, schema=VALIDATION_SCHEMA)


def test_extends_must_be_array():
    """extends must be an array."""
    validation_block = {
        "schema_version": 1,
        "extends": "base-template.md"
    }
    with pytest.raises(ValidationError):
        validate(instance=validation_block, schema=VALIDATION_SCHEMA)


def test_extends_rejects_duplicates():
    """extends array must have unique items."""
    validation_block = {
        "schema_version": 1,
        "extends": ["base.md", "base.md"]
    }
    with pytest.raises(ValidationError):
        validate(instance=validation_block, schema=VALIDATION_SCHEMA)


def test_conditions_with_valid_predicates():
    """conditions accepts named predicate expressions."""
    validation_block = {
        "schema_version": 1,
        "conditions": {
            "readme_exists": "file_exists('README.md')",
            "has_md_files": "md_files_exist(exclude=['CLAUDE.md'])",
            "section_present": "section_present('Introduction')"
        }
    }
    validate(instance=validation_block, schema=VALIDATION_SCHEMA)


def test_conditions_key_must_be_valid_identifier():
    """condition keys must be valid identifiers (lowercase, underscore)."""
    validation_block = {
        "schema_version": 1,
        "conditions": {
            "Invalid-Key": "file_exists('test.md')"
        }
    }
    with pytest.raises(ValidationError):
        validate(instance=validation_block, schema=VALIDATION_SCHEMA)


def test_conditions_value_must_be_function_call():
    """condition values must be function call expressions."""
    validation_block = {
        "schema_version": 1,
        "conditions": {
            "invalid": "just_a_string"
        }
    }
    with pytest.raises(ValidationError):
        validate(instance=validation_block, schema=VALIDATION_SCHEMA)


def test_title_pattern():
    """title_pattern accepts regex string."""
    validation_block = {
        "schema_version": 1,
        "title_pattern": "^# .+ Guide for LLM Agents$"
    }
    validate(instance=validation_block, schema=VALIDATION_SCHEMA)


def test_max_lines():
    """max_lines accepts positive integer."""
    validation_block = {
        "schema_version": 1,
        "max_lines": 50
    }
    validate(instance=validation_block, schema=VALIDATION_SCHEMA)


def test_max_lines_rejects_zero():
    """max_lines must be at least 1."""
    validation_block = {
        "schema_version": 1,
        "max_lines": 0
    }
    with pytest.raises(ValidationError):
        validate(instance=validation_block, schema=VALIDATION_SCHEMA)


def test_forbidden_patterns():
    """forbidden accepts array of pattern objects."""
    validation_block = {
        "schema_version": 1,
        "forbidden": [
            {
                "pattern": "(?i)how to",
                "reason": "how-to instructions",
                "severity": "error"
            },
            {
                "pattern": "(?i)step 1",
                "reason": "step-by-step procedures"
            }
        ]
    }
    validate(instance=validation_block, schema=VALIDATION_SCHEMA)


def test_forbidden_pattern_requires_reason():
    """forbidden pattern must include reason."""
    validation_block = {
        "schema_version": 1,
        "forbidden": [
            {
                "pattern": "(?i)how to"
            }
        ]
    }
    with pytest.raises(ValidationError):
        validate(instance=validation_block, schema=VALIDATION_SCHEMA)


def test_forbidden_severity_enum():
    """forbidden severity must be 'error' or 'warn'."""
    validation_block = {
        "schema_version": 1,
        "forbidden": [
            {
                "pattern": "test",
                "reason": "test reason",
                "severity": "critical"
            }
        ]
    }
    with pytest.raises(ValidationError):
        validate(instance=validation_block, schema=VALIDATION_SCHEMA)


def test_required_sections_basic():
    """required_sections accepts array of section objects."""
    validation_block = {
        "schema_version": 1,
        "required_sections": [
            {
                "name": "Purpose",
                "must_exist": True
            },
            {
                "name": "Context",
                "must_exist": True
            }
        ]
    }
    validate(instance=validation_block, schema=VALIDATION_SCHEMA)


def test_required_section_needs_constraint():
    """required_section must have must_exist, require_if, or forbid_if."""
    validation_block = {
        "schema_version": 1,
        "required_sections": [
            {
                "name": "Purpose"
            }
        ]
    }
    with pytest.raises(ValidationError):
        validate(instance=validation_block, schema=VALIDATION_SCHEMA)


def test_required_section_with_require_if():
    """required_section can use require_if with condition name."""
    validation_block = {
        "schema_version": 1,
        "conditions": {
            "readme_exists": "file_exists('README.md')"
        },
        "required_sections": [
            {
                "name": "Hub",
                "require_if": "readme_exists"
            }
        ]
    }
    validate(instance=validation_block, schema=VALIDATION_SCHEMA)


def test_required_section_with_forbid_if():
    """required_section can use forbid_if with condition name."""
    validation_block = {
        "schema_version": 1,
        "conditions": {
            "has_readme": "file_exists('README.md')"
        },
        "required_sections": [
            {
                "name": "Installation",
                "forbid_if": "has_readme"
            }
        ]
    }
    validate(instance=validation_block, schema=VALIDATION_SCHEMA)


def test_required_section_with_paragraph_constraints():
    """required_section can specify max_paragraphs and min_paragraphs."""
    validation_block = {
        "schema_version": 1,
        "required_sections": [
            {
                "name": "Purpose",
                "must_exist": True,
                "max_paragraphs": 3,
                "min_paragraphs": 1
            }
        ]
    }
    validate(instance=validation_block, schema=VALIDATION_SCHEMA)


def test_required_section_with_content_pattern():
    """required_section can specify content_pattern regex."""
    validation_block = {
        "schema_version": 1,
        "required_sections": [
            {
                "name": "Hub",
                "must_exist": True,
                "content_pattern": r"^\*\*`README\.md`\*\* - Read when"
            }
        ]
    }
    validate(instance=validation_block, schema=VALIDATION_SCHEMA)


def test_required_section_with_subsections():
    """required_section can specify subsections_required."""
    validation_block = {
        "schema_version": 1,
        "required_sections": [
            {
                "name": "Decision",
                "must_exist": True,
                "subsections_required": {
                    "min": 3,
                    "max": 7,
                    "pattern": "^### "
                }
            }
        ]
    }
    validate(instance=validation_block, schema=VALIDATION_SCHEMA)


def test_required_section_with_files_rules():
    """required_section can specify files_rules for file listings."""
    validation_block = {
        "schema_version": 1,
        "required_sections": [
            {
                "name": "Files",
                "must_exist": True,
                "files_rules": {
                    "must_list_all_md": True,
                    "exclude_globs": ["README.md", "CLAUDE.md"],
                    "entry_pattern": r"^\*\*`.+\.md`\*\* - Read when"
                }
            }
        ]
    }
    validate(instance=validation_block, schema=VALIDATION_SCHEMA)


def test_filename_pattern():
    """filename_pattern accepts regex string."""
    validation_block = {
        "schema_version": 1,
        "filename_pattern": r"^README\.md$"
    }
    validate(instance=validation_block, schema=VALIDATION_SCHEMA)


def test_frontmatter_required_fields():
    """frontmatter.required_fields accepts array of field names."""
    validation_block = {
        "schema_version": 1,
        "frontmatter": {
            "required_fields": ["doc_type", "status", "primary_category"]
        }
    }
    validate(instance=validation_block, schema=VALIDATION_SCHEMA)


def test_frontmatter_field_constraints_enum():
    """frontmatter.field_constraints can specify enum values."""
    validation_block = {
        "schema_version": 1,
        "frontmatter": {
            "field_constraints": {
                "doc_type": {
                    "enum": ["hub", "spoke", "index"]
                }
            }
        }
    }
    validate(instance=validation_block, schema=VALIDATION_SCHEMA)


def test_frontmatter_field_constraints_pattern():
    """frontmatter.field_constraints can specify pattern regex."""
    validation_block = {
        "schema_version": 1,
        "frontmatter": {
            "field_constraints": {
                "last_review": {
                    "pattern": r"^\d{4}-\d{2}-\d{2}$"
                }
            }
        }
    }
    validate(instance=validation_block, schema=VALIDATION_SCHEMA)


def test_frontmatter_field_constraints_array():
    """frontmatter.field_constraints can specify array type with min/max items."""
    validation_block = {
        "schema_version": 1,
        "frontmatter": {
            "field_constraints": {
                "consolidated_spokes": {
                    "type": "array",
                    "min_items": 3
                }
            }
        }
    }
    validate(instance=validation_block, schema=VALIDATION_SCHEMA)


def test_frontmatter_conditional_constraints():
    """frontmatter.conditional_constraints supports if/then logic."""
    validation_block = {
        "schema_version": 1,
        "frontmatter": {
            "conditional_constraints": [
                {
                    "if_field": "status",
                    "equals": "superseded",
                    "then_required": ["superseded_by"]
                },
                {
                    "if_field": "doc_type",
                    "equals": "hub",
                    "then_forbidden": ["hub_document"]
                }
            ]
        }
    }
    validate(instance=validation_block, schema=VALIDATION_SCHEMA)


def test_frontmatter_forbidden_fields():
    """frontmatter.forbidden_fields accepts array of field names."""
    validation_block = {
        "schema_version": 1,
        "frontmatter": {
            "forbidden_fields": ["version", "revision", "changelog"]
        }
    }
    validate(instance=validation_block, schema=VALIDATION_SCHEMA)


def test_complete_claude_md_validation_block():
    """Complete CLAUDE.md validation block from plan (line 415)."""
    validation_block = {
        "schema_version": 1,
        "conditions": {
            "readme_exists": "file_exists('README.md')",
            "has_md_files": "md_files_exist(exclude=['README.md', 'CLAUDE.md'])",
            "has_subdirs": "subdirs_exist()"
        },
        "title_pattern": "^# .+ Guide for LLM Agents$",
        "max_lines": 50,
        "forbidden": [
            {
                "pattern": "(?i)how to",
                "reason": "how-to instructions",
                "severity": "error"
            },
            {
                "pattern": "(?i)step 1",
                "reason": "step-by-step procedures",
                "severity": "error"
            },
            {
                "pattern": "(?i)contains information",
                "reason": "explanatory content",
                "severity": "error"
            },
            {
                "pattern": "(?i)describes",
                "reason": "explanatory content",
                "severity": "error"
            }
        ],
        "required_sections": [
            {
                "name": "Purpose",
                "must_exist": True,
                "max_paragraphs": 3
            },
            {
                "name": "Hub",
                "require_if": "readme_exists",
                "content_pattern": r"^\*\*`README\.md`\*\* - Read when"
            },
            {
                "name": "Files",
                "require_if": "has_md_files",
                "files_rules": {
                    "must_list_all_md": True,
                    "exclude_globs": ["README.md", "CLAUDE.md"],
                    "entry_pattern": r"^\*\*`.+\.md`\*\* - Read when"
                }
            },
            {
                "name": "Subdirectories",
                "require_if": "has_subdirs",
                "files_rules": {
                    "must_list_all_subdirs": True,
                    "entry_pattern": r"^\*\*`.+/`\*\* - Read when"
                }
            }
        ]
    }
    validate(instance=validation_block, schema=VALIDATION_SCHEMA)


def test_complete_hub_validation_block():
    """Complete hub validation block from plan (line 499)."""
    validation_block = {
        "schema_version": 1,
        "filename_pattern": r"^README\.md$",
        "frontmatter": {
            "required_fields": [
                "doc_type",
                "status",
                "primary_category",
                "consolidated_spokes"
            ],
            "field_constraints": {
                "doc_type": {
                    "enum": ["hub"]
                },
                "status": {
                    "enum": ["draft", "active", "deprecated", "superseded"]
                },
                "consolidated_spokes": {
                    "type": "array",
                    "min_items": 3
                }
            },
            "conditional_constraints": [
                {
                    "if_field": "status",
                    "equals": "superseded",
                    "then_required": ["superseded_by"]
                }
            ]
        },
        "required_sections": [
            {
                "name": "Context",
                "must_exist": True,
                "min_paragraphs": 2,
                "max_paragraphs": 4
            },
            {
                "name": "Decision",
                "must_exist": True,
                "subsections_required": {
                    "min": 3,
                    "max": 7,
                    "pattern": "^### "
                }
            },
            {
                "name": "Consequences",
                "must_exist": True
            },
            {
                "name": "Related Documents",
                "must_exist": True
            }
        ]
    }
    validate(instance=validation_block, schema=VALIDATION_SCHEMA)


def test_complete_spoke_validation_block():
    """Complete spoke validation block from plan (line 549)."""
    validation_block = {
        "schema_version": 1,
        "frontmatter": {
            "required_fields": [
                "doc_type",
                "status",
                "primary_category",
                "hub_document"
            ],
            "field_constraints": {
                "doc_type": {
                    "enum": ["spoke"]
                },
                "hub_document": {
                    "pattern": r".*\.md$"
                }
            }
        }
    }
    validate(instance=validation_block, schema=VALIDATION_SCHEMA)


def test_reject_unknown_top_level_fields():
    """Schema rejects unknown top-level fields."""
    validation_block = {
        "schema_version": 1,
        "unknown_field": "some value"
    }
    with pytest.raises(ValidationError):
        validate(instance=validation_block, schema=VALIDATION_SCHEMA)


def test_reject_malformed_subsections_required():
    """subsections_required must be an object."""
    validation_block = {
        "schema_version": 1,
        "required_sections": [
            {
                "name": "Decision",
                "must_exist": True,
                "subsections_required": ["min", "max"]
            }
        ]
    }
    with pytest.raises(ValidationError):
        validate(instance=validation_block, schema=VALIDATION_SCHEMA)


def test_reject_malformed_files_rules():
    """files_rules must be an object."""
    validation_block = {
        "schema_version": 1,
        "required_sections": [
            {
                "name": "Files",
                "must_exist": True,
                "files_rules": "must_list_all_md"
            }
        ]
    }
    with pytest.raises(ValidationError):
        validate(instance=validation_block, schema=VALIDATION_SCHEMA)


def test_cross_cutting_index_validation_block():
    """Simpler validation block for cross-cutting index template."""
    validation_block = {
        "schema_version": 1,
        "title_pattern": "^# .+ Index$",
        "frontmatter": {
            "required_fields": ["doc_type", "status", "primary_category"],
            "field_constraints": {
                "doc_type": {
                    "enum": ["index"]
                }
            }
        },
        "required_sections": [
            {
                "name": "Purpose",
                "must_exist": True,
                "max_paragraphs": 2
            },
            {
                "name": "Documents",
                "must_exist": True
            }
        ]
    }
    validate(instance=validation_block, schema=VALIDATION_SCHEMA)


if __name__ == "__main__":
    pytest.main([__file__, "-v", "--tb=short"])
