use anyhow::Result;
use chrono::NaiveDate;
use clap::Parser;
use rerun::{
    blueprint::{
        Blueprint, ContainerLike, Horizontal, SelectionPanel, TextDocumentView, TimePanel,
        TimeSeriesView, Vertical,
    },
    external::re_sdk_types::blueprint::components::PanelState,
};
use serde::Deserialize;
use std::collections::{BTreeSet, HashMap};
use strum::{EnumIter, IntoEnumIterator};

#[derive(Debug, Clone, Copy, EnumIter)]
#[allow(clippy::upper_case_acronyms)]
enum Ticker {
    AAPL,
    AMZN,
    GOOGL,
    META,
    MSFT,
}

impl Ticker {
    fn as_str(&self) -> &'static str {
        match self {
            Ticker::AAPL => "AAPL",
            Ticker::AMZN => "AMZN",
            Ticker::GOOGL => "GOOGL",
            Ticker::META => "META",
            Ticker::MSFT => "MSFT",
        }
    }
}

#[derive(Debug, Deserialize)]
struct TickerInfo {
    name: String,
    industry: String,
    market_cap: Option<u64>,
    total_revenue: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct QuoteData {
    timestamp: i64,
    high: f64,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum BlueprintMode {
    Auto,
    OneStock,
    OneStockWithInfo,
    CompareTwo,
    OneStockNoPeaks,
    Grid,
}

#[derive(Debug, Parser)]
#[command(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,

    /// Select the blueprint to use
    #[arg(long, value_enum, default_value = "grid")]
    blueprint: BlueprintMode,

    /// Show the time and selection panels
    #[arg(long)]
    show_panels: bool,
}

/// Get ticker info JSON string for a ticker symbol
fn get_ticker_info_json(ticker: Ticker) -> &'static str {
    match ticker {
        Ticker::AAPL => include_str!("data/info/aapl.json"),
        Ticker::AMZN => include_str!("data/info/amzn.json"),
        Ticker::GOOGL => include_str!("data/info/googl.json"),
        Ticker::META => include_str!("data/info/meta.json"),
        Ticker::MSFT => include_str!("data/info/msft.json"),
    }
}

/// Get quote data JSON string for a ticker symbol
fn get_quote_data_json(ticker: Ticker) -> &'static str {
    match ticker {
        Ticker::AAPL => include_str!("data/quotes/aapl.json"),
        Ticker::AMZN => include_str!("data/quotes/amzn.json"),
        Ticker::GOOGL => include_str!("data/quotes/googl.json"),
        Ticker::META => include_str!("data/quotes/meta.json"),
        Ticker::MSFT => include_str!("data/quotes/msft.json"),
    }
}

/// Format large numbers in a human-readable way (e.g., 2.8T, 394B, 1.2M)
fn format_large_number(num: u64) -> String {
    let num_f = num as f64;
    if num_f >= 1e12 {
        format!("{:.1}T", num_f / 1e12)
    } else if num_f >= 1e9 {
        format!("{:.1}B", num_f / 1e9)
    } else if num_f >= 1e6 {
        format!("{:.1}M", num_f / 1e6)
    } else if num_f >= 1e3 {
        format!("{:.1}K", num_f / 1e3)
    } else {
        num.to_string()
    }
}

/// Extract unique dates from quote timestamps.
/// Returns dates sorted in ascending order.
fn extract_dates_from_quotes(quotes: &[QuoteData]) -> Vec<NaiveDate> {
    let dates: BTreeSet<NaiveDate> = quotes
        .iter()
        .filter_map(|q| chrono::DateTime::from_timestamp(q.timestamp, 0).map(|dt| dt.date_naive()))
        .collect();
    dates.into_iter().collect()
}

/// Group quotes by date.
fn group_quotes_by_date(quotes: &[QuoteData]) -> HashMap<NaiveDate, Vec<&QuoteData>> {
    let mut grouped: HashMap<NaiveDate, Vec<&QuoteData>> = HashMap::new();
    for quote in quotes {
        if let Some(dt) = chrono::DateTime::from_timestamp(quote.timestamp, 0) {
            let date = dt.date_naive();
            grouped.entry(date).or_default().push(quote);
        }
    }
    grouped
}

/// Brand colors for each stock ticker.
fn brand_color(ticker: Ticker) -> u32 {
    match ticker {
        Ticker::AAPL => 0xA2AAADFF,
        Ticker::AMZN => 0xFF9900FF,
        Ticker::GOOGL => 0x34A853FF,
        Ticker::META => 0x0081FBFF,
        Ticker::MSFT => 0xF14F21FF,
    }
}

