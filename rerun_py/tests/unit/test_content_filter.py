"""Unit tests for the ContentFilter helper."""

from __future__ import annotations

import pytest
from rerun.catalog._content_filter import ContentFilter

# -- ContentFilter builder -----------------------------------------------------


class TestContentFilterEverything:
    def test_exprs(self) -> None:
        assert ContentFilter.everything().to_exprs() == ["/**"]

    def test_repr(self) -> None:
        assert repr(ContentFilter.everything()) == "ContentFilter(['/**'])"


class TestContentFilterNothing:
    def test_exprs(self) -> None:
        assert ContentFilter.nothing().to_exprs() == []

    def test_repr(self) -> None:
        assert repr(ContentFilter.nothing()) == "ContentFilter([])"


class TestContentFilterInclude:
    def test_string_exact(self) -> None:
        f = ContentFilter.nothing().include("/robot/arm")
        assert f.to_exprs() == ["/robot/arm"]

    def test_string_subtree_explicit(self) -> None:
        # User already wrote /**
        f = ContentFilter.nothing().include("/robot/arm/**")
        assert f.to_exprs() == ["/robot/arm/**"]

    def test_string_subtree_param(self) -> None:
        f = ContentFilter.nothing().include("/robot/arm", subtree=True)
        assert f.to_exprs() == ["/robot/arm/**"]

    def test_string_subtree_param_idempotent(self) -> None:
        # subtree=True on a path that already ends with /** should not double-append
        f = ContentFilter.nothing().include("/robot/arm/**", subtree=True)
        assert f.to_exprs() == ["/robot/arm/**"]

    def test_rejects_path_without_leading_slash(self) -> None:
        with pytest.raises(ValueError, match="must start with '/'"):
            ContentFilter.nothing().include("robot/arm")


class TestContentFilterExclude:
    def test_string_exact(self) -> None:
        f = ContentFilter.everything().exclude("/robot/raw")
        assert f.to_exprs() == ["/**", "-/robot/raw"]

    def test_string_subtree_explicit(self) -> None:
        f = ContentFilter.everything().exclude("/robot/raw/**")
        assert f.to_exprs() == ["/**", "-/robot/raw/**"]

    def test_string_subtree_param(self) -> None:
        f = ContentFilter.everything().exclude("/robot/raw", subtree=True)
        assert f.to_exprs() == ["/**", "-/robot/raw/**"]

    def test_string_subtree_param_idempotent(self) -> None:
        f = ContentFilter.everything().exclude("/robot/raw/**", subtree=True)
        assert f.to_exprs() == ["/**", "-/robot/raw/**"]

    def test_rejects_path_without_leading_slash(self) -> None:
        with pytest.raises(ValueError, match="must start with '/'"):
            ContentFilter.everything().exclude("robot/raw")


class TestContentFilterIncludeProperties:
    def test_adds_properties_rule(self) -> None:
        f = ContentFilter.everything().include_properties()
        assert f.to_exprs() == ["/**", "/__properties/**"]

    def test_nothing_plus_properties(self) -> None:
        f = ContentFilter.nothing().include_properties()
        assert f.to_exprs() == ["/__properties/**"]


class TestContentFilterChaining:
    def test_full_example(self) -> None:
        """Matches the motivating example from the feature request."""
        f = ContentFilter.everything().exclude("/robot/raw/**").include("/robot/raw/i_need_this").include_properties()
        assert f.to_exprs() == [
            "/**",
            "-/robot/raw/**",
            "/robot/raw/i_need_this",
            "/__properties/**",
        ]

    def test_nothing_then_allowlist(self) -> None:
        f = ContentFilter.nothing().include("/a", subtree=True).include("/b/**")
        assert f.to_exprs() == ["/a/**", "/b/**"]

    def test_multiple_excludes(self) -> None:
        f = ContentFilter.everything().exclude("/debug/**").exclude("/tmp/**")
        assert f.to_exprs() == ["/**", "-/debug/**", "-/tmp/**"]

    def test_to_exprs_returns_copy(self) -> None:
        """Mutating the returned list must not affect the filter."""
        f = ContentFilter.everything()
        exprs = f.to_exprs()
        exprs.append("injected")
        assert f.to_exprs() == ["/**"]

    def test_chaining_returns_new_instance(self) -> None:
        """Builder methods return a new ContentFilter (immutable)."""
        f = ContentFilter.everything()
        f2 = f.exclude("/x")
        f3 = f2.include("/y")
        f4 = f3.include_properties()
        assert f is not f2
        assert f2 is not f3
        assert f3 is not f4

    def test_branching_from_shared_base(self) -> None:
        """Branching from a shared base must not mutate the base."""
        base = ContentFilter.everything().exclude("/debug/**")
        a = base.include("/a")
        b = base.include("/b")
        assert base.to_exprs() == ["/**", "-/debug/**"]
        assert a.to_exprs() == ["/**", "-/debug/**", "/a"]
        assert b.to_exprs() == ["/**", "-/debug/**", "/b"]


class TestContentFilterEquality:
    def test_equal(self) -> None:
        assert ContentFilter.everything() == ContentFilter.everything()

    def test_not_equal(self) -> None:
        assert ContentFilter.everything() != ContentFilter.nothing()

    def test_not_equal_to_non_filter(self) -> None:
        assert ContentFilter.everything() != "/**"

    def test_hash_equal(self) -> None:
        assert hash(ContentFilter.everything()) == hash(ContentFilter.everything())

    def test_hash_usable_in_set(self) -> None:
        s = {ContentFilter.everything(), ContentFilter.everything(), ContentFilter.nothing()}
        assert len(s) == 2
