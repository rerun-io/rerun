from __future__ import annotations

import pyarrow as pa
import pyarrow.dataset as ds
from datafusion import SessionContext, udtf
from inline_snapshot import snapshot as inline_snapshot

# TODO: remove this file


def test_simple_udtf() -> None:
    """
    Test a simple user-defined table function (UDTF) that generates a sequence of numbers.

    This test demonstrates how to create and use a UDTF with DataFusion.
    UDTFs are table-valued functions that can generate data dynamically.
    """

    # Create a UDTF that generates a range of numbers
    @udtf(name="generate_range")
    def generate_range(n):
        """Generate a table with numbers from 0 to n-1."""
        # Extract the Python value from the datafusion expression
        count = n.python_value().as_py()

        # Create a PyArrow table with a single column
        table = pa.table({"value": list(range(count))})

        # Convert to a dataset (required by DataFusion UDTFs)
        dataset = ds.InMemoryDataset(table)
        return dataset

    # Create a standalone DataFusion session context
    ctx = SessionContext()

    # Register the UDTF
    ctx.register_udtf(generate_range)

    # Use the UDTF in a SQL query
    result_df = ctx.sql("SELECT * FROM generate_range(5)")

    # Verify the schema
    assert str(result_df.schema()) == inline_snapshot("""\
value: int64\
""")

    # Verify the data
    result_table = result_df.to_arrow_table()
    assert len(result_table) == 5
    assert result_table["value"].to_pylist() == [0, 1, 2, 3, 4]


def test_udtf_with_transformation() -> None:
    """
    Test a UDTF that generates data and then applies transformations.

    This demonstrates using a UDTF in combination with SQL operations
    like filtering, transformations, and ordering.
    """

    # Create a UDTF that generates key-value pairs
    @udtf(name="generate_pairs")
    def generate_pairs(n):
        """Generate a table with id and squared value pairs."""
        count = n.python_value().as_py()

        # Create a table with two columns
        table = pa.table({"id": list(range(count)), "squared": [i * i for i in range(count)]})

        dataset = ds.InMemoryDataset(table)
        return dataset

    # Create a standalone DataFusion session context
    ctx = SessionContext()
    ctx.register_udtf(generate_pairs)

    # Use the UDTF with filtering and additional computation
    result_df = ctx.sql(
        """
        SELECT
            id,
            squared,
            squared * 2 as doubled_square
        FROM generate_pairs(4)
        WHERE id > 0
        ORDER BY id
        """
    )

    # Verify the data
    result_table = result_df.to_arrow_table()
    assert len(result_table) == 3
    assert result_table["id"].to_pylist() == [1, 2, 3]
    assert result_table["squared"].to_pylist() == [1, 4, 9]
    assert result_table["doubled_square"].to_pylist() == [2, 8, 18]


def test_udtf_with_multiple_arguments() -> None:
    """
    Test a UDTF that accepts multiple arguments.

    This shows how to create a more complex UDTF with multiple parameters
    and demonstrates generating a multiplication table.
    """

    # Create a UDTF that generates a multiplication table
    @udtf(name="multiplication_table")
    def multiplication_table(start, end):
        """Generate multiplication table for numbers from start to end-1."""
        start_val = start.python_value().as_py()
        end_val = end.python_value().as_py()

        # Generate multiplication table
        rows = []
        for i in range(start_val, end_val):
            for j in range(start_val, end_val):
                rows.append({"x": i, "y": j, "product": i * j})

        table = pa.table({
            "x": [r["x"] for r in rows],
            "y": [r["y"] for r in rows],
            "product": [r["product"] for r in rows],
        })

        dataset = ds.InMemoryDataset(table)
        return dataset

    # Create a standalone DataFusion session context
    ctx = SessionContext()
    ctx.register_udtf(multiplication_table)

    # Use the UDTF with multiple arguments
    result_df = ctx.sql("SELECT * FROM multiplication_table(2, 4) WHERE product > 4 ORDER BY x, y")

    # Verify the data
    result_table = result_df.to_arrow_table()
    expected_results = [
        (2, 3, 6),
        (3, 2, 6),
        (3, 3, 9),
    ]

    assert len(result_table) == len(expected_results)
    for i, (exp_x, exp_y, exp_prod) in enumerate(expected_results):
        assert result_table["x"][i].as_py() == exp_x
        assert result_table["y"][i].as_py() == exp_y
        assert result_table["product"][i].as_py() == exp_prod
