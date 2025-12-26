#!/usr/bin/env python3
"""
Test suite for predicate engine.

Tests all 4 predicate functions with edge cases, expression parser,
and error handling.
"""

import pytest
from pathlib import Path
from predicates import (
    PredicateContext,
    PredicateError,
    PredicateParseError,
    PredicateEvaluationError,
    file_exists,
    md_files_exist,
    section_present,
    subdirs_exist,
    parse_predicate_expression,
    evaluate_predicate,
)


# ============================================================================
# Predicate Function Tests: file_exists
# ============================================================================

def test_file_exists_when_file_present(tmp_path):
    """Test file_exists returns True when file exists."""
    # Create test file
    doc_file = tmp_path / "CLAUDE.md"
    doc_file.write_text("# Test")
    readme = tmp_path / "README.md"
    readme.write_text("# Readme")

    ctx = PredicateContext(doc_file)
    assert file_exists("README.md", ctx) is True


def test_file_exists_when_file_absent(tmp_path):
    """Test file_exists returns False when file does not exist."""
    doc_file = tmp_path / "CLAUDE.md"
    doc_file.write_text("# Test")

    ctx = PredicateContext(doc_file)
    assert file_exists("NONEXISTENT.md", ctx) is False


def test_file_exists_with_subdirectory(tmp_path):
    """Test file_exists with relative path in subdirectory."""
    doc_file = tmp_path / "CLAUDE.md"
    doc_file.write_text("# Test")

    subdir = tmp_path / "subdir"
    subdir.mkdir()
    subfile = subdir / "file.txt"
    subfile.write_text("content")

    ctx = PredicateContext(doc_file)
    assert file_exists("subdir/file.txt", ctx) is True
    assert file_exists("subdir/missing.txt", ctx) is False


def test_file_exists_rejects_directory(tmp_path):
    """Test file_exists returns False for directories."""
    doc_file = tmp_path / "CLAUDE.md"
    doc_file.write_text("# Test")

    subdir = tmp_path / "subdir"
    subdir.mkdir()

    ctx = PredicateContext(doc_file)
    # Directory exists but is not a file
    assert file_exists("subdir", ctx) is False


# ============================================================================
# Predicate Function Tests: md_files_exist
# ============================================================================

def test_md_files_exist_when_files_present(tmp_path):
    """Test md_files_exist returns True when markdown files exist."""
    doc_file = tmp_path / "CLAUDE.md"
    doc_file.write_text("# Test")
    other = tmp_path / "other.md"
    other.write_text("# Other")

    ctx = PredicateContext(doc_file)
    assert md_files_exist(exclude=[], ctx=ctx) is True


def test_md_files_exist_when_no_files(tmp_path):
    """Test md_files_exist returns False when no markdown files exist."""
    doc_file = tmp_path / "CLAUDE.md"
    doc_file.write_text("# Test")

    ctx = PredicateContext(doc_file)
    # Exclude the only .md file
    assert md_files_exist(exclude=["CLAUDE.md"], ctx=ctx) is False


def test_md_files_exist_with_exclusions(tmp_path):
    """Test md_files_exist properly excludes specified files."""
    doc_file = tmp_path / "CLAUDE.md"
    doc_file.write_text("# Test")
    readme = tmp_path / "README.md"
    readme.write_text("# Readme")
    other = tmp_path / "other.md"
    other.write_text("# Other")

    ctx = PredicateContext(doc_file)

    # Exclude some files, should still find other.md
    assert md_files_exist(exclude=["CLAUDE.md", "README.md"], ctx=ctx) is True

    # Exclude all files
    assert md_files_exist(exclude=["CLAUDE.md", "README.md", "other.md"], ctx=ctx) is False


def test_md_files_exist_empty_directory(tmp_path):
    """Test md_files_exist returns False in empty directory."""
    # Create directory with no .md files
    subdir = tmp_path / "empty"
    subdir.mkdir()
    doc_file = subdir / "test.txt"  # Not a .md file
    doc_file.write_text("test")

    # Create a .md file to use as doc_path
    actual_doc = subdir / "doc.md"
    actual_doc.write_text("# Doc")

    ctx = PredicateContext(actual_doc)
    assert md_files_exist(exclude=["doc.md"], ctx=ctx) is False


