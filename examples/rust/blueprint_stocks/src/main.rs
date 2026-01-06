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

/// Static JSON data
const TICKER_INFO_JSON: &str = include_str!("data/ticker_info.json");
const QUOTE_RANGE_JSON: &str = include_str!("data/quote_range.json");

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

/// Brand colors for each stock symbol.
fn brand_color(symbol: &str) -> u32 {
    match symbol {
        "AAPL" => 0xA2AAADFF,
        "AMZN" => 0xFF9900FF,
        "GOOGL" => 0x34A853FF,
        "META" => 0x0081FBFF,
        "MSFT" => 0xF14F21FF,
        _ => 0xFFFFFFFF,
    }
}

fn style_plot(symbol: &str) -> rerun::SeriesLines {
    rerun::SeriesLines::new()
        .with_colors([brand_color(symbol)])
        .with_names([symbol])
}

fn style_peak(symbol: &str) -> rerun::SeriesPoints {
    rerun::SeriesPoints::new()
        .with_colors([0xFF0000FF])
        .with_names([format!("{symbol} (peak)")])
        .with_markers([rerun::components::MarkerShape::Up])
}

/// A blueprint enabling auto views, which matches the application default.
fn auto_blueprint() -> Blueprint {
    Blueprint::auto()
}

/// Create a blueprint showing a single stock.
fn one_stock(symbol: &str) -> ContainerLike {
    TimeSeriesView::new(symbol)
        .with_origin(format!("/stocks/{symbol}"))
        .into()
}

/// Create a blueprint showing a single stock with its info arranged vertically.
fn one_stock_with_info(symbol: &str) -> ContainerLike {
    Vertical::new(vec![
        TextDocumentView::new(symbol)
            .with_origin(format!("/stocks/{symbol}/info"))
            .into(),
        TimeSeriesView::new(symbol)
            .with_origin(format!("/stocks/{symbol}"))
            .into(),
    ])
    .with_row_shares([1.0, 4.0])
    .into()
}

/// Create a blueprint comparing 2 stocks for a single day.
fn compare_two(symbol1: &str, symbol2: &str, day: &str) -> ContainerLike {
    TimeSeriesView::new(format!("{symbol1} vs {symbol2} ({day})"))
        .with_contents([
            format!("+ /stocks/{symbol1}/{day}"),
            format!("+ /stocks/{symbol2}/{day}"),
        ])
        .into()
}

/// Create a blueprint showing a single stock without annotated peaks.
fn one_stock_no_peaks(symbol: &str) -> ContainerLike {
    TimeSeriesView::new(symbol)
        .with_origin(format!("/stocks/{symbol}"))
        .with_contents(["+ $origin/**", "- $origin/peaks/**"])
        .into()
}

/// Create a grid of stocks and their time series over all days.
fn stock_grid(symbols: &[&str], dates: &[&str]) -> ContainerLike {
    let rows: Vec<ContainerLike> = symbols
        .iter()
        .map(|&symbol| {
            let mut views: Vec<ContainerLike> = vec![
                TextDocumentView::new(symbol)
                    .with_origin(format!("/stocks/{symbol}/info"))
                    .into(),
            ];

            for &day in dates {
                views.push(
                    TimeSeriesView::new(day)
                        .with_origin(format!("/stocks/{symbol}/{day}"))
                        .into(),
                );
            }

            Horizontal::new(views).with_name(symbol).into()
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

    let symbols = ["AAPL", "AMZN", "GOOGL", "META", "MSFT"];

    // Load static data
    let ticker_info: HashMap<String, TickerInfo> = serde_json::from_str(TICKER_INFO_JSON)?;
    let quote_range: HashMap<String, Vec<QuoteData>> = serde_json::from_str(QUOTE_RANGE_JSON)?;

    // Extract dates from the first symbol's quotes
    let dates = quote_range
        .get("AAPL")
        .map(|quotes| extract_dates_from_quotes(quotes))
        .unwrap_or_default();

    let date_strings: Vec<String> = dates.iter().map(|d| d.to_string()).collect();
    let date_strs: Vec<&str> = date_strings.iter().map(|s| s.as_str()).collect();

    // Select the blueprint based on the command-line argument
    let blueprint = match args.blueprint {
        BlueprintMode::Auto => auto_blueprint(),
        BlueprintMode::OneStock => {
            let viewport = one_stock("AAPL");
            if args.show_panels {
                Blueprint::new(viewport)
            } else {
                hide_panels(viewport)
            }
        }
        BlueprintMode::OneStockWithInfo => {
            let viewport = one_stock_with_info("AMZN");
            if args.show_panels {
                Blueprint::new(viewport)
            } else {
                hide_panels(viewport)
            }
        }
        BlueprintMode::CompareTwo => {
            let viewport = compare_two("META", "MSFT", date_strs.last().unwrap());
            if args.show_panels {
                Blueprint::new(viewport)
            } else {
                hide_panels(viewport)
            }
        }
        BlueprintMode::OneStockNoPeaks => {
            let viewport = one_stock_no_peaks("GOOGL");
            if args.show_panels {
                Blueprint::new(viewport)
            } else {
                hide_panels(viewport)
            }
        }
        BlueprintMode::Grid => {
            let viewport = stock_grid(&symbols, &date_strs);
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
    for &symbol in &symbols {
        for &date_str in &date_strs {
            rec.set_time_sequence("stable_time", 0);
            rec.log_static(format!("stocks/{symbol}/{date_str}"), &style_plot(symbol))?;
            rec.log_static(
                format!("stocks/{symbol}/peaks/{date_str}"),
                &style_peak(symbol),
            )?;
        }
    }

    for &symbol in &symbols {
        // Log company information
        let info_md = if let Some(info) = ticker_info.get(symbol) {
            let market_cap = info
                .market_cap
                .map(format_large_number)
                .unwrap_or_else(|| "N/A".to_string());
            let revenue = info
                .total_revenue
                .map(format_large_number)
                .unwrap_or_else(|| "N/A".to_string());

            format!(
                "- **Name**: {}\n- **Industry**: {}\n- **Market cap**: ${}\n- **Total Revenue**: ${}\n",
                info.name, info.industry, market_cap, revenue
            )
        } else {
            format!("# {symbol}\n\nCompany information unavailable")
        };

        rec.set_time_sequence("stable_time", 0);
        rec.log_static(
            format!("stocks/{symbol}/info"),
            &rerun::TextDocument::new(info_md).with_media_type(rerun::MediaType::MARKDOWN),
        )?;

        // Log quote data
        if let Some(quotes) = quote_range.get(symbol) {
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
                            format!("stocks/{symbol}/{date_str}"),
                            &rerun::Scalars::new([quote.high]),
                        )?;

                        // Log peak
                        if Some(i) == peak_idx {
                            rec.log(
                                format!("stocks/{symbol}/peaks/{date_str}"),
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
