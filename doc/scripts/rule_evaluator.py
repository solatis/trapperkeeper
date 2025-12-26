#!/usr/bin/env python3
"""
Rule Evaluator - Generic rule interpreter for template-driven validation.

This module provides a generic rule evaluation engine that reads validation rules
from template frontmatter and validates documents against those rules.

Key Features:
- Parse validation rules from template frontmatter
- Execute rules in order: title_pattern, max_lines, forbidden, filename_pattern
- Return structured validation errors with line numbers
- Conditional logic support with predicates (M5)
- Section rules with conditional requirements (M5)
"""

import re
from pathlib import Path
from typing import List, Dict, Optional, Any
from dataclasses import dataclass

try:
    from predicates import PredicateContext, evaluate_predicate, PredicateError
    from markdown_parser import parse_markdown, extract_sections, get_section, count_paragraphs, count_sentences, extract_list_items
    PREDICATES_AVAILABLE = True
except ImportError:
    PREDICATES_AVAILABLE = False


@dataclass
class ValidationError:
    """
    Structured validation error with detailed information.

    Attributes:
        file_path: Path to the document that failed validation
        line_number: Line number where error occurred (0 for file-level errors)
        rule_violated: Name of the validation rule that was violated
        detail: Detailed description of the error
        expected: What was expected (empty string if not applicable)
        found: What was actually found (empty string if not applicable)
        severity: Error severity level ("error" or "warn")
    """
    file_path: str
    line_number: int
    rule_violated: str
    detail: str
    expected: str
    found: str
    severity: str = "error"

    def format_error(self) -> str:
        """
        Format validation error for console output.

        Returns:
            Formatted error string

        Example:
            [ERROR] doc/test/example.md:1: title_pattern
              Detail: Title does not match required pattern
              Expected: Title ending with "Guide"
              Found: "# Welcome"
        """
        severity_tag = f"[{self.severity.upper()}]"
        file_line = f"{self.file_path}:{self.line_number}" if self.line_number > 0 else self.file_path
        header = f"{severity_tag} {file_line}: {self.rule_violated}"

        parts = [header]
        if self.detail:
            parts.append(f"  Detail: {self.detail}")
        if self.expected:
            parts.append(f"  Expected: {self.expected}")
        if self.found:
            parts.append(f"  Found: {self.found}")

        return "\n".join(parts)