def test_md_files_exist_requires_context():
    """Test md_files_exist raises error when context is None."""
    with pytest.raises(PredicateEvaluationError, match="requires context parameter"):
        md_files_exist(exclude=[], ctx=None)


# ============================================================================
# Predicate Function Tests: section_present
# ============================================================================

def test_section_present_stub_raises_error(tmp_path):
    """Test section_present raises error when no AST provided."""
    doc_file = tmp_path / "CLAUDE.md"
    doc_file.write_text("# Test\n## Files\n")

    ctx = PredicateContext(doc_file, doc_ast=None)

    with pytest.raises(PredicateEvaluationError, match="requires doc_ast"):
        section_present("Files", ctx)


def test_section_present_with_ast_works(tmp_path):
    """Test section_present works with AST provided."""
    from markdown_parser import parse_markdown

    doc_file = tmp_path / "CLAUDE.md"
    doc_file.write_text("# Test\n## Files\n\nContent.\n\n## Summary\n\nMore content.")

    # Parse document to get real AST
    text = doc_file.read_text()
    ast = parse_markdown(text)
    ctx = PredicateContext(doc_file, doc_ast=ast)

    # Should work with real AST
    assert section_present("Files", ctx) is True
    assert section_present("Summary", ctx) is True
    assert section_present("Missing", ctx) is False


# ============================================================================
# Predicate Function Tests: subdirs_exist
# ============================================================================

def test_subdirs_exist_when_present(tmp_path):
    """Test subdirs_exist returns True when subdirectories exist."""
    doc_file = tmp_path / "CLAUDE.md"
    doc_file.write_text("# Test")

    subdir = tmp_path / "subdir"
    subdir.mkdir()

    ctx = PredicateContext(doc_file)
    assert subdirs_exist(ctx) is True


def test_subdirs_exist_when_absent(tmp_path):
    """Test subdirs_exist returns False when no subdirectories exist."""
    doc_file = tmp_path / "CLAUDE.md"
    doc_file.write_text("# Test")

    # Create other files but no directories
    other_file = tmp_path / "other.txt"
    other_file.write_text("content")

    ctx = PredicateContext(doc_file)
    assert subdirs_exist(ctx) is False


def test_subdirs_exist_multiple_subdirs(tmp_path):
    """Test subdirs_exist returns True when multiple subdirectories exist."""
    doc_file = tmp_path / "CLAUDE.md"
    doc_file.write_text("# Test")

    (tmp_path / "dir1").mkdir()
    (tmp_path / "dir2").mkdir()
    (tmp_path / "dir3").mkdir()

    ctx = PredicateContext(doc_file)
    assert subdirs_exist(ctx) is True


# ============================================================================
# Expression Parser Tests
# ============================================================================

def test_parse_simple_function_call():
    """Test parsing simple function call with string argument."""
    func_name, kwargs = parse_predicate_expression('file_exists("README.md")')
    assert func_name == "file_exists"
    assert kwargs == {"path": "README.md"}


def test_parse_function_with_keyword_arg():
    """Test parsing function call with keyword argument."""
    func_name, kwargs = parse_predicate_expression('md_files_exist(exclude=["a.md"])')
    assert func_name == "md_files_exist"
    assert kwargs == {"exclude": ["a.md"]}


def test_parse_function_with_multiple_exclusions():
    """Test parsing function call with list of multiple items."""
    func_name, kwargs = parse_predicate_expression(
        'md_files_exist(exclude=["a.md", "b.md", "c.md"])'
    )
    assert func_name == "md_files_exist"
    assert kwargs == {"exclude": ["a.md", "b.md", "c.md"]}


