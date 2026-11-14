"""Tests of check_steps.py on snippets in several comment syntaxes and on a git range."""

import contextlib
import io
import os
import re
import shutil
import subprocess
import sys
import tempfile
import textwrap
import unittest
from contextlib import redirect_stderr, redirect_stdout

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import check_steps as cs  # noqa: E402


def run(src, ext=".rs", warn_long=None, deny=(), tab_width=4, warn_text=False):
    """Return the findings for a snippet as (line, rule) pairs, sorted."""
    opts = cs.Options(tab_width, [re.compile(d) for d in deny], warn_long, warn_text)
    lang = cs.LANGS[cs.EXTENSIONS[ext]]
    lines = textwrap.dedent(src).strip("\n").split("\n")
    return sorted((f.line, f.rule) for f in cs.check_lines("x" + ext, lines, lang, opts))


def rules(src, ext=".rs", **kw):
    return sorted(r for _, r in run(src, ext, **kw))


def main_output(argv):
    """Return (exit status, stdout lines, stderr text) of one run of main."""
    out, err = io.StringIO(), io.StringIO()
    with redirect_stdout(out), redirect_stderr(err):
        try:
            status = cs.main(argv)
        except SystemExit as e:
            status = e.code
    return status, out.getvalue().splitlines(), err.getvalue()


PARSE = """
pub fn parse(text: &str) -> Result<Expr, SyntaxError> {
    // Parsing works as follows:
    //
    // 1. We split the text into tokens. `lex` does not fail. A text it
    //    cannot split ends in a `Bad` token, and the grammar reports it only
    //    when it gets there, so errors come in the order of the text.
    // 2. We run the grammar from the first token. It gives a tree, no
    //    match, or a refusal such as `UnknownFunction` for `lenn(name) > 3`.
    // 3. On a tree, we resolve the names.
    // 4. Otherwise we build the error from the refusal and from the log of
    //    the token tests.

    // 1. split into tokens
    let input = Input::new(text, lex(text));

    // 2. run the grammar
    let refusal = match rule(&input, 0) {
        Ok(Some((mut tree, _))) => {
            // 3. resolve the names
            resolve(&mut tree, &mut Vec::new());
            return Ok(tree);
        }
        // Without a match, the log is the only source of the error, as for
        // `a > 1 b`.
        Ok(None) => None,
        Err(refusal) => Some(refusal),
    };

    // 4. build the error
    let error = explain(&input, refusal);
    Err(error)
}
"""


class ParseExample(unittest.TestCase):
    def test_reference_shape_passes(self):
        self.assertEqual(run(PARSE), [])

    def test_old_inverted_order_fails(self):
        src = PARSE.replace("// 3. resolve the names", "// 2. resolve the names")
        self.assertEqual(rules(src), ["N2", "N2"])

    def test_missing_backref(self):
        src = PARSE.replace("            // 3. resolve the names\n", "")
        self.assertEqual(run(src), [(9, "N2")])


class N1ListNumbering(unittest.TestCase):
    def test_gap(self):
        src = """
        fn f() {
            // 1. a.
            // 2. b.
            // 4. d.

            // 1. a
            a();
            // 2. b
            b();
            // 4. d
            d();
        }
        """
        self.assertIn((2, "N1"), run(src))

    def test_repeat(self):
        src = """
        func f() {
        \t// 1. a.
        \t// 2. b.
        \t// 2. c.
        \t// 1. a
        \ta()
        \t// 2. b
        \tb()
        }
        """
        self.assertIn((2, "N1"), run(src, ".go"))

    def test_list_not_from_one_is_n7(self):
        src = """
        fn f() {
            // 3. On a tree, resolve the names.
            // 4. Otherwise build the error.
            a();
        }
        """
        self.assertEqual(run(src), [(2, "N4"), (2, "N7")])


class N2Count(unittest.TestCase):
    PY = """
    def f(x):
        # Two steps:
        #
        # 1. Load the data.
        # 2. Store it.

        # 1. load
        d = load(x)
        # 2. store
        store(d)
    """

    def test_python_passes(self):
        self.assertEqual(run(self.PY, ".py"), [])

    def test_python_missing(self):
        self.assertEqual(run(self.PY.replace("    # 2. store\n", ""), ".py"), [(5, "N2")])

    def test_python_duplicate(self):
        src = self.PY + "    # 2. store again\n    store(d)\n"
        self.assertEqual(run(src, ".py"), [(11, "N2")])

    def test_list_without_any_backref(self):
        src = """
        int f() {
            // 1. Read the header.
            // 2. Read the body.
            read();
        }
        """
        self.assertEqual(run(src, ".cpp"), [(2, "N2"), (3, "N2")])


class N3Order(unittest.TestCase):
    def test_clojure_order(self):
        src = """
        (defn f [x]
          ;; 1. Parse it.
          ;; 2. Check it.
          ;; 3. Store it.

          ;; 1. parse
          (let [p (parse x)]
            ;; 3. store
            (store p)
            ;; 2. check
            (check p)))
        """
        self.assertEqual(run(src, ".clj"), [(10, "N3")])

    def test_clojure_passes(self):
        src = """
        (defn f [x]
          ;; 1. Parse it.
          ;; 2. Store it.

          ;; 1. parse
          (let [p (parse x)]
            ;; 2. store
            (store p)))
        """
        self.assertEqual(run(src, ".clj"), [])


