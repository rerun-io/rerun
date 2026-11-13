from __future__ import annotations

import tempfile
import textwrap
import unittest
from pathlib import Path

from check_test_targets import cargo_metadata, check  # type: ignore[import-not-found]


class Workspace:
    def __init__(self, root: Path) -> None:
        self.root = root
        self.members: list[str] = []

    def crate(self, name: str, files: dict[str, str], manifest: str = "") -> None:
        crate_dir = self.root / name
        self.members.append(name)
        (crate_dir / "src").mkdir(parents=True)
        (crate_dir / "Cargo.toml").write_text(
            f'[package]\nname = "{name}"\nversion = "0.1.0"\nedition = "2021"\n\n' + textwrap.dedent(manifest)
        )
        for path, content in files.items():
            (crate_dir / path).parent.mkdir(parents=True, exist_ok=True)
            (crate_dir / path).write_text(textwrap.dedent(content))

    def errors(self, untracked: set[str] | None = None) -> list[str]:
        members = ", ".join(f'"{member}"' for member in self.members)
        (self.root / "Cargo.toml").write_text(f'[workspace]\nresolver = "2"\nmembers = [{members}]\n')
        tracked = None
        if untracked is not None:
            skip = {(self.root / path).resolve() for path in untracked}
            tracked = {path.resolve() for path in self.root.rglob("*") if path.is_file()} - skip
        errors: list[str]
        errors, _ = check(cargo_metadata(self.root), tracked)
        return errors


class CheckTestTargetsTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.workspace = Workspace(Path(self.tmp.name))

    def errors_for(self, files: dict[str, str], manifest: str = "", name: str = "fixture") -> list[str]:
        self.workspace.crate(name, files, manifest)
        return self.workspace.errors()

    def assert_no_tests(self, files: dict[str, str], manifest: str = "") -> None:
        errors = self.errors_for(files, manifest)
        self.assertEqual(len(errors), 1, errors)
        self.assertIn("contains no tests", errors[0])

    def assert_has_tests(self, files: dict[str, str], manifest: str = "") -> None:
        self.assertEqual(self.errors_for(files, manifest), [])

    def test_lib_without_tests_and_test_true_fails(self) -> None:
        errors = self.errors_for({"src/lib.rs": "pub fn f() {}\n"})
        self.assertEqual(
            errors,
            ["fixture/Cargo.toml: lib target `fixture` contains no tests. Set `test = false` in its `[lib]` section."],
        )

    def test_bin_without_tests_and_test_true_fails(self) -> None:
        errors = self.errors_for({"src/main.rs": "fn main() {}\n"})
        self.assertEqual(
            errors,
            [
                (
                    "fixture/Cargo.toml: bin target `fixture` contains no tests. "
                    "Set `test = false` in its `[[bin]]` section."
                )
            ],
        )

    def test_proc_macro_without_tests_fails(self) -> None:
        errors = self.errors_for({"src/lib.rs": "pub fn f() {}\n"}, "[lib]\nproc-macro = true\n")
        self.assertEqual(len(errors), 1, errors)
        self.assertIn("lib target `fixture` contains no tests", errors[0])

    def test_untracked_module_file_counts_as_unknown(self) -> None:
        files = {"src/lib.rs": "mod generated;\n", "src/generated.rs": "pub fn f() {}\n"}
        self.workspace.crate("fixture", files)
        self.assertEqual(self.workspace.errors(untracked={"fixture/src/generated.rs"}), [])

    def test_untracked_module_file_with_test_false_fails(self) -> None:
        files = {"src/lib.rs": "mod generated;\n", "src/generated.rs": "pub fn f() {}\n"}
        self.workspace.crate("fixture", files, "[lib]\ntest = false\n")
        errors = self.workspace.errors(untracked={"fixture/src/generated.rs"})
        self.assertEqual(len(errors), 1, errors)
        self.assertIn("cannot find the file for `mod generated;`", errors[0])

    def test_tracked_module_file_is_followed(self) -> None:
        files = {"src/lib.rs": "mod generated;\n", "src/generated.rs": "pub fn f() {}\n"}
        self.workspace.crate("fixture", files)
        errors = self.workspace.errors(untracked=set())
        self.assertEqual(len(errors), 1, errors)
        self.assertIn("contains no tests", errors[0])

    def test_lib_without_tests_and_test_false_passes(self) -> None:
        self.assertEqual(self.errors_for({"src/lib.rs": "pub fn f() {}\n"}, "[lib]\ntest = false\n"), [])

    def test_bin_with_tests_and_test_false_fails(self) -> None:
        errors = self.errors_for(
            {"src/main.rs": "fn main() {}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {}\n}\n"},
            '[[bin]]\nname = "fixture"\ntest = false\n',
        )
        self.assertEqual(
            errors,
            [
                (
                    "fixture/Cargo.toml: bin target `fixture` has `test = false`, so its tests never run "
                    "(fixture/src/main.rs:5: `#[test]`). Remove `test = false` from its `[[bin]]` section."
                )
            ],
        )

    def test_lib_with_tests_and_test_false_fails(self) -> None:
        errors = self.errors_for({"src/lib.rs": "#[tokio::test]\nasync fn t() {}\n"}, "[lib]\ntest = false\n")
        self.assertEqual(len(errors), 1, errors)
        self.assertIn("lib target `fixture` has `test = false`", errors[0])
        self.assertIn("`#[tokio::test]`", errors[0])
        self.assertIn("`[lib]` section", errors[0])

    def test_test_markers(self) -> None:
        markers = [
            "#[test]",
            "#[::core::prelude::v1::test]",
            "#[tokio::test]",
            '#[tokio::test(flavor = "multi_thread", worker_threads = 2)]',
            "#[wasm_bindgen_test]",
            "#[wasm_bindgen_test::wasm_bindgen_test]",
            "#[test_log::test]",
            "#[test_case(1)]",
            "#[rstest]",
            '#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]',
            '#[cfg_attr(not(target_arch = "wasm32"), tokio::test, ignore)]',
        ]
        for i, marker in enumerate(markers):
            with self.subTest(marker=marker):
                self.workspace.crate(f"marker{i}", {"src/lib.rs": f"{marker}\nfn t() {{}}\n"})
        self.assertEqual(self.workspace.errors(), [])

    def test_macro_rules_generating_tests_invoked_from_other_crate(self) -> None:
        self.workspace.crate(
            "generator",
            {
                "src/lib.rs": """\
                    #[macro_export]
                    macro_rules! outer {
                        ($($name:ident),*) => {
                            #[macro_export]
                            macro_rules! generate_tests {
                                () => { $( #[tokio::test] async fn $name() {} )* };
                            }
                        };
                    }
                    outer!(a, b);

                    #[macro_export]
                    macro_rules! wrapper {
                        () => { $crate::generate_tests!(); };
                    }
                """
            },
        )
        self.workspace.crate("direct", {"src/lib.rs": "generator::generate_tests!();\n"})
        self.workspace.crate("indirect", {"src/lib.rs": "generator::wrapper! {}\n"})
        self.workspace.crate(
            "unrelated",
            {"src/lib.rs": "macro_rules! m { () => { #[inline] fn f() {} }; }\nm!();\n"},
        )
        errors = self.workspace.errors()
        self.assertEqual(len(errors), 1, errors)
        self.assertIn("unrelated/Cargo.toml: lib target `unrelated` contains no tests", errors[0])

    def test_generated_tests_with_test_false_fails(self) -> None:
        self.workspace.crate(
            "generator",
            {"src/lib.rs": "#[macro_export]\nmacro_rules! gen { () => { #[test] fn t() {} }; }\n"},
        )
        self.workspace.crate("user", {"src/lib.rs": "generator::gen!();\n"}, "[lib]\ntest = false\n")
        errors = self.workspace.errors()
        self.assertEqual(len(errors), 1, errors)
        self.assertIn("user/src/lib.rs:1: `gen!` generates tests", errors[0])

    def test_tests_only_in_nested_submodule_files(self) -> None:
        self.assert_has_tests({
            "src/lib.rs": "mod a;\n",
            "src/a.rs": "mod b;\n",
            "src/a/b/mod.rs": "mod c;\n",
            "src/a/b/c.rs": "#[test]\nfn t() {}\n",
        })

    def test_tests_only_behind_cfg(self) -> None:
        cfgs = [
            '#[cfg(target_os = "windows")]',
            '#[cfg(feature = "nope")]',
            '#[cfg(target_arch = "wasm32")]',
            "#[cfg(any())]",
        ]
        for i, cfg in enumerate(cfgs):
            with self.subTest(cfg=cfg):
                self.workspace.crate(
                    f"cfg{i}",
                    {"src/lib.rs": f"{cfg}\nmod platform;\n", "src/platform.rs": "#[test]\nfn t() {}\n"},
                )
        self.assertEqual(self.workspace.errors(), [])

    def test_path_attributes(self) -> None:
        self.assert_has_tests({
            "src/lib.rs": '#[path = "elsewhere/impl.rs"]\nmod imp;\n',
            "src/elsewhere/impl.rs": "mod child;\n",
            "src/elsewhere/child.rs": "#[test]\nfn t() {}\n",
        })

    def test_path_attribute_inside_inline_module(self) -> None:
        self.assert_has_tests({
            "src/lib.rs": 'mod v1 {\n    #[path = "./gen.rs"]\n    mod generated;\n}\n',
            "src/v1/gen.rs": "#[test]\nfn t() {}\n",
        })

    def test_cfg_attr_path(self) -> None:
        self.assert_has_tests({
            "src/lib.rs": textwrap.dedent("""\
                    #[cfg_attr(target_arch = "wasm32", path = "wrapper_wasm.rs")]
                    #[cfg_attr(not(target_arch = "wasm32"), path = "wrapper_native.rs")]
                    mod wrapper;
                """),
            "src/wrapper_native.rs": "pub fn f() {}\n",
            "src/wrapper_wasm.rs": "#[wasm_bindgen_test]\nfn t() {}\n",
        })

    def test_inline_module_nests_file_modules(self) -> None:
        self.assert_has_tests({
            "src/lib.rs": "pub mod outer {\n    pub(crate) mod inner;\n}\n",
            "src/outer/inner.rs": "#[test]\nfn t() {}\n",
        })

    def test_non_mod_rs_file_owns_subdirectory(self) -> None:
        self.assert_has_tests({
            "src/main.rs": "mod a;\nfn main() {}\n",
            "src/a.rs": "mod inline {\n    mod deep;\n}\n",
            "src/a/inline/deep.rs": "#[test]\nfn t() {}\n",
        })

    def test_include(self) -> None:
        self.assert_has_tests({
            "src/lib.rs": 'mod generated {\n    include!("generated_tests.rs");\n}\n',
            "src/generated_tests.rs": "#[test]\nfn t() {}\n",
        })

    def test_unresolvable_include_counts_as_tests(self) -> None:
        files = {"src/lib.rs": 'include!(concat!(env!("OUT_DIR"), "/gen.rs"));\n'}
        self.assert_has_tests(files)

    def test_unresolvable_include_with_test_false_fails(self) -> None:
        errors = self.errors_for(
            {"src/lib.rs": 'include!(concat!(env!("OUT_DIR"), "/gen.rs"));\n'}, "[lib]\ntest = false\n"
        )
        self.assertEqual(len(errors), 1, errors)
        self.assertIn("possibly: fixture/src/lib.rs:1: cannot resolve `include!`", errors[0])

    def test_missing_module_file_counts_as_tests(self) -> None:
        self.assert_has_tests({"src/lib.rs": "mod generated_by_build_rs;\n"})

    def test_lib_and_bin_in_one_package_only_lib_has_tests(self) -> None:
        errors = self.errors_for({
            "src/lib.rs": "mod util;\npub fn f() {}\n",
            "src/util.rs": "#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {}\n}\n",
            "src/main.rs": "fn main() {\n    fixture::f();\n}\n",
            "src/bin/tool.rs": "fn main() {}\n",
        })
        self.assertEqual(
            sorted(errors),
            [
                "fixture/Cargo.toml: bin target `fixture` contains no tests. Set `test = false` in its `[[bin]]` section.",
                "fixture/Cargo.toml: bin target `tool` contains no tests. Set `test = false` in its `[[bin]]` section.",
            ],
        )

    def test_bin_sharing_module_with_lib(self) -> None:
        self.assert_has_tests({
            "src/lib.rs": "mod shared;\n",
            "src/main.rs": "mod shared;\nfn main() {}\n",
            "src/shared.rs": "#[test]\nfn t() {}\n",
        })

    def test_markers_in_comments_and_strings_do_not_count(self) -> None:
        self.assert_no_tests({
            "src/lib.rs": textwrap.dedent("""\
                    //! ```
                    //! #[test]
                    //! fn t() {}
                    //! ```
                    // #[test]
                    /* #[test] /* nested */ #[test] */
                    pub const A: &str = "#[test]";
                    pub const B: &str = r#"#[tokio::test] "quoted""#;
                    pub const C: &[u8] = br"#[test]";
                    pub const D: char = '#';
                    pub fn e<'a>(x: &'a str) -> &'a str { x }
                    #[cfg(test)]
                    pub fn only_in_tests() {}
                    #[cfg_attr(not(test), expect(dead_code))]
                    fn helper() {}
                    #[latest]
                    pub fn g() {}
                    pub fn test() {}
                """)
        })


if __name__ == "__main__":
    unittest.main()