def test_parse_function_with_empty_list():
    """Test parsing function call with empty list argument."""
    func_name, kwargs = parse_predicate_expression('md_files_exist(exclude=[])')
    assert func_name == "md_files_exist"
    assert kwargs == {"exclude": []}


def test_parse_function_no_args():
    """Test parsing function call with no arguments."""
    func_name, kwargs = parse_predicate_expression('subdirs_exist()')
    assert func_name == "subdirs_exist"
    assert kwargs == {}


def test_parse_section_present():
    """Test parsing section_present function call."""
    func_name, kwargs = parse_predicate_expression('section_present("Files")')
    assert func_name == "section_present"
    assert kwargs == {"name": "Files"}


def test_parse_invalid_function_name():
    """Test parsing raises error for unknown function."""
    with pytest.raises(PredicateParseError, match="Unknown predicate function: invalid_func"):
        parse_predicate_expression('invalid_func("arg")')


def test_parse_invalid_format_missing_parens():
    """Test parsing raises error for missing parentheses."""
    with pytest.raises(PredicateParseError, match="Invalid predicate expression format"):
        parse_predicate_expression('file_exists "README.md"')


def test_parse_invalid_format_no_closing_paren():
    """Test parsing raises error for missing closing parenthesis."""
    with pytest.raises(PredicateParseError, match="Invalid predicate expression format"):
        parse_predicate_expression('file_exists("README.md"')


def test_parse_invalid_value_format():
    """Test parsing raises error for unquoted string."""
    with pytest.raises(PredicateParseError, match="Invalid value format"):
        parse_predicate_expression('file_exists(README.md)')


# ============================================================================
# Integration Tests: evaluate_predicate
# ============================================================================

def test_evaluate_file_exists_integration(tmp_path):
    """Test full evaluation of file_exists predicate."""
    doc_file = tmp_path / "CLAUDE.md"
    doc_file.write_text("# Test")
    readme = tmp_path / "README.md"
    readme.write_text("# Readme")

    ctx = PredicateContext(doc_file)

    assert evaluate_predicate('file_exists("README.md")', ctx) is True
    assert evaluate_predicate('file_exists("MISSING.md")', ctx) is False


def test_evaluate_md_files_exist_integration(tmp_path):
    """Test full evaluation of md_files_exist predicate."""
    doc_file = tmp_path / "CLAUDE.md"
    doc_file.write_text("# Test")
    readme = tmp_path / "README.md"
    readme.write_text("# Readme")
    other = tmp_path / "other.md"
    other.write_text("# Other")

    ctx = PredicateContext(doc_file)

    # Should find other.md
    result = evaluate_predicate('md_files_exist(exclude=["CLAUDE.md", "README.md"])', ctx)
    assert result is True

    # Should find nothing
    result = evaluate_predicate(
        'md_files_exist(exclude=["CLAUDE.md", "README.md", "other.md"])', ctx
    )
    assert result is False


def test_evaluate_subdirs_exist_integration(tmp_path):
    """Test full evaluation of subdirs_exist predicate."""
    doc_file = tmp_path / "CLAUDE.md"
    doc_file.write_text("# Test")

    ctx = PredicateContext(doc_file)
    assert evaluate_predicate('subdirs_exist()', ctx) is False

    # Create subdirectory
    (tmp_path / "subdir").mkdir()
    assert evaluate_predicate('subdirs_exist()', ctx) is True


def test_evaluate_section_present_integration(tmp_path):
    """Test evaluation of section_present works with AST."""
    from markdown_parser import parse_markdown

    doc_file = tmp_path / "CLAUDE.md"
    doc_file.write_text("# Test\n## Files\n\nContent.\n\n## Summary\n\nMore content.")

    # Parse document to get AST
    text = doc_file.read_text()
    ast = parse_markdown(text)
    ctx = PredicateContext(doc_file, doc_ast=ast)

    # Should work with AST
    assert evaluate_predicate('section_present("Files")', ctx) is True
    assert evaluate_predicate('section_present("Summary")', ctx) is True
    assert evaluate_predicate('section_present("Missing")', ctx) is False