class N4Orphan(unittest.TestCase):
    TWO_FUNCTIONS = """
    fn a() {
        // 1. One.
        // 2. Two.

        // 1. one
        one();
        // 2. two
        two();
    }

    fn b() {
        // 2. something
        x();
    }

    fn c() {
        // 1. One.
        // 2. Two.

        // 1. one
        one();
        // 2. two
        two();
    }
    """

    def test_stray_between_two_functions(self):
        self.assertEqual(run(self.TWO_FUNCTIONS), [(12, "N4")])

    def test_number_not_in_list(self):
        src = """
        fn a() {
            // 1. One.
            // 2. Two.

            // 1. one
            one();
            // 2. two
            two();
            // 3. three
            three();
        }
        """
        self.assertEqual(run(src), [(9, "N4")])

    def test_haskell_without_list(self):
        src = """
        f :: Int -> Int
        f x =
          -- 1. double
          let y = x * 2
          -- 2. add
          in y + 1
        """
        self.assertEqual(run(src, ".hs"), [(3, "N4"), (5, "N4")])

    def test_haskell_passes(self):
        src = """
        f :: Int -> Int
        f x =
          -- 1. Double the input.
          -- 2. Add one.

          -- 1. double
          let y = x * 2
          -- 2. add
          in y + 1
        """
        self.assertEqual(run(src, ".hs"), [])


class N5N6Shape(unittest.TestCase):
    BASE = """
    fn f() {
        // 1. One.
        // 2. Two.
        // 3. Three.

        // 1. one
        one();
        // 2. two
        two();
        // 3. three
        three();
    }
    """

    def test_bare(self):
        self.assertEqual(run(self.BASE.replace("// 2. two", "// 2.")), [(8, "N5")])

    def test_combined(self):
        src = self.BASE.replace("// 2. two", "// 2. and 3.").replace(
            "        // 3. three\n", "")
        self.assertEqual(run(src), [(8, "N5"), (8, "N6")])

    def test_combined_dash(self):
        src = self.BASE.replace("// 2. two", "// 2-3. both").replace(
            "        // 3. three\n", "")
        self.assertEqual(run(src), [(8, "N6")])

    def test_erlang_bare(self):
        src = """
        f(X) ->
            %% 1. Check the input.
            %% 2. Store it.

            %% 1.
            ok = check(X),
            %% 2. store
            store(X).
        """
        self.assertEqual(run(src, ".erl"), [(5, "N5")])

    def test_erlang_passes(self):
        src = """
        %% @doc Store X.
        %% 1. this is edoc and is skipped.
        %% 2. so is this.
        f(X) ->
            %% 1. Check the input.
            %% 2. Store it.

            %% 1. check
            ok = check(X),
            %% 2. store
            store(X).
        """
        self.assertEqual(run(src, ".erl"), [])


class N7LongNumbered(unittest.TestCase):
    def test_inverted_shape(self):
        src = """
        fn f() {
            // 1. One.
            // 2. Two.

            // 1. We read the header first, because the body
            //    length is in it.
            one();
            // 2. two
            two();
        }
        """
        self.assertEqual(run(src), [(5, "N7")])

    def test_go_passes_and_fails(self):
        ok = """
        func f() {
        \t// The function works in two steps:
        \t//
        \t//  1. Pin the buffers, because the C side keeps their
        \t//     addresses.
        \t//  2. Call C.
        \tx := 1
        \t// 1. pin
        \tpin(x)
        \t// 2. call
        \tcall(x)
        }
        """
        self.assertEqual(run(ok, ".go"), [])
        bad = ok.replace("\t// 2. call\n",
                         "\t// 2. call C. The C side reads\n        \t//    the buffers.\n")
        self.assertEqual(run(bad, ".go"), [(10, "N7")])


class WrappedSentence(unittest.TestCase):
    def test_prose_line_starting_with_number(self):
        src = """
        fn f() {
            // `isinstance(price, number)` is refused at `number`, bytes 18 to
            // 24.
            x();
        }
        """
        self.assertEqual(run(src), [])

    def test_item_wrapping_onto_number(self):
        src = """
        fn f() {
            // 1. Read the header, which takes
            //    24 bytes.
            // 2. Read the body, whose length is the last field of the
            // 24. That wrap sits at the item's column but the line above
            //    does not end a sentence.
            // 3. Close.

            // 1. header
            a();
            // 2. body
            b();
            // 3. close
            c();
        }
        """
        self.assertEqual(run(src), [])

    def test_item_wrapping_onto_number_after_continuation(self):
        src = """
        fn f() {
            // 1. Read the header, which takes up to
            //    24. bytes in the worst case.
            // 2. Read the body.

            // 1. header
            a();
            // 2. body
            b();
        }
        """
        self.assertEqual(run(src), [])