class RuleEvaluator:
    """
    Generic rule evaluation engine for template-driven validation.

    Evaluates documents against validation rules defined in template frontmatter.
    Supports basic rules (M4) and conditional rules (M5+).
    """

    def __init__(self, validation_schema: Dict[str, Any], predicate_context: Optional['PredicateContext'] = None):
        """
        Initialize rule evaluator with validation schema.

        Args:
            validation_schema: Validation rules from template frontmatter
            predicate_context: Optional context for predicate evaluation (M5)
        """
        self.schema = validation_schema
        self.predicate_context = predicate_context
        self.errors: List[ValidationError] = []
        self.condition_results: Dict[str, bool] = {}

    def evaluate(self, document_path: Path, document_content: str, frontmatter: Optional[Dict[str, Any]] = None) -> List[ValidationError]:
        """
        Evaluate document against all validation rules.

        Executes rules in order:
        1. Evaluate conditions (M5)
        2. filename_pattern (if present)
        3. frontmatter (M7, if present)
        4. title_pattern (if present)
        5. max_lines (if present)
        6. forbidden (if present)
        7. required_sections (M5, if present)

        Args:
            document_path: Path to the document being validated
            document_content: Content of the document
            frontmatter: Optional frontmatter dictionary extracted from document

        Returns:
            List of ValidationError objects (empty if validation passes)
        """
        self.errors = []

        # M5: Evaluate conditions first
        if "conditions" in self.schema:
            self._evaluate_conditions(document_path, document_content)

        # Rule 1: filename_pattern
        if "filename_pattern" in self.schema:
            self._check_filename_pattern(document_path, self.schema["filename_pattern"])

        # M7: frontmatter validation
        if "frontmatter" in self.schema and frontmatter is not None:
            self._check_frontmatter(document_path, frontmatter, self.schema["frontmatter"])

        # Rule 2: title_pattern
        if "title_pattern" in self.schema:
            self._check_title_pattern(document_path, document_content, self.schema["title_pattern"])

        # Rule 3: max_lines
        if "max_lines" in self.schema:
            self._check_max_lines(document_path, document_content, self.schema["max_lines"])

        # Rule 4: forbidden
        if "forbidden" in self.schema:
            self._check_forbidden(document_path, document_content, self.schema["forbidden"])

        # M5: required_sections
        if "required_sections" in self.schema:
            self._check_required_sections(document_path, document_content, self.schema["required_sections"])

        return self.errors

    def _check_filename_pattern(self, document_path: Path, pattern: str) -> None:
        """
        Check if filename matches required pattern.

        Args:
            document_path: Path to the document
            pattern: Regular expression pattern for filename
        """
        filename = document_path.name

        try:
            if not re.match(pattern, filename):
                self.errors.append(ValidationError(
                    file_path=str(document_path),
                    line_number=0,
                    rule_violated="filename_pattern",
                    detail="Filename does not match required pattern",
                    expected=f"Filename matching pattern: {pattern}",
                    found=filename,
                    severity="error"
                ))
        except re.error as e:
            self.errors.append(ValidationError(
                file_path=str(document_path),
                line_number=0,
                rule_violated="filename_pattern",
                detail=f"Invalid regex pattern: {e}",
                expected="Valid regular expression",
                found=pattern,
                severity="error"
            ))

    def _check_frontmatter(self, document_path: Path, frontmatter: Dict[str, Any], fm_rules: Dict[str, Any]) -> None:
        """
        Check frontmatter against validation rules.

        Args:
            document_path: Path to the document
            frontmatter: Frontmatter dictionary extracted from document
            fm_rules: Frontmatter validation rules from template
        """
        # Check required fields
        if "required_fields" in fm_rules:
            for field in fm_rules["required_fields"]:
                if field not in frontmatter:
                    self.errors.append(ValidationError(
                        file_path=str(document_path),
                        line_number=0,
                        rule_violated="frontmatter",
                        detail=f"Missing required frontmatter field: {field}",
                        expected=f"Field '{field}' present in frontmatter",
                        found="Field not found",
                        severity="error"
                    ))

        # Check field constraints
        if "field_constraints" in fm_rules:
            self._check_field_constraints(document_path, frontmatter, fm_rules["field_constraints"])

        # Check conditional constraints
        if "conditional_constraints" in fm_rules:
            for constraint in fm_rules["conditional_constraints"]:
                if_field = constraint.get("if_field")
                equals = constraint.get("equals")
                then_required = constraint.get("then_required", [])

                # Check if condition is met
                if if_field in frontmatter and frontmatter[if_field] == equals:
                    # Condition is true, check required fields
                    for required_field in then_required:
                        if required_field not in frontmatter:
                            self.errors.append(ValidationError(
                                file_path=str(document_path),
                                line_number=0,
                                rule_violated="frontmatter",
                                detail=f"Conditional constraint: if {if_field}={equals}, then {required_field} is required",
                                expected=f"Field '{required_field}' present when {if_field}={equals}",
                                found="Field not found",
                                severity="error"
                            ))

    def _check_field_constraints(self, document_path: Path, frontmatter: Dict[str, Any], field_constraints: Dict[str, Any]) -> None:
        """
        Check field constraints in frontmatter.

        Args:
            document_path: Path to the document
            frontmatter: Frontmatter dictionary extracted from document
            field_constraints: Field constraint rules
        """
        for field_name, constraints in field_constraints.items():
            if field_name not in frontmatter:
                # Field doesn't exist, skip constraint checking
                # (required_fields check will catch this if field is required)
                continue

            field_value = frontmatter[field_name]

            # Check enum constraint
            if "enum" in constraints:
                valid_values = constraints["enum"]
                if field_value not in valid_values:
                    self.errors.append(ValidationError(
                        file_path=str(document_path),
                        line_number=0,
                        rule_violated="frontmatter",
                        detail=f"Field '{field_name}' has invalid value",
                        expected=f"One of: {', '.join(valid_values)}",
                        found=str(field_value),
                        severity="error"
                    ))

            # Check pattern constraint
            if "pattern" in constraints:
                pattern = constraints["pattern"]
                try:
                    if not re.match(pattern, str(field_value)):
                        self.errors.append(ValidationError(
                            file_path=str(document_path),
                            line_number=0,
                            rule_violated="frontmatter",
                            detail=f"Field '{field_name}' does not match required pattern",
                            expected=f"Pattern: {pattern}",
                            found=str(field_value),
                            severity="error"
                        ))
                except re.error as e:
                    self.errors.append(ValidationError(
                        file_path=str(document_path),
                        line_number=0,
                        rule_violated="frontmatter",
                        detail=f"Invalid regex pattern for field '{field_name}': {e}",
                        expected="Valid regular expression",
                        found=pattern,
                        severity="error"
                    ))

            # Check type constraint
            if "type" in constraints:
                expected_type = constraints["type"]

                if expected_type == "array":
                    if not isinstance(field_value, list):
                        self.errors.append(ValidationError(
                            file_path=str(document_path),
                            line_number=0,
                            rule_violated="frontmatter",
                            detail=f"Field '{field_name}' must be an array",
                            expected="Array/list type",
                            found=f"{type(field_value).__name__}",
                            severity="error"
                        ))
                    else:
                        # Check min_items if specified
                        if "min_items" in constraints:
                            min_items = constraints["min_items"]
                            if len(field_value) < min_items:
                                self.errors.append(ValidationError(
                                    file_path=str(document_path),
                                    line_number=0,
                                    rule_violated="frontmatter",
                                    detail=f"Field '{field_name}' has too few items",
                                    expected=f"Minimum {min_items} items",
                                    found=f"{len(field_value)} items",
                                    severity="error"
                                ))

                elif expected_type == "string":
                    if not isinstance(field_value, str):
                        self.errors.append(ValidationError(
                            file_path=str(document_path),
                            line_number=0,
                            rule_violated="frontmatter",
                            detail=f"Field '{field_name}' must be a string",
                            expected="String type",
                            found=f"{type(field_value).__name__}",
                            severity="error"
                        ))

    def _check_title_pattern(self, document_path: Path, content: str, pattern: str) -> None:
        """
        Check if document title (first H1) matches required pattern.

        Args:
            document_path: Path to the document
            content: Document content
            pattern: Regular expression pattern for title
        """
        lines = content.split('\n')

        # Find first H1 heading
        title_line_num = 0
        title = None

        for i, line in enumerate(lines, start=1):
            if line.startswith('# '):
                title = line
                title_line_num = i
                break

        if title is None:
            self.errors.append(ValidationError(
                file_path=str(document_path),
                line_number=1,
                rule_violated="title_pattern",
                detail="Document has no title (H1 heading)",
                expected=f"Title matching pattern: {pattern}",
                found="No H1 heading found",
                severity="error"
            ))
            return

        try:
            if not re.match(pattern, title):
                self.errors.append(ValidationError(
                    file_path=str(document_path),
                    line_number=title_line_num,
                    rule_violated="title_pattern",
                    detail="Title does not match required pattern",
                    expected=f"Title matching pattern: {pattern}",
                    found=title,
                    severity="error"
                ))
        except re.error as e:
            self.errors.append(ValidationError(
                file_path=str(document_path),
                line_number=title_line_num,
                rule_violated="title_pattern",
                detail=f"Invalid regex pattern: {e}",
                expected="Valid regular expression",
                found=pattern,
                severity="error"
            ))

    def _check_max_lines(self, document_path: Path, content: str, max_lines: int) -> None:
        """
        Check if document exceeds maximum line count.

        Args:
            document_path: Path to the document
            content: Document content
            max_lines: Maximum allowed lines
        """
        lines = content.split('\n')
        line_count = len(lines)

        if line_count > max_lines:
            self.errors.append(ValidationError(
                file_path=str(document_path),
                line_number=0,
                rule_violated="max_lines",
                detail=f"Document exceeds maximum line limit",
                expected=f"Maximum {max_lines} lines",
                found=f"{line_count} lines",
                severity="error"
            ))

    def _check_forbidden(self, document_path: Path, content: str, forbidden_list: List[Dict[str, str]]) -> None:
        """
        Check for forbidden patterns in document content.

        Args:
            document_path: Path to the document
            content: Document content
            forbidden_list: List of forbidden pattern dictionaries
        """
        lines = content.split('\n')

        for forbidden_rule in forbidden_list:
            pattern = forbidden_rule.get("pattern", "")
            reason = forbidden_rule.get("reason", "")
            severity = forbidden_rule.get("severity", "error")

            try:
                # Search for pattern in entire document
                matches = re.finditer(pattern, content, re.MULTILINE | re.IGNORECASE)

                for match in matches:
                    # Find line number by counting newlines up to match position
                    line_num = content[:match.start()].count('\n') + 1

                    # Extract the matched text
                    matched_text = match.group(0)

                    # Get the line content
                    line_content = lines[line_num - 1] if line_num <= len(lines) else ""

                    self.errors.append(ValidationError(
                        file_path=str(document_path),
                        line_number=line_num,
                        rule_violated="forbidden",
                        detail=f"Forbidden pattern found: {reason}",
                        expected="Content without forbidden pattern",
                        found=f"Pattern '{matched_text}' found in: {line_content[:60]}...",
                        severity=severity
                    ))

            except re.error as e:
                self.errors.append(ValidationError(
                    file_path=str(document_path),
                    line_number=0,
                    rule_violated="forbidden",
                    detail=f"Invalid regex pattern: {e}",
                    expected="Valid regular expression",
                    found=pattern,
                    severity="error"
                ))

    def _evaluate_conditions(self, document_path: Path, document_content: str) -> None:
        """
        Evaluate all conditions from the validation schema.

        Args:
            document_path: Path to the document being validated
            document_content: Content of the document
        """
        if not PREDICATES_AVAILABLE:
            self.errors.append(ValidationError(
                file_path=str(document_path),
                line_number=0,
                rule_violated="conditions",
                detail="Conditions require predicates and markdown_parser modules",
                expected="Install markdown-it-py package",
                found="Missing dependencies",
                severity="error"
            ))
            return

        conditions = self.schema.get("conditions", {})

        # Create or use existing predicate context
        if self.predicate_context is None:
            # Parse document to AST for section_present predicates
            doc_ast = parse_markdown(document_content, cache_key=str(document_path))
            self.predicate_context = PredicateContext(document_path, doc_ast=doc_ast)

        # Evaluate each condition
        for condition_name, predicate_expr in conditions.items():
            try:
                result = evaluate_predicate(predicate_expr, self.predicate_context)
                self.condition_results[condition_name] = result
            except PredicateError as e:
                self.errors.append(ValidationError(
                    file_path=str(document_path),
                    line_number=0,
                    rule_violated="conditions",
                    detail=f"Failed to evaluate condition '{condition_name}': {e}",
                    expected=f"Valid predicate expression",
                    found=predicate_expr,
                    severity="error"
                ))
                # Set to False on error to skip dependent rules
                self.condition_results[condition_name] = False

    def _check_required_sections(self, document_path: Path, document_content: str, section_rules: List[Dict[str, Any]]) -> None:
        """
        Check required sections with conditional logic.

        Args:
            document_path: Path to the document being validated
            document_content: Content of the document
            section_rules: List of section rule dictionaries
        """
        if not PREDICATES_AVAILABLE:
            self.errors.append(ValidationError(
                file_path=str(document_path),
                line_number=0,
                rule_violated="required_sections",
                detail="required_sections requires markdown_parser module",
                expected="Install markdown-it-py package",
                found="Missing dependencies",
                severity="error"
            ))
            return

        # Parse document to get sections
        doc_ast = parse_markdown(document_content, cache_key=str(document_path))
        sections = extract_sections(doc_ast)

        for rule in section_rules:
            section_name = rule.get("name", "")

            # Check conditional requirements
            if not self._should_apply_section_rule(rule):
                continue

            # Get section if exists
            section_info = sections.get(section_name)

            # Check must_exist
            must_exist = rule.get("must_exist", False)
            if must_exist and section_info is None:
                self.errors.append(ValidationError(
                    file_path=str(document_path),
                    line_number=0,
                    rule_violated="required_sections",
                    detail=f"Required section '{section_name}' is missing",
                    expected=f"Section named '{section_name}'",
                    found="Section not found",
                    severity="error"
                ))
                continue

            # Check must_not_exist (for forbid_if logic)
            must_not_exist = rule.get("must_not_exist", False)
            if must_not_exist and section_info is not None:
                self.errors.append(ValidationError(
                    file_path=str(document_path),
                    line_number=section_info.line_start + 1,
                    rule_violated="required_sections",
                    detail=f"Forbidden section '{section_name}' is present",
                    expected=f"Section '{section_name}' should not exist",
                    found=f"Section found at line {section_info.line_start + 1}",
                    severity="error"
                ))
                continue

            # Skip further checks if section doesn't exist
            if section_info is None:
                continue

            # Check section content rules
            self._check_section_content(document_path, section_name, section_info, rule)

            # Check files_rules
            if "files_rules" in rule:
                self._check_files_rules(document_path, section_name, section_info, rule["files_rules"])

    def _should_apply_section_rule(self, rule: Dict[str, Any]) -> bool:
        """
        Determine if a section rule should be applied based on conditions.

        Args:
            rule: Section rule dictionary

        Returns:
            True if rule should be applied, False otherwise
        """
        # Check require_if
        if "require_if" in rule:
            condition_name = rule["require_if"]
            # Rule applies only if condition is True
            return self.condition_results.get(condition_name, False)

        # Check forbid_if
        if "forbid_if" in rule:
            condition_name = rule["forbid_if"]
            # Rule applies only if condition is True (to forbid the section)
            return self.condition_results.get(condition_name, False)

        # No conditional requirement, always apply
        return True

    def _check_section_content(self, document_path: Path, section_name: str, section_info: Any, rule: Dict[str, Any]) -> None:
        """
        Check section content rules (min_paragraphs, max_paragraphs, max_sentences, content_pattern, subsections_required).

        Args:
            document_path: Path to the document being validated
            section_name: Name of the section
            section_info: SectionInfo object from markdown_parser
            rule: Section rule dictionary
        """
        # Check min_paragraphs (M7)
        if "min_paragraphs" in rule:
            min_paragraphs = rule["min_paragraphs"]
            paragraph_count = count_paragraphs(section_info)

            if paragraph_count < min_paragraphs:
                self.errors.append(ValidationError(
                    file_path=str(document_path),
                    line_number=section_info.line_start + 1,  # Convert to 1-based
                    rule_violated="required_sections",
                    detail=f"Section '{section_name}' has too few paragraphs",
                    expected=f"Minimum {min_paragraphs} paragraphs",
                    found=f"{paragraph_count} paragraphs",
                    severity="error"
                ))

        # Check max_paragraphs
        if "max_paragraphs" in rule:
            max_paragraphs = rule["max_paragraphs"]
            paragraph_count = count_paragraphs(section_info)

            if paragraph_count > max_paragraphs:
                self.errors.append(ValidationError(
                    file_path=str(document_path),
                    line_number=section_info.line_start + 1,  # Convert to 1-based
                    rule_violated="required_sections",
                    detail=f"Section '{section_name}' exceeds paragraph limit",
                    expected=f"Maximum {max_paragraphs} paragraphs",
                    found=f"{paragraph_count} paragraphs",
                    severity="error"
                ))

        # Check max_sentences
        if "max_sentences" in rule:
            max_sentences = rule["max_sentences"]
            sentence_count = count_sentences(section_info)

            if sentence_count > max_sentences:
                self.errors.append(ValidationError(
                    file_path=str(document_path),
                    line_number=section_info.line_start + 1,  # Convert to 1-based
                    rule_violated="required_sections",
                    detail=f"Section '{section_name}' exceeds sentence limit",
                    expected=f"Maximum {max_sentences} sentences",
                    found=f"{sentence_count} sentences",
                    severity="error"
                ))

        # Check content_pattern
        if "content_pattern" in rule:
            pattern = rule["content_pattern"]
            try:
                if not re.search(pattern, section_info.content):
                    self.errors.append(ValidationError(
                        file_path=str(document_path),
                        line_number=section_info.line_start + 1,  # Convert to 1-based
                        rule_violated="required_sections",
                        detail=f"Section '{section_name}' content does not match required pattern",
                        expected=f"Content matching pattern: {pattern}",
                        found=f"Pattern not found in section content",
                        severity="error"
                    ))
            except re.error as e:
                self.errors.append(ValidationError(
                    file_path=str(document_path),
                    line_number=section_info.line_start + 1,
                    rule_violated="required_sections",
                    detail=f"Invalid regex pattern in content_pattern for section '{section_name}': {e}",
                    expected="Valid regular expression",
                    found=pattern,
                    severity="error"
                ))

        # Check subsections_required (M7)
        if "subsections_required" in rule:
            subsections_rules = rule["subsections_required"]
            self._check_subsections(document_path, section_name, section_info, subsections_rules)

    def _check_subsections(self, document_path: Path, section_name: str, section_info: Any, subsections_rules: Dict[str, Any]) -> None:
        """
        Check subsection requirements for a section.

        For hub documents, subsections are ### headings that immediately follow a ## section.
        Since markdown_parser extracts these as separate sections, we need to count them
        by looking at the document content directly.

        Args:
            document_path: Path to the document being validated
            section_name: Name of the parent section
            section_info: SectionInfo object from markdown_parser
            subsections_rules: Subsection validation rules
        """
        if not PREDICATES_AVAILABLE:
            return

        # Parse document to get all sections
        from markdown_parser import parse_markdown, extract_sections
        doc_path_str = str(document_path)

        # Read document content
        content = document_path.read_text(encoding='utf-8')
        doc_ast = parse_markdown(content, cache_key=doc_path_str)
        sections = extract_sections(doc_ast)

        # Find the parent section in the extracted sections
        if section_name not in sections:
            return

        parent_section = sections[section_name]
        subsection_pattern = subsections_rules.get("pattern", "^### ")

        # Count consecutive sections immediately following parent that match subsection pattern
        # We look for sections whose names would be introduced by lines matching the pattern
        subsection_count = 0

        # Get all section names in order
        section_names = list(sections.keys())
        try:
            parent_index = section_names.index(section_name)
        except ValueError:
            return

        # Check subsequent sections to see if they are subsections
        # A subsection is one that appears immediately after the parent (or another subsection)
        # and would be created by a ### heading
        lines = content.split('\n')
        in_subsection_region = False
        parent_line = parent_section.line_start

        for i in range(parent_line + 1, len(lines)):
            line = lines[i].strip()

            # Check if this is a ## heading (end of subsection region)
            if line.startswith('## ') and not line.startswith('### '):
                break

            # Check if this matches the subsection pattern
            if line.startswith('### '):
                try:
                    if re.match(subsection_pattern, line):
                        subsection_count += 1
                        in_subsection_region = True
                except re.error as e:
                    self.errors.append(ValidationError(
                        file_path=str(document_path),
                        line_number=section_info.line_start + 1,
                        rule_violated="required_sections",
                        detail=f"Invalid regex pattern in subsections pattern for section '{section_name}': {e}",
                        expected="Valid regular expression",
                        found=subsection_pattern,
                        severity="error"
                    ))
                    return

        # Check min subsections
        if "min" in subsections_rules:
            min_subsections = subsections_rules["min"]
            if subsection_count < min_subsections:
                self.errors.append(ValidationError(
                    file_path=str(document_path),
                    line_number=section_info.line_start + 1,
                    rule_violated="required_sections",
                    detail=f"Section '{section_name}' has too few subsections",
                    expected=f"Minimum {min_subsections} subsections matching pattern: {subsection_pattern}",
                    found=f"{subsection_count} subsections",
                    severity="error"
                ))

        # Check max subsections
        if "max" in subsections_rules:
            max_subsections = subsections_rules["max"]
            if subsection_count > max_subsections:
                self.errors.append(ValidationError(
                    file_path=str(document_path),
                    line_number=section_info.line_start + 1,
                    rule_violated="required_sections",
                    detail=f"Section '{section_name}' has too many subsections",
                    expected=f"Maximum {max_subsections} subsections matching pattern: {subsection_pattern}",
                    found=f"{subsection_count} subsections",
                    severity="error"
                ))

    def _check_files_rules(self, document_path: Path, section_name: str, section_info: Any, files_rules: Dict[str, Any]) -> None:
        """
        Check files_rules for a section.

        Args:
            document_path: Path to the document being validated
            section_name: Name of the section
            section_info: SectionInfo object from markdown_parser
            files_rules: Files rules dictionary
        """
        # Extract content lines from section (CLAUDE.md uses paragraphs, not lists)
        section_lines = section_info.content.split('\n')
        # Filter out empty lines
        content_lines = [line.strip() for line in section_lines if line.strip()]

        # Check must_list_all_md
        if files_rules.get("must_list_all_md", False):
            # Get all .md files in document directory
            doc_dir = document_path.parent
            exclude_globs = files_rules.get("exclude_globs", [])

            # Find all .md files
            md_files = [f.name for f in doc_dir.glob("*.md") if f.is_file()]

            # Filter out excluded files
            exclude_set = set(exclude_globs)
            md_files = [f for f in md_files if f not in exclude_set]

            # Check if all files are listed
            for md_file in md_files:
                # Check if file is mentioned in section content
                file_mentioned = any(md_file in line for line in content_lines)

                if not file_mentioned:
                    self.errors.append(ValidationError(
                        file_path=str(document_path),
                        line_number=section_info.line_start + 1,
                        rule_violated="required_sections",
                        detail=f"Section '{section_name}' missing required file entry",
                        expected=f"File '{md_file}' listed in section",
                        found="File not mentioned in section",
                        severity="error"
                    ))

        # Check must_list_all_subdirs
        if files_rules.get("must_list_all_subdirs", False):
            # Get all subdirectories in document directory
            doc_dir = document_path.parent
            exclude_globs = files_rules.get("exclude_globs", [])

            # Find all subdirectories (excluding hidden and dunder dirs)
            subdirs = [d.name for d in doc_dir.iterdir()
                      if d.is_dir()
                      and not d.name.startswith('.')
                      and not d.name.startswith('__')]

            # Filter out excluded directories
            exclude_set = set(exclude_globs)
            subdirs = [d for d in subdirs if d not in exclude_set]

            # Check if all subdirs are listed (with trailing slash)
            for subdir in subdirs:
                # Check if subdir is mentioned in section content (with or without trailing slash)
                subdir_with_slash = f"{subdir}/"
                subdir_mentioned = any(subdir_with_slash in line or f"`{subdir}`" in line for line in content_lines)

                if not subdir_mentioned:
                    self.errors.append(ValidationError(
                        file_path=str(document_path),
                        line_number=section_info.line_start + 1,
                        rule_violated="required_sections",
                        detail=f"Section '{section_name}' missing required subdirectory entry",
                        expected=f"Subdirectory '{subdir_with_slash}' listed in section",
                        found="Subdirectory not mentioned in section",
                        severity="error"
                    ))

        # Check entry_pattern
        if "entry_pattern" in files_rules:
            pattern = files_rules["entry_pattern"]

            try:
                # Check each line that looks like an entry (not empty, starts with **)
                entry_lines = [line for line in content_lines if line.startswith('**')]

                for entry_line in entry_lines:
                    if not re.match(pattern, entry_line):
                        self.errors.append(ValidationError(
                            file_path=str(document_path),
                            line_number=section_info.line_start + 1,
                            rule_violated="required_sections",
                            detail=f"Section '{section_name}' has incorrectly formatted entry",
                            expected=f"Entry matching pattern: {pattern}",
                            found=f"'{entry_line[:50]}...'",
                            severity="error"
                        ))
            except re.error as e:
                self.errors.append(ValidationError(
                    file_path=str(document_path),
                    line_number=section_info.line_start + 1,
                    rule_violated="required_sections",
                    detail=f"Invalid regex pattern in entry_pattern for section '{section_name}': {e}",
                    expected="Valid regular expression",
                    found=pattern,
                    severity="error"
                ))


