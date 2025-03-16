#!/usr/bin/env python3
"""
A simple application that fetches stock data from Yahoo Finance and visualizes it using the Rerun SDK.

The main focus of this example is using blueprints to control how the data is displayed in the viewer.
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
    """A blueprint enabling auto views, which matches the application default."""
    return rrb.Blueprint(auto_views=True, auto_layout=True)


def one_stock(symbol: str) -> rrb.ContainerLike:
    """Create a blueprint showing a single stock."""
    return rrb.TimeSeriesView(name=f"{symbol}", origin=f"/stocks/{symbol}")


def one_stock_with_info(symbol: str) -> rrb.ContainerLike:
    """Create a blueprint showing a single stock with its info arranged vertically."""
    return rrb.Vertical(
        rrb.TextDocumentView(name=f"{symbol}", origin=f"/stocks/{symbol}/info"),
        rrb.TimeSeriesView(name=f"{symbol}", origin=f"/stocks/{symbol}"),
        row_shares=[1, 4],
    )


def compare_two(symbol1: str, symbol2: str, day: Any) -> rrb.ContainerLike:
    """Create a blueprint comparing 2 stocks for a single day."""
    return rrb.TimeSeriesView(
        name=f"{symbol1} vs {symbol2} ({day})",
        contents=[
            f"+ /stocks/{symbol1}/{day}",
            f"+ /stocks/{symbol2}/{day}",
        ],
    )


def one_stock_no_peaks(symbol: str) -> rrb.ContainerLike:
    """
    Create a blueprint showing a single stock without annotated peaks.

    This uses an exclusion pattern to hide the peaks.
    """
    return rrb.TimeSeriesView(
        name=f"{symbol}",
        origin=f"/stocks/{symbol}",
        contents=[
            "+ $origin/**",
            "- $origin/peaks/**",
        ],
    )


def stock_grid(symbols: list[str], dates: list[Any]) -> rrb.ContainerLike:
    """Create a grid of stocks and their time series over all days."""
    return rrb.Vertical(
        contents=[
            rrb.Horizontal(
                contents=[rrb.TextDocumentView(name=f"{symbol}", origin=f"/stocks/{symbol}/info")]
                + [rrb.TimeSeriesView(name=f"{day}", origin=f"/stocks/{symbol}/{day}") for day in dates],
                name=symbol,
            )
            for symbol in symbols
        ],
    )


def hide_panels(viewport: rrb.ContainerLike) -> rrb.BlueprintLike:
    """Wrap a viewport in a blueprint that hides the time and selection panels."""
    return rrb.Blueprint(
        viewport,
        rrb.TimePanel(state="collapsed"),
        rrb.SelectionPanel(state="collapsed"),
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
        marker="up",
    )


################################################################################
# Main script
################################################################################


def main() -> None:
    parser = argparse.ArgumentParser(description="Visualize stock data using the Rerun SDK")
    parser.add_argument(
        "--blueprint",
        choices=["auto", "one-stock", "one-stock-with-info", "compare-two", "one-stock-no-peaks", "grid"],
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
        if args.blueprint == "one-stock":
            viewport = one_stock("AAPL")
        elif args.blueprint == "one-stock-with-info":
            viewport = one_stock_with_info("AMZN")
        elif args.blueprint == "one-stock-no-peaks":
            viewport = one_stock_no_peaks("GOOGL")
        elif args.blueprint == "compare-two":
            viewport = compare_two("META", "MSFT", dates[-1])
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
            rr.log(f"stocks/{symbol}/{day}", style_plot(symbol), static=True)
            rr.log(f"stocks/{symbol}/peaks/{day}", style_peak(symbol), static=True)

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
            static=True,
        )

        for day in dates:
            open_time = dt.datetime.combine(day, dt.time(9, 30))
            close_time = dt.datetime.combine(day, dt.time(16, 00))

            hist = stock.history(start=open_time, end=close_time, interval="5m")
            if len(hist.index) == 0:
                continue

            hist.index = hist.index - et_timezone.localize(open_time)
            peak = hist.High.idxmax()

            for row in hist.itertuples():
                rr.set_time("time", timedelta=row.Index)
                rr.log(f"stocks/{symbol}/{day}", rr.Scalar(row.High))
                if row.Index == peak:
                    rr.log(f"stocks/{symbol}/peaks/{day}", rr.Scalar(row.High))

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