class ScopeAndIntroductions(unittest.TestCase):
    def test_intro_and_definition(self):
        src = """
        fn f() {
            // Merging works in steps. A run is a sorted slice of the input.
            // A run is closed when the next value is smaller.
            //
            // 1. Find the runs.
            // 2. Merge them pairwise.

            // 1. find runs
            let runs = find(x);
            // 2. merge
            merge(runs)
        }
        """
        self.assertEqual(run(src), [])

    def test_prose_only(self):
        src = """
        fn f() {
            // This function only forwards its argument, because the caller
            // owns the buffer.
            g(x)
        }
        """
        self.assertEqual(run(src), [])

    def test_python_nested_def(self):
        src = """
        def outer(xs):
            # 1. Build the key function.
            # 2. Sort with it.

            # 1. key function
            def key(x):
                # 1. Split the name.
                # 2. Lower-case it.

                # 1. split
                parts = x.split()
                # 2. lower
                return parts[0].lower()

            # 2. sort
            return sorted(xs, key=key)
        """
        self.assertEqual(run(src, ".py"), [])

    def test_python_nested_def_missing_inner(self):
        src = """
        def outer(xs):
            # 1. Build the key function.
            # 2. Sort with it.

            # 1. key function
            def key(x):
                # 1. Split the name.
                # 2. Lower-case it.

                # 1. split
                return x.split()[0].lower()

            # 2. sort
            return sorted(xs, key=key)
        """
        self.assertEqual(run(src, ".py"), [(8, "N2")])

    def test_python_docstring_list(self):
        src = '''
        def f(x):
            """Do it.

            # 1. this is not a comment, it is in the docstring
            1. Steps of the contract.
            2. More.
            """
            # 1. stray
            return x
        '''
        self.assertEqual(run(src, ".py"), [(8, "N4")])

    def test_rust_doc_comment_list(self):
        src = """
        /// Reads a value.
        ///
        /// 1. first
        /// 2. second
        fn f() {
            //! 1. inner doc
            x();
        }
        """
        self.assertEqual(run(src), [])

    def test_cpp_method_doc_comment(self):
        src = """
        class A {
            // Reads a value:
            // 1. the header
            // 2. the body
            int read(int fd) const;

            // Same, defined inline:
            // 1. the header
            // 2. the body
            int read2(int fd) {
                return 0;
            }
        };
        """
        self.assertEqual(run(src, ".hpp"), [])

    def test_cpp_preprocessor_at_column_zero(self):
        src = """
        int f() {
            // 1. One.
            // 2. Two.

            // 1. one
            one();
        #ifdef X
            // 2. two
            two();
        #endif
        }
        """
        self.assertEqual(run(src, ".cpp"), [])

    def test_tabs_go(self):
        src = "func f() {\n\t// 1. A.\n\t// 2. B.\n\tif x {\n\t\t// 1. a\n\t\ta()\n\t}\n" \
              "\t// 2. b\n\tb()\n}\n"
        self.assertEqual(run(src, ".go"), [])

    def test_test_scenario_list(self):
        src = """
        #[test]
        fn login_after_rotation() {
            // The scenario:
            //
            // 1. A user logs in with the first key.
            // 2. The key rotates.
            // 3. The old token still verifies.

            // 1. log in
            let t = login();
            // 2. rotate
            rotate();
            // 3. verify the old token
            assert!(verify(t));
        }
        """
        self.assertEqual(run(src), [])


class StringLiterals(unittest.TestCase):
    def test_single_line_string_is_not_a_comment(self):
        src = """
        fn f() {
            let s = "// 1. text";
        }
        """
        self.assertEqual(run(src), [])

    def test_rust_raw_string_is_skipped(self):
        src = '''
        fn f() {
            let s = r#"
            // 1. text
            "#;
        }
        '''
        self.assertEqual(run(src), [])

    def test_go_backtick_string_is_skipped(self):
        src = "func f() {\n\ts := `\n\t// 1. text\n\t`\n}\n"
        self.assertEqual(run(src, ".go"), [])

    def test_shell_heredoc_is_skipped(self):
        src = """
        f() {
          cat <<EOF
          # 1. text
        EOF
        }
        """
        self.assertEqual(run(src, ".sh"), [])

    def test_rust_plain_multiline_string_is_skipped(self):
        src = """
        fn f() {
            let s = "
            // 1. text
            ";
        }
        """
        self.assertEqual(run(src), [])

    def test_c_block_comment_is_skipped(self):
        src = """
        int f() {
            /*
            // 1. text
            */
            return 0;
        }
        """
        self.assertEqual(run(src, ".cpp"), [])


class Extras(unittest.TestCase):
    SRC = """
    fn f() {
        // 1. One, see doc/adr/0005-x.md.
        // 2. Two.

        // 1. one with a back-reference that is far too long to read
        one();
        // 2. two
        two();
    }
    """

    def test_off_by_default(self):
        self.assertEqual(run(self.SRC), [])

    def test_warn_long(self):
        self.assertEqual(run(self.SRC, warn_long=8), [(5, "W1")])

    def test_deny(self):
        self.assertEqual(run(self.SRC, deny=[r"doc/adr/"]), [(2, "D1")])


class Markdown(unittest.TestCase):
    def test_fenced_blocks(self):
        md = "# Plan\n\n```rust\n" + textwrap.dedent(PARSE).strip("\n") + \
             "\n```\n\n```go\nfunc f() {\n\t// 2.\n}\n```\n\n```text\n// 2.\n```\n"
        with tempfile.TemporaryDirectory() as d:
            p = os.path.join(d, "plan.md")
            with open(p, "w") as f:
                f.write(md)
            out = io.StringIO()
            with redirect_stdout(out):
                status = cs.main(["--markdown", p])
        lines = out.getvalue().splitlines()
        go_line = md.splitlines().index("\t// 2.") + 1
        self.assertEqual(status, 1)
        self.assertEqual(lines[-1], "check_steps: 2 findings in 1 file")
        self.assertEqual(lines[0], "%s:%d: N4: step 2 is outside the scope of any walkthrough"
                         % (p, go_line))
        self.assertIn(":%d: N5:" % go_line, lines[1])


class CommandLine(unittest.TestCase):
    def test_walk_and_exit_codes(self):
        with tempfile.TemporaryDirectory() as d:
            os.makedirs(os.path.join(d, "target"))
            with open(os.path.join(d, "target", "x.rs"), "w") as f:
                f.write("fn f() {\n    // 2.\n}\n")
            with open(os.path.join(d, "ok.rs"), "w") as f:
                f.write(textwrap.dedent(PARSE))
            with open(os.path.join(d, "notes.txt"), "w") as f:
                f.write("// 2.\n")
            out = io.StringIO()
            with redirect_stdout(out):
                self.assertEqual(cs.main([d]), 0)
            self.assertEqual(out.getvalue(), "check_steps: 0 findings in 1 file\n")

    def test_usage_error(self):
        self.assertEqual(main_output(["/nonexistent/path"])[0], 2)