fn style_plot(ticker: Ticker) -> rerun::SeriesLines {
    rerun::SeriesLines::new()
        .with_colors([brand_color(ticker)])
        .with_names([ticker.as_str()])
}

fn style_peak(ticker: Ticker) -> rerun::SeriesPoints {
    let ticker_str = ticker.as_str();
    rerun::SeriesPoints::new()
        .with_colors([0xFF0000FF])
        .with_names([format!("{ticker_str} (peak)")])
        .with_markers([rerun::components::MarkerShape::Up])
}

/// A blueprint enabling auto views, which matches the application default.
fn auto_blueprint() -> Blueprint {
    Blueprint::auto()
}

/// Create a blueprint showing a single stock.
fn one_stock(ticker: Ticker) -> ContainerLike {
    let ticker_str = ticker.as_str();
    TimeSeriesView::new(ticker_str)
        .with_origin(format!("/stocks/{ticker_str}"))
        .into()
}

/// Create a blueprint showing a single stock with its info arranged vertically.
fn one_stock_with_info(ticker: Ticker) -> ContainerLike {
    let ticker_str = ticker.as_str();
    Vertical::new(vec![
        TextDocumentView::new(ticker_str)
            .with_origin(format!("/stocks/{ticker_str}/info"))
            .into(),
        TimeSeriesView::new(ticker_str)
            .with_origin(format!("/stocks/{ticker_str}"))
            .into(),
    ])
    .with_row_shares([1.0, 4.0])
    .into()
}

/// Create a blueprint comparing 2 stocks for a single day.
fn compare_two(ticker1: Ticker, ticker2: Ticker, day: &str) -> ContainerLike {
    let ticker1_str = ticker1.as_str();
    let ticker2_str = ticker2.as_str();
    TimeSeriesView::new(format!("{ticker1_str} vs {ticker2_str} ({day})"))
        .with_contents([
            format!("+ /stocks/{ticker1_str}/{day}"),
            format!("+ /stocks/{ticker2_str}/{day}"),
        ])
        .into()
}

/// Create a blueprint showing a single stock without annotated peaks.
fn one_stock_no_peaks(ticker: Ticker) -> ContainerLike {
    let ticker_str = ticker.as_str();
    TimeSeriesView::new(ticker_str)
        .with_origin(format!("/stocks/{ticker_str}"))
        .with_contents(["+ $origin/**", "- $origin/peaks/**"])
        .into()
}

/// Create a grid of stocks and their time series over all days.
fn stock_grid(tickers: &[Ticker], dates: &[&str]) -> ContainerLike {
    let rows: Vec<ContainerLike> = tickers
        .iter()
        .map(|&ticker| {
            let ticker_str = ticker.as_str();
            let mut views: Vec<ContainerLike> = vec![
                TextDocumentView::new(ticker_str)
                    .with_origin(format!("/stocks/{ticker_str}/info"))
                    .into(),
            ];

            for &day in dates {
                views.push(
                    TimeSeriesView::new(day)
                        .with_origin(format!("/stocks/{ticker_str}/{day}"))
                        .into(),
                );
            }

            Horizontal::new(views).with_name(ticker_str).into()
        })
        .collect();

    Vertical::new(rows).into()
}

/// Wrap a viewport in a blueprint that hides the time and selection panels.
fn hide_panels(viewport: ContainerLike) -> Blueprint {
    Blueprint::new(viewport)
        .with_time_panel(TimePanel::new().with_state(PanelState::Collapsed))
        .with_selection_panel(SelectionPanel::from_state(PanelState::Collapsed))
}