def test_evaluate_invalid_expression(tmp_path):
    """Test evaluation with malformed expression raises parse error."""
    doc_file = tmp_path / "CLAUDE.md"
    doc_file.write_text("# Test")

    ctx = PredicateContext(doc_file)

    with pytest.raises(PredicateParseError):
        evaluate_predicate('invalid expression format', ctx)


def test_evaluate_wrong_argument_type(tmp_path):
    """Test evaluation with wrong argument types raises error."""
    doc_file = tmp_path / "CLAUDE.md"
    doc_file.write_text("# Test")

    ctx = PredicateContext(doc_file)

    # file_exists expects 'path', not 'exclude'
    with pytest.raises(PredicateEvaluationError, match="Invalid arguments"):
        evaluate_predicate('file_exists(exclude=["test"])', ctx)


# ============================================================================
# Edge Case Tests
# ============================================================================

def test_context_with_nonexistent_doc_path():
    """Test PredicateContext with nonexistent document path."""
    # Should not raise error on construction
    ctx = PredicateContext("/nonexistent/path/doc.md")
    assert ctx.doc_path == Path("/nonexistent/path/doc.md")
    assert ctx.doc_dir == Path("/nonexistent/path")


def test_file_exists_with_relative_path_traversal(tmp_path):
    """Test file_exists with path traversal (..)."""
    # Create subdirectory structure
    subdir = tmp_path / "subdir"
    subdir.mkdir()
    doc_file = subdir / "CLAUDE.md"
    doc_file.write_text("# Test")

    # Create file at sibling directory
    other_dir = tmp_path / "other"
    other_dir.mkdir()
    other_file = other_dir / "file.txt"
    other_file.write_text("content")

    ctx = PredicateContext(doc_file)

    # File is in parent's sibling directory
    assert file_exists("../other/file.txt", ctx) is True
    assert file_exists("../other/missing.txt", ctx) is False


def test_md_files_exist_with_empty_exclude_list(tmp_path):
    """Test md_files_exist with explicitly empty exclude list."""
    doc_file = tmp_path / "CLAUDE.md"
    doc_file.write_text("# Test")
    other = tmp_path / "other.md"
    other.write_text("# Other")

    ctx = PredicateContext(doc_file)
    assert md_files_exist(exclude=[], ctx=ctx) is True


def test_whitespace_handling_in_expressions(tmp_path):
    """Test that expressions handle whitespace correctly."""
    doc_file = tmp_path / "CLAUDE.md"
    doc_file.write_text("# Test")
    readme = tmp_path / "README.md"
    readme.write_text("# Readme")

    ctx = PredicateContext(doc_file)

    # Extra whitespace should be handled
    assert evaluate_predicate('  file_exists("README.md")  ', ctx) is True
    assert evaluate_predicate('file_exists(  "README.md"  )', ctx) is True


def test_special_characters_in_filenames(tmp_path):
    """Test predicates with special characters in filenames."""
    doc_file = tmp_path / "CLAUDE.md"
    doc_file.write_text("# Test")

    # Create file with special characters
    special_file = tmp_path / "file-with_special.chars.md"
    special_file.write_text("# Special")

    ctx = PredicateContext(doc_file)
    assert evaluate_predicate('file_exists("file-with_special.chars.md")', ctx) is True


def test_case_sensitive_filenames(tmp_path):
    """Test that file checks are case-sensitive (on case-sensitive filesystems)."""
    doc_file = tmp_path / "CLAUDE.md"
    doc_file.write_text("# Test")
    readme = tmp_path / "README.md"
    readme.write_text("# Readme")

    ctx = PredicateContext(doc_file)

    # This behavior depends on filesystem, but we should check lowercase doesn't match
    # On case-insensitive filesystems (macOS default), this might pass
    result = evaluate_predicate('file_exists("readme.md")', ctx)
    # We can't assert specific behavior since it's filesystem-dependent
    assert isinstance(result, bool)


if __name__ == '__main__':
    pytest.main([__file__, '-v'])