class Adjacency(unittest.TestCase):
    def test_backref_glued_to_list_is_read_as_part_of_it(self):
        # Without an empty line or code between them, the back-reference joins
        # the list block, and the list reads 1 2 1 2.
        src = """
        fn f() {
            // 1. One.
            // 2. Two.
            // 1. one
            one();
            // 2. two
            two();
        }
        """
        self.assertEqual(run(src)[0], (2, "N1"))

    def test_prose_after_list_needs_an_empty_line(self):
        src = """
        fn f() {
            // 1. One.
            // 2. Two.
            //
            // A closing paragraph.
            x();
            // 1. one
            one();
            // 2. two
            two();
        }
        """
        self.assertEqual(run(src), [])

    def test_cpp_go_style_walkthrough_directly_above_code(self):
        src = """
        int f() {
            // The function works in two steps:
            //
            //  1. Read.
            //  2. Write.
            int x = 0;
            // 1. read
            read(x);
            // 2. write
            return write(x);
        }
        """
        self.assertEqual(run(src, ".cpp"), [])


class WarnArguments(unittest.TestCase):
    SRC = textwrap.dedent("""\
        fn f() {
            // 1. Open the file, because the reader needs it.
            // 2. Close the file, so that the handle is freed.

            // 1. open the file for reading
            open();
            // 2. close the file
            close();
        }
        """)

    def run_main(self, flags):
        with tempfile.TemporaryDirectory() as d:
            path = os.path.join(d, "x.rs")
            with open(path, "w") as f:
                f.write(self.SRC)
            return main_output(flags + [path])

    def test_warn_turns_on_all_warnings_with_limit_eight(self):
        status, lines, _ = self.run_main(["--warn"])
        self.assertEqual(status, 0)
        self.assertEqual(lines[-1], "check_steps: 0 findings, 0 warnings in 1 file")

    def test_warn_long_sets_the_limit(self):
        status, lines, _ = self.run_main(["--warn-long", "3"])
        self.assertEqual(status, 0)
        self.assertIn(":5: W1: back-reference has 5 words, more than 3", lines[0])
        self.assertEqual(lines[-1], "check_steps: 0 findings, 1 warning in 1 file")


class ItemAfterItemNeedsSentenceEnd(unittest.TestCase):
    def test_wrapped_number_after_open_line_is_prose(self):
        src = """
        fn h(n: usize) -> usize {
            // The steps:
            //
            // 1. We reserve room for the header, which is
            //    always 24 bytes. The tail adds up to
            // 2. more bytes for the checksum.
            // 2. We copy.

            // 1. reserve
            let a = n + 24;
            // 2. copy
            a
        }
        """
        self.assertEqual(run(src), [])

    def test_wrapped_number_equal_to_the_next_step_is_prose(self):
        src = """
        fn o() {
            // Reading works so:
            //
            // 1. We read the header. Its length field counts in units of
            // 4. bytes, so we multiply.
            // 2. We read the body.

            // 1. read the header
            header();
            // 2. read the body
            body();
        }
        """
        self.assertEqual(run(src), [])

    def test_semicolon_ends_an_item(self):
        src = """
        func f() {
        \t// The checks:
        \t//  1. the index is present;
        \t//  2. no batch is empty.
        \tx := 1
        \t// 1. index
        \tindex(x)
        \t// 2. batches
        \tbatches(x)
        }
        """
        self.assertEqual(run(src, ".go"), [])


class OnlyCodeEndsAScope(unittest.TestCase):
    def test_python_comment_at_column_zero(self):
        src = """
        def run(x):
            # It works so:
            #
            # 1. We parse.
            # 2. We emit.

            # 1. parse
            y = int(x)
        # TODO remove
            # 2. emit
            return y
        """
        self.assertEqual(run(src, ".py"), [])

    def test_go_label_outdented_by_gofmt(self):
        src = """
        package p

        func F(xs []int) int {
        \t// It works so:
        \t//
        \t// 1. We scan.
        \t// 2. We sum.

        \t// 1. scan
        outer:
        \tfor _, x := range xs {
        \t\tif x < 0 {
        \t\t\tbreak outer
        \t\t}
        \t}
        \t// 2. sum
        \treturn 0
        }
        """
        self.assertEqual(run(src, ".go"), [])

    def test_ref_after_the_body_is_not_in_an_inner_scope(self):
        src = """
        def outer(xs):
            # 1. Build the key function.
            # 2. Sort with it.

            # 1. key function
            def key(x):
                # 1. Split the name.
                # 2. Lower-case it.

                # 1. split
                parts = x.split()
                # 2. lower
                return parts[0].lower()
            # 2. sort
            return sorted(xs, key=key)
        """
        self.assertEqual(run(src, ".py"), [])


class StringsThatSpanLines(unittest.TestCase):
    def test_rust_string_opened_at_column_zero(self):
        src = """
        fn g(v: &str) -> String {
            // It works so:
            //
            // 1. We format.
            // 2. We trim.

            // 1. format
            let s = format!(
        "{}
          x", v);
            // 2. trim
            s.trim().to_string()
        }
        """
        self.assertEqual(run(src), [])

    def test_char_literal_holding_a_quote_opens_no_string(self):
        src = """
        int f(char c) {
            // 1. One.
            // 2. Two.

            // 1. one
            if (c == '"') return 1;
            // 2. two
            return "\\"" [0];
        }
        """
        self.assertEqual(run(src, ".cpp"), [])

    def test_cpp_raw_string(self):
        src = """
        int run() {
            // It works so:
            //
            // 1. We set up.
            // 2. We return.

            // 1. set up
            const char* s = R"(
        // 7. fake
        )";
            // 2. return
            return 0;
        }
        """
        self.assertEqual(run(src, ".cpp"), [])

    def test_rust_byte_raw_string(self):
        src = """
        fn k() {
            // It works so:
            //
            // 1. We load.
            // 2. We check.

            // 1. load
            let b = br#"
            // 2. fake
            "#;
            // 3. check
            check(b);
        }
        """
        self.assertEqual(run(src), [(5, "N2"), (11, "N4")])

    def test_go_rune_holding_a_backtick(self):
        src = """
        package p

        func G(c byte) int {
        \t// It works so:
        \t//
        \t// 1. We test.
        \t// 2. We count.

        \t// 1. test
        \tif c == '`' {
        \t\treturn 1
        \t}
        \t// 3. count
        \treturn 0
        }
        """
        self.assertEqual(run(src, ".go"), [(7, "N2"), (13, "N4")])

    def test_heredoc_needs_an_upper_case_tag(self):
        src = """
        f() {
          # 1. Collect.
          # 2. Print.

          # 1. collect
          out=$(( x <<shift ))
          # 2. print
          cat <<END
          # 3. text of the heredoc
        END
        }
        """
        self.assertEqual(run(src, ".sh"), [])

    def test_ruby_is_not_checked(self):
        self.assertNotIn(".rb", cs.EXTENSIONS)
        with tempfile.TemporaryDirectory() as d:
            with open(os.path.join(d, "push.rb"), "w") as f:
                f.write("def run(xs)\n  # 1. We collect.\n  # 2. We sum.\n\n"
                        "  # 3. sum\n  out << xs.first\nend\n")
            status, lines, _ = main_output([d])
        self.assertEqual((status, lines), (0, ["check_steps: 0 findings in 0 files"]))


