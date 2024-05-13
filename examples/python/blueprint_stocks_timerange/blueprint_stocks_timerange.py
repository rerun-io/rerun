#!/usr/bin/env python3
"""
A simple application that fetches stock data from Yahoo Finance and visualizes it using the Rerun SDK.

The main focus of this example is using blueprints to control how the data is displayed in the viewer.

This is an alternative version of the blueprint_stocks example that uses time ranges to create the daily
time series for each stock. This allows the underlying data to be stored on a single entity rather than
splitting it across multiple entities for each day.
"""

from __future__ import annotations

import argparse
import datetime as dt
from typing import Any

import humanize
import pytz
import rerun as rr
import rerun.blueprint as rrb
import yfinance as yf

################################################################################
# Helper functions to create blueprints
################################################################################


def auto_blueprint() -> rrb.BlueprintLike:
    """A blueprint enabling auto space views, which matches the application default."""
    return rrb.Blueprint(auto_space_views=True, auto_layout=True)


def time_ranges_for_day(day: dt.date) -> list[rrb.VisibleTimeRange]:
    """Create a time range for a single day."""
    open = dt.datetime.combine(day, dt.time(9, 30)).timestamp()
    close = dt.datetime.combine(day, dt.time(16, 0)).timestamp()
    return [
        rrb.VisibleTimeRange(
            "time",
            start=rrb.TimeRangeBoundary.absolute(seconds=open),
            end=rrb.TimeRangeBoundary.absolute(seconds=close),
        )
    ]


def one_stock_on_day(symbol: str, day: dt.date) -> rrb.ContainerLike:
    """Create a blueprint showing a single stock."""
    return rrb.TimeSeriesView(
        name=f"{symbol}: {day}",
        origin=f"/stocks/{symbol}",
        time_ranges=time_ranges_for_day(day),
    )


def compare_two_over_time(symbol1: str, symbol2: str, dates: list[dt.date]) -> rrb.ContainerLike:
    """Create a blueprint comparing 2 stocks for a single day."""
    return rrb.Vertical(
        name=f"{symbol1} vs {symbol2}",
        contents=[
            rrb.TimeSeriesView(
                name=f"{day}",
                origin="/stocks",
                contents=[
                    f"+ $origin/{symbol1}",
                    f"+ $origin/{symbol2}",
                ],
                time_ranges=time_ranges_for_day(day),
            )
            for day in dates
        ],
    )


def stock_grid(symbols: list[str], dates: list[Any]) -> rrb.ContainerLike:
    """Create a grid of stocks and their time series over all days."""
    return rrb.Vertical(
        contents=[
            rrb.Horizontal(
                contents=[rrb.TextDocumentView(name=f"{symbol}", origin=f"/stocks/{symbol}/info")]
                + [
                    rrb.TimeSeriesView(
                        name=f"{day}",
                        origin=f"/stocks/{symbol}",
                        time_ranges=time_ranges_for_day(day),
                    )
                    for day in dates
                ],
                name=symbol,
            )
            for symbol in symbols
        ]
    )


def hide_panels(viewport: rrb.ContainerLike) -> rrb.BlueprintLike:
    """Wrap a viewport in a blueprint that hides the time and selection panels."""
    return rrb.Blueprint(
        viewport,
        rrb.TimePanel(expanded=True),
        rrb.SelectionPanel(expanded=False),
    )


################################################################################
# Helper functions for styling
################################################################################

brand_colors = {
    "AAPL": 0xA2AAADFF,
    "AMZN": 0xFF9900FF,
    "GOOGL": 0x34A853FF,
    "META": 0x0081FBFF,
    "MSFT": 0xF14F21FF,
}


def style_plot(symbol: str) -> rr.SeriesLine:
    return rr.SeriesLine(
        color=brand_colors[symbol],
        name=symbol,
    )


def style_peak(symbol: str) -> rr.SeriesPoint:
    return rr.SeriesPoint(
        color=0xFF0000FF,
        name=f"{symbol} (peak)",
        marker="Up",
    )


################################################################################
# Main script
################################################################################


def main() -> None:
    parser = argparse.ArgumentParser(description="Visualize stock data using the Rerun SDK")
    parser.add_argument(
        "--blueprint",
        choices=["auto", "one", "compare", "grid"],
        default="grid",
        help="Select the blueprint to use",
    )
    parser.add_argument(
        "--show_panels",
        action="store_true",
        help="Show the time and selection panels",
    )

    rr.script_add_args(parser)
    args = parser.parse_args()

    et_timezone = pytz.timezone("America/New_York")
    current_date = dt.datetime.now(et_timezone).date()
    symbols = ["AAPL", "AMZN", "GOOGL", "META", "MSFT"]
    dates = list(filter(lambda x: x.weekday() < 5, [current_date - dt.timedelta(days=i) for i in range(7, 0, -1)]))

    if args.blueprint == "auto":
        blueprint = auto_blueprint()
    else:
        if args.blueprint == "one":
            viewport = one_stock_on_day("AAPL", dates[3])
        elif args.blueprint == "compare":
            viewport = compare_two_over_time("META", "MSFT", dates)
        elif args.blueprint == "grid":
            viewport = stock_grid(symbols, dates)
        else:
            raise ValueError(f"Unknown blueprint: {args.blueprint}")

        if not args.show_panels:
            blueprint = hide_panels(viewport)
        else:
            blueprint = viewport

    rr.script_setup(args, "rerun_example_blueprint_stocks")
    rr.send_blueprint(blueprint)

    # In a future blueprint release, this can move into the blueprint as well
    for symbol in symbols:
        for day in dates:
            rr.log(f"stocks/{symbol}", style_plot(symbol), timeless=True)
            rr.log(f"stocks/{symbol}/daily_peaks", style_peak(symbol), timeless=True)

    for symbol in symbols:
        stock = yf.Ticker(symbol)

        name = stock.info["shortName"]
        industry = stock.info["industry"]
        marketCap = humanize.intword(stock.info["marketCap"])
        revenue = humanize.intword(stock.info["totalRevenue"])

        info_md = (
            f"- **Name**: {name}\n"
            f"- **Industry**: {industry}\n"
            f"- **Market cap**: ${marketCap}\n"
            f"- **Total Revenue**: ${revenue}\n"
        )

        rr.log(
            f"stocks/{symbol}/info",
            rr.TextDocument(info_md, media_type=rr.MediaType.MARKDOWN),
            timeless=True,
        )

        min_time = dt.datetime.combine(dates[0], dt.time(0, 0))
        max_time = dt.datetime.combine(dates[-1], dt.time(16, 00))

        hist = stock.history(start=min_time, end=max_time, interval="5m")
        if len(hist.index) == 0:
            continue

        daily_peaks = []

        for day in dates:
            open_time = et_timezone.localize(dt.datetime.combine(day, dt.time(9, 30)))
            close_time = et_timezone.localize(dt.datetime.combine(day, dt.time(16, 00)))
            daily_peaks.append(hist.loc[open_time:close_time].High.idxmax())  # type: ignore[misc]

        for row in hist.itertuples():
            rr.set_time_seconds("time", row.Index.timestamp())
            rr.log(f"stocks/{symbol}", rr.Scalar(row.High))
            if row.Index in daily_peaks:
                rr.log(f"stocks/{symbol}/daily_peaks", rr.Scalar(row.High))

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