fn main() -> Result<()> {
    let args = Args::parse();

    let tickers: Vec<Ticker> = Ticker::iter().collect();

    // Load data from individual JSON files
    let ticker_info: HashMap<String, TickerInfo> = tickers
        .iter()
        .map(|&ticker| {
            (
                ticker.as_str().to_owned(),
                serde_json::from_str(get_ticker_info_json(ticker)).unwrap(),
            )
        })
        .collect();

    let quote_range: HashMap<String, Vec<QuoteData>> = tickers
        .iter()
        .map(|&ticker| {
            (
                ticker.as_str().to_owned(),
                serde_json::from_str(get_quote_data_json(ticker)).unwrap(),
            )
        })
        .collect();

    // Extract dates from the first tickers's quotes
    let dates = quote_range
        .get(Ticker::AAPL.as_str())
        .map(|quotes| extract_dates_from_quotes(quotes))
        .unwrap_or_default();

    let date_strings: Vec<String> = dates.iter().map(|d| d.to_string()).collect();
    let date_strs: Vec<&str> = date_strings.iter().map(|s| s.as_str()).collect();

    // Select the blueprint based on the command-line argument
    let blueprint = match args.blueprint {
        BlueprintMode::Auto => auto_blueprint(),
        BlueprintMode::OneStock => {
            let viewport = one_stock(Ticker::AAPL);
            if args.show_panels {
                Blueprint::new(viewport)
            } else {
                hide_panels(viewport)
            }
        }
        BlueprintMode::OneStockWithInfo => {
            let viewport = one_stock_with_info(Ticker::AMZN);
            if args.show_panels {
                Blueprint::new(viewport)
            } else {
                hide_panels(viewport)
            }
        }
        BlueprintMode::CompareTwo => {
            let viewport = compare_two(Ticker::META, Ticker::MSFT, date_strs.last().unwrap());
            if args.show_panels {
                Blueprint::new(viewport)
            } else {
                hide_panels(viewport)
            }
        }
        BlueprintMode::OneStockNoPeaks => {
            let viewport = one_stock_no_peaks(Ticker::GOOGL);
            if args.show_panels {
                Blueprint::new(viewport)
            } else {
                hide_panels(viewport)
            }
        }
        BlueprintMode::Grid => {
            let viewport = stock_grid(&tickers, &date_strs);
            if args.show_panels {
                Blueprint::new(viewport)
            } else {
                hide_panels(viewport)
            }
        }
    };

    let (rec, _serve_guard) = args
        .rerun
        .init_with_blueprint("rerun_example_blueprint_stocks", blueprint)?;

    // Log styling for plots (static)
    for &ticker in &tickers {
        let ticker_str = ticker.as_str();
        for &date_str in &date_strs {
            rec.set_time_sequence("stable_time", 0);
            rec.log_static(
                format!("stocks/{ticker_str}/{date_str}"),
                &style_plot(ticker),
            )?;
            rec.log_static(
                format!("stocks/{ticker_str}/peaks/{date_str}"),
                &style_peak(ticker),
            )?;
        }
    }

    for &ticker in &tickers {
        let ticker_str = ticker.as_str();
        // Log company information
        let info_md = if let Some(info) = ticker_info.get(ticker_str) {
            let market_cap = info
                .market_cap
                .map(format_large_number)
                .unwrap_or_else(|| "N/A".to_owned());
            let revenue = info
                .total_revenue
                .map(format_large_number)
                .unwrap_or_else(|| "N/A".to_owned());

            format!(
                "- **Name**: {}\n- **Industry**: {}\n- **Market cap**: ${}\n- **Total Revenue**: ${}\n",
                info.name, info.industry, market_cap, revenue
            )
        } else {
            format!("# {ticker_str}\n\nCompany information unavailable")
        };

        rec.set_time_sequence("stable_time", 0);
        rec.log_static(
            format!("stocks/{ticker_str}/info"),
            &rerun::TextDocument::new(info_md).with_media_type(rerun::MediaType::MARKDOWN),
        )?;

        // Log quote data
        if let Some(quotes) = quote_range.get(ticker_str) {
            let quotes_by_date = group_quotes_by_date(quotes);

            for date in &dates {
                let date_str = date.to_string();

                if let Some(day_quotes) = quotes_by_date.get(date) {
                    if day_quotes.is_empty() {
                        continue;
                    }

                    // Find peak for this day
                    let peak_idx = day_quotes
                        .iter()
                        .enumerate()
                        .max_by(|(_, a), (_, b)| {
                            a.high
                                .partial_cmp(&b.high)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .map(|(idx, _)| idx);

                    // Log time series data
                    for (i, quote) in day_quotes.iter().enumerate() {
                        rec.set_time_sequence("time", i as i64);
                        rec.log(
                            format!("stocks/{ticker_str}/{date_str}"),
                            &rerun::Scalars::new([quote.high]),
                        )?;

                        // Log peak
                        if Some(i) == peak_idx {
                            rec.log(
                                format!("stocks/{ticker_str}/peaks/{date_str}"),
                                &rerun::Scalars::new([quote.high]),
                            )?;
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