class TrailingBackReference(unittest.TestCase):
    def test_trailing_refs_are_n8_and_count_for_their_steps(self):
        src = """
        fn t() {
            // How:
            //
            // 1. We open.
            // 2. We close.

            let x = open(); // 1. open
            close(x); // 2. close
        }
        """
        self.assertEqual(run(src), [(7, "N8"), (8, "N8")])

    def test_comment_marker_inside_a_string_is_code(self):
        src = """
        fn t() {
            let url = "http://host/1. path"; // the host
            let s = "a // 1. b";
        }
        """
        self.assertEqual(run(src), [])

    def test_python_trailing_ref(self):
        self.assertEqual(run("x = '#'  # 2. store\n", ".py"), [(1, "N4"), (1, "N8")])


class NumberedListInANote(unittest.TestCase):
    def test_list_right_below_code_in_a_scope(self):
        src = """
        fn n(x: u32) -> u32 {
            // It works so:
            //
            // 1. We clamp.
            // 2. We scale.

            // 1. clamp
            let y = x.min(7);
            // The limit is 7 for two reasons:
            // 1. the table has 8 rows;
            // 2. the hardware caps it.
            // 2. scale
            y * 2
        }
        """
        self.assertEqual(run(src), [(10, "N7")])

    def test_back_references_glued_together(self):
        src = """
        fn w() {
            // How:
            //
            // 1. We open.
            // 2. We close.

            // 1. open
            // 2. close
            let x = open();
            close(x);
        }
        """
        self.assertEqual(run(src), [(7, "N7")])

    def test_two_walkthroughs_in_one_function_stay_walkthroughs(self):
        src = """
        fn q() {
            // First part:
            //
            // 1. We open.
            // 2. We read.

            // 1. open
            open();
            // 2. read
            read();

            // Second part:
            //
            // 1. We parse.
            // 2. We close.

            // 1. parse
            parse();
            // 2. close
            close();
        }
        """
        self.assertEqual(run(src), [])


class Warnings(unittest.TestCase):
    def test_items_without_reasons(self):
        src = """
        fn m() {
            // 1. open
            // 2. close

            // 1. open
            open();
            // 2. close
            close();
        }
        """
        self.assertEqual(run(src), [(2, "N7")])
        self.assertEqual(run(src, warn_text=True), [(2, "N7"), (2, "W2"), (3, "W2")])

    def test_item_equal_to_its_back_reference(self):
        src = """
        fn m() {
            // 1. Open.
            // 2. Close the file, so that the handle is freed.

            // 1. open
            open();
            // 2. close
            close();
        }
        """
        self.assertEqual(run(src), [])
        self.assertEqual(run(src, warn_text=True), [(2, "W2")])

    def test_back_reference_without_a_word_of_its_item(self):
        src = """
        fn u() {
            // How:
            //
            // 1. We open.
            // 2. We close.

            // 1. close
            open();
            // 2. open
            close();
        }
        """
        self.assertEqual(run(src), [])
        self.assertEqual(run(src, warn_text=True), [(7, "W3"), (9, "W3")])

    def test_warnings_do_not_change_the_exit_status(self):
        with tempfile.TemporaryDirectory() as d:
            path = os.path.join(d, "x.rs")
            with open(path, "w") as f:
                f.write("fn m() {\n    // 1. Open.\n    // 2. Close.\n\n    // 1. open\n"
                        "    open();\n    // 2. close\n    close();\n}\n")
            status, lines, _ = main_output(["--warn", path])
        self.assertEqual(status, 0)
        self.assertEqual(lines[-1], "check_steps: 0 findings, 2 warnings in 1 file")


class ProseAboveANumberedList(unittest.TestCase):
    def test_intro_ending_in_a_period_starts_a_walkthrough(self):
        src = """
        fn s(xs: &[u32]) -> u32 {
            // The function sums the even values in two steps.
            // 1. It filters the odd values out, because they do not count.
            // 2. It adds the rest, because the caller wants the total.

            // 1. filter the odd values
            let ev = xs.iter().filter(|x| *x % 2 == 0);
            // 2. add the rest
            ev.sum()
        }
        """
        self.assertEqual(run(src), [])

    def test_lists_below_code_after_prose_are_reported(self):
        src = """
        package p

        func cfg() {
        \tx := load()
        \t// Order of precedence
        \t// 1. flags
        \t// 2. env
        \tuse(x)
        \t// The fields are read in this order.
        \t// 1. The name.
        \t// 2. The type.
        \tuse(x)
        }
        """
        self.assertEqual(run(src, ".go"), [(6, "N7"), (10, "N7")])

    def test_intro_without_punctuation_above_sentences(self):
        src = """
        fn bad() {
            // The two steps of the function
            // 1. It opens.
            // 2. It closes.

            // 1. open
            open();
            // 2. close
            close();
        }
        """
        self.assertEqual(run(src), [])

    def test_wrapped_number_equal_to_step_two_is_one_comment(self):
        src = """
        fn v() {
            // 1. It reads the rows; at most
            // 2. rows per page are kept.
            keep();
        }
        """
        self.assertEqual(run(src), [(2, "N4"), (2, "N7")])

    def test_titles_in_a_note_are_reported_once(self):
        src = """
        func f(args []any) {
        \tfor i := range args {
        \t\tif skip {
        \t\t\tcontinue
        \t\t}

        \t\t// Two syntaxes are possible in slog:
        \t\t//   1. slog.Attr{Key: "k", Value: v}
        \t\t//   2. alternating "key", value pairs
        \t\t// We must support both so callers remain ergonomic.
        \t\tswitch v := args[i].(type) {
        \t\tcase slog.Attr:
        \t\t\tuse(v)
        \t\t}
        \t}
        }
        """
        self.assertEqual(run(src, ".go"), [(8, "N7")])

    def test_years_are_prose(self):
        src = """
        fn y() {
            // 2019-2020. were the years of the old format.
            let a = 1;
            // 3.5 is the ratio.
            a
        }
        """
        self.assertEqual(run(src), [])