def validate_document(document_path: Path, template_validation: Dict[str, Any]) -> List[ValidationError]:
    """
    Convenience function to validate a document against template rules.

    Args:
        document_path: Path to the document to validate
        template_validation: Validation schema from template frontmatter

    Returns:
        List of ValidationError objects (empty if validation passes)
    """
    content = document_path.read_text(encoding='utf-8')
    evaluator = RuleEvaluator(template_validation)
    return evaluator.evaluate(document_path, content)


if __name__ == '__main__':
    # Example usage
    import sys
    import json

    if len(sys.argv) < 3:
        print("Usage: rule_evaluator.py <document_path> <validation_json>")
        print("\nExample validation JSON:")
        print(json.dumps({
            "schema_version": 1,
            "title_pattern": "^# .+ Guide$",
            "max_lines": 50,
            "forbidden": [
                {"pattern": "(?i)how to", "reason": "how-to instructions", "severity": "error"}
            ]
        }, indent=2))
        sys.exit(1)

    doc_path = Path(sys.argv[1])
    validation_json = sys.argv[2]

    try:
        validation_schema = json.loads(validation_json)
    except json.JSONDecodeError as e:
        print(f"Error parsing validation JSON: {e}")
        sys.exit(1)

    errors = validate_document(doc_path, validation_schema)

    if errors:
        print(f"Validation failed with {len(errors)} error(s):\n")
        for error in errors:
            print(error.format_error())
            print()
        sys.exit(1)
    else:
        print(f"Validation passed: {doc_path}")
        sys.exit(0)
