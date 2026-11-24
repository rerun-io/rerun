"""Build up a blueprint step by step for synthetic stock prices."""

# region: script
import datetime as dt

import numpy as np

import rerun as rr
import rerun.blueprint as rrb

brand_colors = {
    "AAPL": 0xA2AAADFF,
    "AMZN": 0xFF9900FF,
    "GOOGL": 0x34A853FF,
    "META": 0x0081FBFF,
    "MSFT": 0xF14F21FF,
}

company_info = {
    "AAPL": ("Apple Inc.", "Consumer Electronics"),
    "AMZN": ("Amazon.com, Inc.", "Internet Retail"),
    "GOOGL": ("Alphabet Inc.", "Internet Content & Information"),
    "META": ("Meta Platforms, Inc.", "Internet Content & Information"),
    "MSFT": ("Microsoft Corporation", "Software - Infrastructure"),
}


def style_plot(symbol: str) -> rr.SeriesLines:
    return rr.SeriesLines(colors=brand_colors[symbol], names=symbol)


def style_peak(symbol: str) -> rr.SeriesPoints:
    return rr.SeriesPoints(
        colors=0xFF0000FF, names=f"{symbol} (peak)", markers="up"
    )


def info_card(symbol: str) -> rr.TextDocument:
    name, industry = company_info[symbol]
    markdown = f"- **Name**: {name}\n- **Industry**: {industry}\n"
    return rr.TextDocument(markdown, media_type=rr.MediaType.MARKDOWN)


symbols = list(brand_colors)
start_date = dt.date(2024, 3, 18)
dates = [start_date + dt.timedelta(days=i) for i in range(5)]

rr.init("rerun_example_blueprint_stocks", spawn=True)

rng = np.random.default_rng(0)

for symbol in symbols:
    rr.log(f"stocks/{symbol}/info", info_card(symbol), static=True)

    for day in dates:
        rr.log(f"stocks/{symbol}/{day}", style_plot(symbol), static=True)
        rr.log(f"stocks/{symbol}/peaks/{day}", style_peak(symbol), static=True)

        # One price every 5 minutes from market open (9:30) to close (16:00).
        prices = 100.0 + np.cumsum(rng.normal(size=78))
        peak = int(np.argmax(prices))

        for i, price in enumerate(prices):
            rr.set_time("time", duration=dt.timedelta(minutes=5 * i))
            rr.log(f"stocks/{symbol}/{day}", rr.Scalars(price))
            if i == peak:
                rr.log(f"stocks/{symbol}/peaks/{day}", rr.Scalars(price))
# endregion: script

# region: one_stock
# Create a single chart for all the AAPL data:
blueprint = rrb.Blueprint(
    rrb.TimeSeriesView(name="AAPL", origin="/stocks/AAPL"),
)
rr.send_blueprint(blueprint)
# endregion: one_stock

# region: hide_panels
# Create a single chart and collapse the selection and time panels:
blueprint = rrb.Blueprint(
    rrb.TimeSeriesView(name="AAPL", origin="/stocks/AAPL"),
    rrb.BlueprintPanel(state="expanded"),
    rrb.SelectionPanel(state="collapsed"),
    rrb.TimePanel(state="collapsed"),
)
rr.send_blueprint(blueprint)
# endregion: hide_panels

# region: one_stock_with_info
# Create a vertical layout of an info document and a time series chart
blueprint = rrb.Blueprint(
    rrb.Vertical(
        rrb.TextDocumentView(name="Info", origin="/stocks/AAPL/info"),
        rrb.TimeSeriesView(name="Chart", origin="/stocks/AAPL"),
        row_shares=[1, 4],
    ),
    rrb.BlueprintPanel(state="expanded"),
    rrb.SelectionPanel(state="collapsed"),
    rrb.TimePanel(state="collapsed"),
)
rr.send_blueprint(blueprint)
# endregion: one_stock_with_info

# region: compare_two
# Create a view with two stock time series
blueprint = rrb.Blueprint(
    rrb.TimeSeriesView(
        name="META vs MSFT",
        contents=[
            "+ /stocks/META/2024-03-19",
            "+ /stocks/MSFT/2024-03-19",
        ],
    ),
    rrb.BlueprintPanel(state="expanded"),
    rrb.SelectionPanel(state="collapsed"),
    rrb.TimePanel(state="collapsed"),
)
rr.send_blueprint(blueprint)
# endregion: compare_two

# region: no_peaks
# Create a chart for AAPL and filter out the peaks:
blueprint = rrb.Blueprint(
    rrb.TimeSeriesView(
        name="AAPL",
        origin="/stocks/AAPL",
        contents=[
            "+ $origin/**",
            "- $origin/peaks/**",
        ],
    ),
    rrb.BlueprintPanel(state="expanded"),
    rrb.SelectionPanel(state="collapsed"),
    rrb.TimePanel(state="collapsed"),
)
rr.send_blueprint(blueprint)
# endregion: no_peaks

# region: grid
# Iterate over all symbols and days to create a comprehensive grid
blueprint = rrb.Blueprint(
    rrb.Vertical(
        contents=[
            rrb.Horizontal(
                contents=[
                    rrb.TextDocumentView(
                        name=f"{symbol}",
                        origin=f"/stocks/{symbol}/info",
                    ),
                ]
                + [
                    rrb.TimeSeriesView(
                        name=f"{day}",
                        origin=f"/stocks/{symbol}/{day}",
                    )
                    for day in dates
                ],
                name=symbol,
            )
            for symbol in symbols
        ]
    ),
    rrb.BlueprintPanel(state="expanded"),
    rrb.SelectionPanel(state="collapsed"),
    rrb.TimePanel(state="collapsed"),
)
rr.send_blueprint(blueprint)
# endregion: grid