class WalkthroughInsideABraceBody(unittest.TestCase):
    def test_inline_method_of_a_header(self):
        src = """
        #pragma once
        #include <mutex>

        class Q {
        public:
            std::vector<int> drain() {
                // 1. It takes the lock, because pushers run on other threads.
                // 2. It swaps the buffer out, so the lock is held briefly.
                std::lock_guard<std::mutex> lock(m_);
                // 2. swap the buffer out
                std::vector<int> out;
                out.swap(buf_);
                return out;
            }
        };
        """
        self.assertEqual(run(src, ".hpp"), [(7, "N2")])

    def test_typescript_without_semicolons(self):
        src = """
        export async function load(db: Db) {
          // 1. It reads the rows, because the cache is cold.
          // 2. It fills the cache, so the next call is fast.
          await warmUp(db)
          // 2. fill the cache
          cache.fill(rows)
        }
        """
        self.assertEqual(run(src, ".ts"), [(2, "N2")])

    def test_kotlin_without_semicolons(self):
        src = """
        class R {
            fun run(db: Db) {
                // 1. It reads the rows, because the cache is cold.
                // 2. It fills the cache, so the next call is fast.
                suspend fetch(db)
                // 2. fill the cache
                cache.fill()
            }
        }
        """
        self.assertEqual(run(src, ".kt"), [(3, "N2")])

    def test_comment_on_a_member_of_a_namespace(self):
        src = """
        namespace store {
            // Reads a value:
            // 1. the header
            // 2. the body
            int read(int fd);
        }
        """
        self.assertEqual(run(src, ".hpp"), [])

    # A class header with a modifier or a template prefix, its member
    # declaration and the header of a method, for one language each.
    CLASSES = (("template <typename T> class Q {", ".hpp", "T drain();", "void take() {"),
               ("open class Q {", ".kt", "fun drain(): Int", "fun take() {"),
               ("export default class Q {", ".ts", "drain(): number", "take() {"))

    def test_comment_on_a_member_of_a_class_with_modifiers(self):
        for header, ext, member, _ in self.CLASSES:
            src = "%s\n    // Drains the queue:\n    // 1. the lock\n    // 2. the buffer\n" \
                  "    %s\n}\n" % (header, member)
            self.assertEqual(run(src, ext), [], header)

    def test_walkthrough_in_a_method_of_a_class_with_modifiers(self):
        for header, ext, _, method in self.CLASSES:
            src = "%s\n    %s\n        // 1. It takes the lock, because pushers wait.\n" \
                  "        // 2. It swaps the buffer out, so the lock is brief.\n" \
                  "        lock()\n        // 2. swap the buffer out\n        swap()\n" \
                  "    }\n}\n" % (header, method)
            self.assertEqual(run(src, ext), [(3, "N2")], header)


class NoteBelowABackReference(unittest.TestCase):
    RUST = """
    fn f() {
        // 1. Run the query, because the rows are needed.
        // 2. Keep the rows, so the caller reads them later.

        // 1. run the query
        // The first row holds the header, so it is skipped.
        // The second row may be empty.
        let rows = run(q)?;
        // 2. keep the rows
        keep(rows);
    }
    """
    PYTHON = """
    def f(q):
        # 1. Run the query, because the rows are needed.
        # 2. Keep the rows, so the caller reads them later.

        # 1. run the query
        # The first row holds the header, so it is skipped.
        rows = run(q)
        # 2. keep the rows
        keep(rows)
    """
    CLOJURE = """
    (defn f [q]
      ;; 1. Run the query, because the rows are needed.
      ;; 2. Keep the rows, so the caller reads them later.

      ;; 1. run the query
      ;; The first row holds the header, so it is skipped.
      (let [rows (run q)]
        ;; 2. keep the rows
        (keep rows)))
    """
    CASES = ((RUST, ".rs", "//"), (PYTHON, ".py", "#"), (CLOJURE, ".clj", ";;"))

    def test_note_at_the_column_of_the_number_passes(self):
        for src, ext, _ in self.CASES:
            self.assertEqual(run(src, ext), [], ext)

    def test_line_indented_further_continues_the_back_reference(self):
        for src, ext, mark in self.CASES:
            bad = src.replace(mark + " The first row", mark + "    the first row")
            self.assertEqual(rules(bad, ext), ["N7"], ext)

    def test_numbered_line_in_the_note_is_n7(self):
        for src, ext, mark in self.CASES:
            # The numbered line keeps the indentation of the note line it follows.
            bad = re.sub(r"( *)(%s The first row holds the header), so it is skipped\." % mark,
                         r"\1\2.\n\1%s 2. It is skipped." % mark, src)
            self.assertEqual(run(bad, ext), [(5, "N7")], ext)

    def test_back_reference_with_a_note_inside_the_block_of_the_step_before(self):
        src = PARSE.replace(
            "            // 3. resolve the names\n",
            "            // 3. resolve the names\n"
            "            // A name that is not known stays as it is.\n")
        self.assertEqual(run(src), [])

    def test_note_does_not_hide_a_missing_step(self):
        src = PARSE.replace(
            "            // 3. resolve the names\n",
            "            // 2. resolve the names\n"
            "            // A name that is not known stays as it is.\n")
        self.assertEqual(rules(src), ["N2", "N2"])

    def test_the_note_is_not_part_of_the_words(self):
        src = self.RUST.replace("// 1. run the query", "// 1. run the query now")
        self.assertEqual(run(src, warn_long=4), [])


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


