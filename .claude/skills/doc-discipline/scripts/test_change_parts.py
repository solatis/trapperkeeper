"""Tests of change_parts.py on diff text given as strings and on a throwaway git repository."""

import contextlib
import io
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import change_parts as cp  # noqa: E402

Span = cp.Span


def diff_of(path, *headers):
    """Return the text of a diff of one file with the given hunk headers and no body."""
    return "diff --git a/%s b/%s\n--- a/%s\n+++ b/%s\n%s\n" % (
        path, path, path, path, "\n".join(headers))


class HunkHeaders(unittest.TestCase):

    def test_with_count(self):
        self.assertEqual(cp.new_side("@@ -10,2 +12,3 @@ fn f()"), (12, 3))

    def test_without_count(self):
        self.assertEqual(cp.new_side("@@ -10 +12 @@"), (12, 1))

    def test_count_zero(self):
        self.assertEqual(cp.new_side("@@ -10,2 +9,0 @@"), (9, 0))

    def test_other_line(self):
        self.assertIsNone(cp.new_side("+@@ not a header"))


class DiffSpans(unittest.TestCase):

    def test_spans_and_changed_lines(self):
        text = diff_of("a.py", "@@ -1 +1,3 @@", "@@ -10,2 +12,0 @@", "@@ -20 +20 @@")
        self.assertEqual(cp.diff_spans(text), ([Span("a.py", 1, 3), Span("a.py", 20, 20)], 4))

    def test_added_line_that_looks_like_a_header(self):
        # An added line "++ b/x" shows as "+++ b/x" in the body and must not rename the file.
        text = diff_of("a.py", "@@ -0,0 +1,2 @@") + "+++ b/x\n+y\n@@ -5 +5 @@\n"
        self.assertEqual(cp.diff_spans(text)[0], [Span("a.py", 1, 2), Span("a.py", 5, 5)])

    def test_name_with_a_space(self):
        text = "diff --git a/a b b/a b\n--- a/a b\t\n+++ b/a b\t\n@@ -1 +1 @@\n"
        self.assertEqual(cp.diff_spans(text)[0], [Span("a b", 1, 1)])

    def test_skipped_files(self):
        skipped = ["Cargo.lock", "target/x.rs", "a/node_modules/m.js", "vendor/v.go",
                   "thirdparty/t.c", "third_party/t.c", "build/b.o", "dist/d.js",
                   ".git/config"]
        text = "".join(diff_of(p, "@@ -0,0 +1 @@") for p in skipped)
        text += diff_of("src/build.rs", "@@ -0,0 +1 @@")
        self.assertEqual(cp.diff_spans(text), ([Span("src/build.rs", 1, 1)], 1))

    def test_binary_file(self):
        text = "diff --git a/i.png b/i.png\nindex 1..2 100644\n" \
               "Binary files a/i.png and b/i.png differ\n"
        self.assertEqual(cp.diff_spans(text), ([], 0))


class Joining(unittest.TestCase):

    def test_close_spans_join(self):
        # Four unchanged lines, 4 to 7, lie between the spans.
        spans = [Span("a", 1, 3), Span("a", 8, 9)]
        self.assertEqual(cp.join_close(spans), [Span("a", 1, 9)])

    def test_far_spans_stay_apart(self):
        spans = [Span("a", 1, 3), Span("a", 9, 9)]
        self.assertEqual(cp.join_close(spans), spans)

    def test_spans_of_two_files_stay_apart(self):
        spans = [Span("a", 1, 3), Span("b", 4, 5)]
        self.assertEqual(cp.join_close(spans), spans)


class Filling(unittest.TestCase):

    def test_part_fills_up_to_the_size(self):
        spans = [Span("a", 1, 4), Span("a", 20, 25), Span("b", 1, 1)]
        self.assertEqual(cp.fill_parts(spans, 10),
                         [[Span("a", 1, 4), Span("a", 20, 25)], [Span("b", 1, 1)]])

    def test_span_over_the_size_takes_a_part_of_its_own(self):
        spans = [Span("a", 1, 2), Span("b", 1, 12), Span("c", 1, 2)]
        self.assertEqual(cp.fill_parts(spans, 10),
                         [[Span("a", 1, 2)], [Span("b", 1, 12)], [Span("c", 1, 2)]])


