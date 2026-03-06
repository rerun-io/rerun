#!/usr/bin/env python3
"""
Market Data Comparison Demo
Shows features for "Any Scalar" visualization:
- Normalizing multiple real-time tickers (NVDA, AAPL, MSFT, GOOGL, AMD, INTC).
- Comparing relative performance using Blueprint Selectors.
"""

from __future__ import annotations

import rerun as rr
import rerun.blueprint as rrb
from rerun.blueprint.datatypes import ComponentSourceKind, VisualizerComponentMapping
import yfinance as yf


def log_market_data(tickers: list[str]) -> None:
    """Fetch, normalize, and log real market data for multiple tickers."""
    print(f"Fetching market data for {tickers}…")
    # Fetch 7 days (max for 1m interval) to ensure we have data even on weekends/holidays
    df = yf.download(tickers, period="7d", interval="1m", progress=False)

    if df.empty:
        print(f"Warning: No data found for {tickers}.")
        return

    # Find the most recent date that has at least 100 data points
    # This ensures we do not show a nearly empty chart if the market just opened.
    daily_counts = df.groupby(df.index.date).size()
    valid_dates = daily_counts[daily_counts >= 100].index

    if len(valid_dates) == 0:
        print("Warning: No day found with at least 100 data points. Using the most recent day with any data.")
        last_date = df.index.max().date()
    else:
        last_date = max(valid_dates)

    df = df[df.index.date == last_date]

    for ticker in tickers:
        try:
            if len(tickers) > 1:
                ticker_data = df.xs(ticker, axis=1, level=1).dropna()
            else:
                ticker_data = df.dropna()

            if ticker_data.empty:
                continue

            timestamps = ticker_data.index
            # Normalization: Scale relative to the first Close price (%)
            baseline = float(ticker_data.iloc[0]["Close"])

            # Nested Any Scalar: prices.close, prices.normalized, details.volume
            market_ticks = [
                [
                    {
                        "prices": {
                            "close": float(row["Close"]),
                            "normalized": (float(row["Close"]) / baseline - 1.0) * 100.0,
                        },
                        "details": {"volume": float(row["Volume"])},
                    }
                ]
                for _, row in ticker_data.iterrows()
            ]

            rr.send_columns(
                f"market/{ticker}",
                indexes=[rr.TimeColumn("market_time", timestamp=timestamps)],
                columns=[*rr.DynamicArchetype.columns(archetype="MarketTelemetry", components={"data": market_ticks})],
            )
        except Exception as e:
            print(f"Error processing {ticker}: {e}")


def run_market_demo() -> None:
    """Run the market data demo and log data."""
    tickers = ["NVDA", "AAPL", "MSFT", "GOOGL", "AMD", "INTC"]
    log_market_data(tickers)


def generate_blueprint() -> rrb.Blueprint:
    """Generate the blueprint for the market demo."""
    tickers = ["NVDA", "AAPL", "MSFT", "GOOGL", "AMD", "INTC"]
    colors = {
        "NVDA": [118, 185, 0],  # NVIDIA Green
        "AAPL": [85, 85, 85],  # Apple Gray
        "MSFT": [0, 164, 239],  # Microsoft Blue
        "GOOGL": [219, 68, 55],  # Google Red
        "AMD": [237, 28, 36],  # AMD Red
        "INTC": [0, 104, 181],  # Intel Blue
    }
    return rrb.Blueprint(
        rrb.Vertical(
            rrb.Horizontal(
                rrb.TimeSeriesView(
                    name="Market Relative Performance (%)",
                    origin="/market",
                    overrides={
                        f"market/{ticker}": [
                            rr.SeriesLines(names=f"{ticker} (Rel)", colors=colors.get(ticker)).visualizer(
                                mappings=[
                                    VisualizerComponentMapping(
                                        target="Scalars:scalars",
                                        source_kind=ComponentSourceKind.SourceComponent,
                                        source_component="MarketTelemetry:data",
                                        selector=".prices.normalized",
                                    )
                                ]
                            )
                        ]
                        for ticker in tickers
                    },
                    axis_x=rrb.TimeAxis(
                        view_range=rrb.TimeRange(
                            start=rrb.TimeRangeBoundary.infinite(), end=rrb.TimeRangeBoundary.infinite()
                        )
                    ),
                ),
                rrb.TimeSeriesView(
                    name="Market Close",
                    origin="/market",
                    overrides={
                        f"market/{ticker}": [
                            rr.SeriesLines(names=f"{ticker} (Close)", colors=colors.get(ticker)).visualizer(
                                mappings=[
                                    VisualizerComponentMapping(
                                        target="Scalars:scalars",
                                        source_kind=ComponentSourceKind.SourceComponent,
                                        source_component="MarketTelemetry:data",
                                        selector=".prices.close",
                                    )
                                ]
                            )
                        ]
                        for ticker in tickers
                    },
                    axis_x=rrb.TimeAxis(
                        view_range=rrb.TimeRange(
                            start=rrb.TimeRangeBoundary.infinite(), end=rrb.TimeRangeBoundary.infinite()
                        )
                    ),
                ),
            ),
            rrb.DataframeView(name="Market Inspector", origin="/market"),
        )
    )


def main() -> None:
    rr.init("rerun_example_any_scalar_market", spawn=True)

    run_market_demo()

    rr.send_blueprint(generate_blueprint())


if __name__ == "__main__":
    main()