GOOD = "fn f() {\n    // 1. One.\n    // 2. Two.\n\n    // 1. one\n    one();\n" \
       "    // 2. two\n    two();\n}\n"
BARE = "fn g() {\n    // 2.\n    x();\n}\n"
PLAN = "# Plan\n\n```rust\n" + BARE + "```\n"


class GitRepo(unittest.TestCase):
    """A throwaway repository with a base commit and a branch commit, and no tests of its own."""

    def setUp(self):
        # The base commit holds a file that the branch renames and a file that
        # it deletes, and both hold a finding that the range must not report.
        self.tmp = tempfile.TemporaryDirectory()
        repo = self.repo = self.tmp.name
        git_in(repo, "init", "-q")
        write(repo, "old.rs", BARE)
        write(repo, "gone.rs", BARE)
        write(repo, "same.rs", BARE)
        git_in(repo, "add", "-A")
        git_in(repo, "commit", "-q", "-m", "base")
        self.base = git_in(repo, "rev-parse", "HEAD")
        os.makedirs(os.path.join(repo, "src"))
        git_in(repo, "mv", "old.rs", "src/renamed.rs")
        git_in(repo, "rm", "-q", "gone.rs")
        write(repo, "src/new.rs", BARE)
        write(repo, "src/good.rs", GOOD)
        write(repo, "doc/plan.md", PLAN)
        write(repo, "notes.txt", BARE)
        git_in(repo, "add", "-A")
        git_in(repo, "commit", "-q", "-m", "branch")
        self.rev = git_in(repo, "rev-parse", "HEAD")
        self.short = git_in(repo, "rev-parse", "--short", "HEAD")
        # The working tree differs from the commit, and the range ignores it.
        write(repo, "src/good.rs", BARE)

    def tearDown(self):
        self.tmp.cleanup()

    def check(self, *argv):
        with working_directory(self.repo):
            return main_output(list(argv))


@unittest.skipUnless(shutil.which("git"), "git is not on PATH")
class GitRange(GitRepo):
    def test_changed_files_at_rev(self):
        status, lines, _ = self.check("--git-range", self.base + "..." + self.rev)
        self.assertEqual(status, 1)
        self.assertEqual(lines, [
            "src/new.rs:2: N4: step 2 is outside the scope of any walkthrough",
            "src/new.rs:2: N5: bare step number 2. needs a few words from its step",
            "src/renamed.rs:2: N4: step 2 is outside the scope of any walkthrough",
            "src/renamed.rs:2: N5: bare step number 2. needs a few words from its step",
            "check_steps: 4 findings in 3 files at " + self.short])

    def test_markdown_too(self):
        status, lines, _ = self.check("--git-range", self.base + "..." + self.rev,
                                      "--markdown-too")
        self.assertEqual(status, 1)
        self.assertIn("doc/plan.md:5: N5: bare step number 2. needs a few words from its step",
                      lines)
        self.assertEqual(lines[-1], "check_steps: 6 findings in 4 files at " + self.short)

    def test_empty_range(self):
        status, lines, _ = self.check("--git-range", self.rev + "..." + self.rev)
        self.assertEqual((status, lines), (0, ["check_steps: 0 findings in 0 files at "
                                               + self.short]))

    def test_malformed_range(self):
        for bad in ("HEAD", "HEAD..HEAD", "HEAD....HEAD", "...HEAD", "--output=x...HEAD"):
            status, lines, err = self.check("--git-range=" + bad)
            self.assertEqual((status, lines), (2, []), bad)
            self.assertEqual(len(err.splitlines()), 1, bad)

    def test_unknown_revision(self):
        status, lines, err = self.check("--git-range", self.base + "...nosuchrev")
        self.assertEqual((status, lines), (2, []))
        self.assertTrue(err.startswith("check_steps: git rev-parse failed: "), err)

    def test_paths_are_refused(self):
        status, _, _ = self.check("--git-range", self.base + "..." + self.rev, ".")
        self.assertEqual(status, 2)