class Cutting(unittest.TestCase):

    def test_cut_at_the_nearest_blank_line(self):
        lines = ["x"] * 100
        lines[7] = ""
        lines[13] = ""
        # Line 8 lies 2 lines from the first target 10. The second target is 18,
        # and line 14 is the only blank line in 9..19.
        self.assertEqual(cp.cut_span(Span("a", 1, 20), lines, 10),
                         [Span("a", 1, 8), Span("a", 9, 14), Span("a", 15, 20)])

    def test_cut_prefers_the_earlier_of_two_blank_lines(self):
        lines = ["x"] * 30
        lines[8] = ""
        lines[10] = ""
        self.assertEqual(cp.cut_span(Span("a", 1, 20), lines, 10)[0], Span("a", 1, 9))

    def test_cut_without_a_blank_line_falls_at_the_size(self):
        lines = ["x"] * 100
        self.assertEqual(cp.cut_span(Span("a", 1, 25), lines, 10),
                         [Span("a", 1, 10), Span("a", 11, 20), Span("a", 21, 25)])

    def test_cut_ignores_a_blank_line_outside_the_window(self):
        lines = ["x"] * 200
        lines[150] = ""
        self.assertEqual(cp.cut_span(Span("a", 1, 200), lines, 100)[0], Span("a", 1, 100))

    def test_cut_never_falls_at_the_last_line_of_the_span(self):
        # A cut at the blank line 12 would leave an empty piece after it.
        lines = ["x"] * 12
        lines[11] = ""
        self.assertEqual(cp.cut_span(Span("a", 1, 12), lines, 10),
                         [Span("a", 1, 10), Span("a", 11, 12)])

    def test_span_at_the_size_is_not_cut(self):
        self.assertEqual(cp.cut_span(Span("a", 1, 10), ["x"] * 10, 10), [Span("a", 1, 10)])


class Output(unittest.TestCase):

    def test_list_lines(self):
        parts = [[Span("a", 1, 4), Span("b", 2, 3)], [Span("c", 5, 5)]]
        self.assertEqual(cp.list_lines(parts, 6, 3), [
            "part 1/2: 6 lines: a:1-4, b:2-3",
            "part 2/2: 1 lines: c:5-5",
            "change_parts: 2 parts, 6 changed lines in 3 files"])

    def test_list_lines_of_an_empty_change(self):
        self.assertEqual(cp.list_lines([], 0, 0),
                         ["change_parts: 0 parts, 0 changed lines in 0 files"])


def git_in(repo, *args):
    """Run git in `repo` without the user's configuration and return its stripped stdout."""
    env = dict(os.environ, GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_NOSYSTEM="1",
               GIT_AUTHOR_NAME="t", GIT_AUTHOR_EMAIL="t@example.com",
               GIT_COMMITTER_NAME="t", GIT_COMMITTER_EMAIL="t@example.com")
    return subprocess.run(["git"] + list(args), cwd=repo, env=env, check=True,
                          capture_output=True, text=True).stdout.strip()


def write(repo, path, text):
    full = os.path.join(repo, path)
    os.makedirs(os.path.dirname(full), exist_ok=True)
    with open(full, "w") as f:
        f.write(text)


@contextlib.contextmanager
def working_directory(path):
    before = os.getcwd()
    os.chdir(path)
    try:
        yield
    finally:
        os.chdir(before)


def main_output(argv):
    """Return (exit status, stdout lines, stderr text) of one run of main."""
    out, err = io.StringIO(), io.StringIO()
    with redirect_stdout(out), redirect_stderr(err):
        try:
            status = cp.main(argv)
        except SystemExit as e:
            status = e.code
    return status, out.getvalue().splitlines(), err.getvalue()


def numbered(prefix, n):
    return "".join("%s %d\n" % (prefix, i) for i in range(1, n + 1))


# Two functions of 30 lines each, with a blank line 31 between them.
TWO_FUNCTIONS = numbered("f", 30) + "\n" + numbered("g", 30)


