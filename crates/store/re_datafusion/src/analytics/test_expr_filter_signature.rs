use datafusion::logical_expr::expr::InList;
use datafusion::logical_expr::{Between, col, lit};

use super::*;

// Literals are scrubbed to `?` placeholders (see `scrub_expr_literals`), so
// these signatures are templates — column names and operators survive, the
// constant values never appear.

#[test]
fn binary_expr_column_on_left() {
    let expr = col("frame_nr").gt(lit(100i64));
    assert_eq!(expr_filter_signature(&expr), "(frame_nr > ?)");
}

#[test]
fn binary_expr_column_on_right() {
    let expr = lit(100i64).lt(col("frame_nr"));
    assert_eq!(expr_filter_signature(&expr), "(? < frame_nr)");
}

#[test]
fn binary_expr_equality() {
    let expr = col("rerun_segment_id").eq(lit("some-segment"));
    assert_eq!(expr_filter_signature(&expr), "(rerun_segment_id = ?)");
}

#[test]
fn between_expr() {
    let expr = Expr::Between(Between {
        expr: Box::new(col("log_time")),
        negated: false,
        low: Box::new(lit(0i64)),
        high: Box::new(lit(1000i64)),
    });
    assert_eq!(expr_filter_signature(&expr), "(log_time BETWEEN ? AND ?)");
}

#[test]
fn in_list_expr() {
    let expr = Expr::InList(InList {
        expr: Box::new(col("rerun_segment_id")),
        list: vec![lit("a"), lit("b")],
        negated: false,
    });
    assert_eq!(expr_filter_signature(&expr), "rerun_segment_id IN (?, ?)");
}

#[test]
fn literals_are_scrubbed_regardless_of_value() {
    // Two filters differing only in their constants must produce the
    // identical template — the whole point of scrubbing.
    let a = col("frame_nr").gt(lit(100i64));
    let b = col("frame_nr").gt(lit(999_999i64));
    assert_eq!(expr_filter_signature(&a), expr_filter_signature(&b));
    assert_eq!(expr_filter_signature(&a), "(frame_nr > ?)");
}

#[test]
fn string_literal_value_never_leaks() {
    let expr = col("entity_path").eq(lit("customer-secret-value"));
    let sig = expr_filter_signature(&expr);
    assert!(
        !sig.contains("customer-secret-value"),
        "leaked literal: {sig}"
    );
    assert_eq!(sig, "(entity_path = ?)");
}

#[test]
fn alias_is_transparent() {
    let expr = col("frame_nr").gt(lit(5i64)).alias("my_filter");
    assert_eq!(expr_filter_signature(&expr), "(frame_nr > ?)");
}

#[test]
fn plain_column_reference() {
    let expr = col("something");
    assert_eq!(expr_filter_signature(&expr), "something");
}

#[test]
fn semicolon_in_column_name_is_escaped() {
    // SQL quotes the identifier as "my;col"; the `;` is then escaped for join safety.
    let expr = col("my;col").gt(lit(0i64));
    assert_eq!(expr_filter_signature(&expr), "(\"my\\;col\" > ?)");
}

#[test]
fn backslash_in_column_name_is_escaped() {
    // SQL quotes the identifier as "my\col"; the `\` is then escaped.
    let expr = col("my\\col").gt(lit(0i64));
    assert_eq!(expr_filter_signature(&expr), "(\"my\\\\col\" > ?)");
}