class Words(unittest.TestCase):
    def test_words_without_a_range_or_a_path(self):
        status, lines, err = main_output(["--words", "focus:", "placement", "also:"])
        self.assertEqual((status, lines), (2, []))
        self.assertEqual(err, "check_steps: no range and no path in the words\n")

    def test_no_words_at_all(self):
        status, _, err = main_output(["--words"])
        self.assertEqual((status, err), (2, "check_steps: no range and no path in the words\n"))

    def test_paths_and_markdown_among_other_words(self):
        with tempfile.TemporaryDirectory() as d:
            write(d, "src/bare.rs", BARE)
            write(d, "plan.md", PLAN)
            with working_directory(d):
                status, lines, _ = main_output(["--words", "src", "plan.md", "numbering",
                                                "-x", "focus:", "more"])
        self.assertEqual(status, 1)
        self.assertEqual(lines, [
            "plan.md:5: N4: step 2 is outside the scope of any walkthrough",
            "plan.md:5: N5: bare step number 2. needs a few words from its step",
            "src/bare.rs:2: N4: step 2 is outside the scope of any walkthrough",
            "src/bare.rs:2: N5: bare step number 2. needs a few words from its step",
            "check_steps: 4 findings in 2 files"])

    def test_options_before_the_words_apply(self):
        with tempfile.TemporaryDirectory() as d:
            write(d, "good.rs", GOOD)
            with working_directory(d):
                status, lines, _ = main_output(["--warn-long", "1", "--deny", "One",
                                                "--words", "good.rs"])
        self.assertEqual(status, 1)
        self.assertEqual(lines, ["good.rs:2: D1: comment matches 'One'",
                                 "check_steps: 1 finding, 0 warnings in 1 file"])

    def test_options_after_words_are_words(self):
        with tempfile.TemporaryDirectory() as d:
            write(d, "good.rs", GOOD)
            with working_directory(d):
                status, lines, _ = main_output(["--words", "good.rs", "--warn"])
        self.assertEqual((status, lines), (0, ["check_steps: 0 findings in 1 file"]))

    def test_words_refuse_paths_ranges_and_markdown(self):
        with tempfile.TemporaryDirectory() as d:
            write(d, "good.rs", GOOD)
            write(d, "plan.md", PLAN)
            with working_directory(d):
                for argv in (["good.rs", "--words", "good.rs"],
                             ["--git-range", "a...b", "--words", "good.rs"],
                             ["--markdown", "plan.md", "--words", "good.rs"]):
                    self.assertEqual(main_output(argv)[0], 2, argv)

    def test_split_words(self):
        self.assertEqual(cs.split_words(["--warn", "--words", "a", "--deny", "b"]),
                         (["--warn"], ["a", "--deny", "b"]))
        self.assertEqual(cs.split_words(["--words=a", "b"]), ([], ["a", "b"]))
        self.assertEqual(cs.split_words(["x"]), (["x"], None))

    def test_classify_words(self):
        with tempfile.TemporaryDirectory() as d:
            write(d, "a.rs", GOOD)
            write(d, "b.md", PLAN)
            with working_directory(d):
                got = cs.classify_words(["base...HEAD", "a.rs", "b.md", ".", "x..y",
                                         "a....b", "-a...b", "a...-b", "...b", "focus:"])
        self.assertEqual(got, (["base...HEAD"], ["a.rs", "."], ["b.md"]))

    def test_scope_ends_at_the_first_marker(self):
        self.assertEqual(cs.scope_words(["a", "b", "focus:", "c", "plan:"]), ["a", "b"])
        with tempfile.TemporaryDirectory() as d:
            write(d, "good.rs", GOOD)
            write(d, "src/bare.rs", BARE)
            with working_directory(d):
                status, lines, _ = main_output(["--words", "good.rs", "focus:", "src"])
        self.assertEqual((status, lines), (0, ["check_steps: 0 findings in 1 file"]))

    def test_file_with_three_dots_in_its_name_is_a_path(self):
        with tempfile.TemporaryDirectory() as d:
            write(d, "v1...v2.hs", "f x = x\n")
            with working_directory(d):
                status, lines, err = main_output(["--words", "v1...v2.hs"])
        self.assertEqual((status, lines, err), (0, ["check_steps: 0 findings in 1 file"], ""))

    def test_two_dots_is_an_ignored_word(self):
        with tempfile.TemporaryDirectory() as d:
            write(d, "good.rs", GOOD)
            with working_directory(d):
                with_path = main_output(["--words", "base..HEAD", "good.rs"])
                alone = main_output(["--words", "base..HEAD"])
        self.assertEqual(with_path, (0, ["check_steps: 0 findings in 1 file"], ""))
        self.assertEqual(alone, (2, [], "check_steps: no range and no path in the words\n"))

    def test_abbreviated_option_is_a_usage_error(self):
        status, lines, err = main_output(["--wor", "x"])
        self.assertEqual((status, lines), (2, []))
        self.assertIn("unrecognized arguments: --wor", err)


@unittest.skipUnless(shutil.which("git"), "git is not on PATH")
class WordsWithARange(GitRepo):
    def test_range_and_paths_are_merged_and_sorted(self):
        write(self.repo, "extra/bare.rs", BARE)
        status, lines, _ = self.check("--words", self.base + "..." + self.rev, "numbering",
                                      "extra", "focus:", "src")
        self.assertEqual(status, 1)
        self.assertEqual(lines, [
            "doc/plan.md:5: N4: step 2 is outside the scope of any walkthrough",
            "doc/plan.md:5: N5: bare step number 2. needs a few words from its step",
            "extra/bare.rs:2: N4: step 2 is outside the scope of any walkthrough",
            "extra/bare.rs:2: N5: bare step number 2. needs a few words from its step",
            "src/new.rs:2: N4: step 2 is outside the scope of any walkthrough",
            "src/new.rs:2: N5: bare step number 2. needs a few words from its step",
            "src/renamed.rs:2: N4: step 2 is outside the scope of any walkthrough",
            "src/renamed.rs:2: N5: bare step number 2. needs a few words from its step",
            "check_steps: 8 findings in 5 files at " + self.short])

    def test_two_ranges_are_refused(self):
        r = self.base + "..." + self.rev
        status, lines, err = self.check("--words", r, r)
        self.assertEqual((status, lines), (2, []))
        self.assertTrue(err.startswith("check_steps: more than one range"), err)

    def test_unknown_range_beside_a_path_is_ignored(self):
        word = self.base + "...nosuchrev"
        status, lines, err = self.check("--words", word, "src")
        self.assertEqual(status, 1)
        self.assertEqual(lines[-1], "check_steps: 6 findings in 3 files")
        self.assertEqual(err, "check_steps: ignored '%s': git does not know this range\n" % word)

    def test_unknown_range_alone_is_no_range(self):
        status, lines, err = self.check("--words", "hmm...maybe")
        self.assertEqual((status, lines), (2, []))
        self.assertEqual(err, "check_steps: ignored 'hmm...maybe': git does not know this "
                              "range\ncheck_steps: no range and no path in the words\n")


if __name__ == "__main__":
    unittest.main()