@unittest.skipUnless(shutil.which("git"), "git is not on PATH")
class GitRange(unittest.TestCase):

    def setUp(self):
        # The base holds a file that the branch renames and edits, and a file
        # that it deletes. The branch adds a long file, a lock file and a file
        # whose last line has no newline.
        self.tmp = tempfile.TemporaryDirectory()
        repo = self.repo = self.tmp.name
        git_in(repo, "init", "-q")
        write(repo, "old.py", numbered("line", 20))
        write(repo, "gone.py", "x\n")
        git_in(repo, "add", "-A")
        git_in(repo, "commit", "-q", "-m", "base")
        self.base = git_in(repo, "rev-parse", "HEAD")
        git_in(repo, "mv", "old.py", "renamed.py")
        write(repo, "renamed.py", numbered("line", 20).replace("line 7\n", "changed 7\n"))
        git_in(repo, "rm", "-q", "gone.py")
        write(repo, "long.py", TWO_FUNCTIONS)
        write(repo, "Cargo.lock", "lock\n")
        write(repo, "tail.py", "a\nb")
        git_in(repo, "add", "-A")
        git_in(repo, "commit", "-q", "-m", "branch")
        self.rev = git_in(repo, "rev-parse", "HEAD")

    def tearDown(self):
        self.tmp.cleanup()

    def run_main(self, *argv):
        with working_directory(self.repo):
            return main_output(list(argv))

    def test_list_form(self):
        status, lines, err = self.run_main(self.base + "..." + self.rev, "--size", "31")
        self.assertEqual((status, err), (0, ""))
        self.assertEqual(lines, [
            "part 1/3: 31 lines: long.py:1-31",
            "part 2/3: 31 lines: long.py:32-61, renamed.py:7-7",
            "part 3/3: 2 lines: tail.py:1-2",
            "change_parts: 3 parts, 64 changed lines in 3 files"])

    def test_spans_of_one_part(self):
        status, lines, _ = self.run_main(self.base + "..." + self.rev, "2", "--size", "31")
        self.assertEqual((status, lines), (0, ["long.py:32-61", "renamed.py:7-7"]))

    def test_part_out_of_range(self):
        for k in ("0", "4"):
            status, lines, err = self.run_main(self.base + "..." + self.rev, k, "--size=31")
            self.assertEqual((status, lines), (2, []))
            self.assertEqual(err, "change_parts: no part %s, the change has 3 parts\n" % k)

    def test_empty_change(self):
        status, lines, _ = self.run_main(self.rev + "..." + self.rev)
        self.assertEqual((status, lines),
                         (0, ["change_parts: 0 parts, 0 changed lines in 0 files"]))
        status, _, err = self.run_main(self.rev + "..." + self.rev, "1")
        self.assertEqual((status, err), (2, "change_parts: no part 1, the change has 0 parts\n"))

    def test_malformed_range(self):
        for bad in ("a..b", "a...-b", "a....b", "a..."):
            status, lines, err = self.run_main(bad)
            self.assertEqual((status, lines), (2, []), bad)
            self.assertTrue(err.startswith("change_parts: malformed range"), err)
            self.assertEqual(err.count("\n"), 1)

    def test_range_that_starts_with_a_dash(self):
        # argparse reads "-a...b" as an unknown option, which is also a usage error.
        status, lines, err = self.run_main("-a...b")
        self.assertEqual((status, lines), (2, []))
        self.assertTrue(err.startswith("change_parts: "), err)
        self.assertEqual(err.count("\n"), 1)

    def test_unknown_revision(self):
        status, lines, err = self.run_main(self.base + "...nosuchrev")
        self.assertEqual((status, lines), (2, []))
        self.assertTrue(err.startswith("change_parts: git diff failed: "), err)
        self.assertEqual(err.count("\n"), 1)

    def test_bad_size(self):
        status, _, err = self.run_main(self.base + "..." + self.rev, "--size", "0")
        self.assertEqual((status, err), (2, "change_parts: --size must be at least 1\n"))


if __name__ == "__main__":
    unittest.main()
