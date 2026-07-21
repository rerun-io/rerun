//! Turn built-in viewer URLs into [`re_ui::LinkButton`]s.

use std::str::FromStr as _;
use std::sync::Arc;

use ahash::HashMap;

use egui::{AtomExt as _, Theme};

use re_log_types::{EntryId, EntryName};
use re_ui::{Icon, LinkButton, icons};

use crate::open_url::{INTRA_RECORDING_URL_SCHEME, ViewerOpenUrl};

/// The icon to use
#[derive(Clone, Copy)]
pub enum LinkKind {
    Recording,
    Dataset,
    Table,
    Folder,
    Proxy,
}

impl LinkKind {
    /// The themed link icon for this kind. These are full-color (fixed blue arrow), so they come in
    /// a light and dark variant rather than being tinted to the text color.
    fn icon(self, theme: Theme) -> Icon {
        match (self, theme) {
            (Self::Recording, Theme::Light) => icons::LINK_RECORDING_LIGHT,
            (Self::Recording, Theme::Dark) => icons::LINK_RECORDING_DARK,
            (Self::Dataset, Theme::Light) => icons::LINK_DATASET_LIGHT,
            (Self::Dataset, Theme::Dark) => icons::LINK_DATASET_DARK,
            (Self::Table, Theme::Light) => icons::LINK_TABLE_LIGHT,
            (Self::Table, Theme::Dark) => icons::LINK_TABLE_DARK,
            (Self::Folder, Theme::Light) => icons::LINK_FOLDER_LIGHT,
            (Self::Folder, Theme::Dark) => icons::LINK_FOLDER_DARK,
            (Self::Proxy, Theme::Light) => icons::LINK_PROXY_LIGHT,
            (Self::Proxy, Theme::Dark) => icons::LINK_PROXY_DARK,
        }
    }
}

/// Tint for the monochrome single-variant icons.
fn icon_tint(theme: Theme) -> egui::Color32 {
    match theme {
        Theme::Light => egui::Color32::BLACK,
        Theme::Dark => egui::Color32::WHITE,
    }
}

/// Resolved display info for a redap entry (dataset/table), used to label a link button.
#[derive(Clone)]
pub struct ResolvedEntry {
    pub name: EntryName,
    pub kind: LinkKind,
}

/// Maps a redap entry reference to its resolved name + icon.
///
/// Built once per frame by the app and captured by the installed URL decorator.
pub type UrlNameLookup = HashMap<(re_uri::Origin, EntryId), ResolvedEntry>;

/// Build the global URL decorator closure for this frame's `lookup` snapshot.
///
/// Install it with [`re_ui::UrlDecorator::set`].
pub fn make_url_decorator(
    lookup: Arc<UrlNameLookup>,
    theme: Theme,
) -> impl Fn(&str) -> Option<LinkButton> + Send + Sync + 'static {
    move |url| url_atoms(url, &lookup, theme)
}

/// Parse a known viewer URL and build its decorated button.
///
/// Dataset/entry ids are resolved to catalog names via `lookup`, falling back to a short-id
/// placeholder on a miss.
pub fn url_atoms(url: &str, lookup: &UrlNameLookup, theme: Theme) -> Option<LinkButton> {
    let button = match ViewerOpenUrl::from_str(url).ok()? {
        ViewerOpenUrl::RedapDatasetSegment(uri) => {
            let (_, dataset_label) = resolve(lookup, &uri.origin, EntryId::from(uri.dataset_id));
            let atoms = dataset_segment_button(&dataset_label, uri.segment_id.as_str(), theme);
            Some(LinkButton::new(url, atoms))
        }

        ViewerOpenUrl::RedapEntry(uri) => {
            let (kind, label) = resolve(lookup, &uri.origin, uri.entry_id);
            Some(LinkButton::new(url, (kind.icon(theme), label)))
        }

        ViewerOpenUrl::RedapCatalog(uri) => {
            let label = uri.origin.host.to_string();
            Some(LinkButton::new(url, (LinkKind::Proxy.icon(theme), label)))
        }

        ViewerOpenUrl::RedapProxy(uri) => {
            let label = uri.origin.host.to_string();
            Some(LinkButton::new(url, (LinkKind::Proxy.icon(theme), label)))
        }

        ViewerOpenUrl::RedapFolder(uri) => {
            let label = folder_leaf(&uri.path);
            Some(LinkButton::new(url, (LinkKind::Folder.icon(theme), label)))
        }

        ViewerOpenUrl::IntraRecordingSelection(_) => {
            let image = icons::ENTITY.as_image().tint(icon_tint(theme));
            let label = url
                .strip_prefix(INTRA_RECORDING_URL_SCHEME)
                .unwrap_or(url)
                .to_owned();
            Some(LinkButton::new(url, (image, label)))
        }

        ViewerOpenUrl::HttpUrl(http_url) => {
            // Show just the file name:
            let name = http_url
                .path_segments()
                .and_then(|mut segments| segments.rfind(|segment| !segment.is_empty()))
                .map(str::to_owned)
                .unwrap_or_else(|| http_url.host_str().unwrap_or(http_url.as_str()).to_owned());
            Some(LinkButton::new(
                url,
                (LinkKind::Recording.icon(theme), name),
            ))
        }

        #[cfg(not(target_arch = "wasm32"))]
        ViewerOpenUrl::FilePath(path) => {
            // Show just the file name:
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.to_string_lossy().into_owned());
            Some(LinkButton::new(
                url,
                (LinkKind::Recording.icon(theme), name),
            ))
        }

        ViewerOpenUrl::WebViewerUrl { url_parameters, .. } => {
            // A web-viewer share link wrapping one or more content URLs: show the inner one's button,
            // but keep opening the outer share link on click.
            let inner = url_atoms(
                &url_parameters.first().sharable_url(None).ok()?,
                lookup,
                theme,
            )?;
            Some(LinkButton::new(url, inner.into_atoms()))
        }

        // No meaningful button content.
        ViewerOpenUrl::WebEventListener
        | ViewerOpenUrl::Settings
        | ViewerOpenUrl::ChunkStoreBrowser { .. } => None,
    };

    // The icons have a blue arrow so we may not tint them:
    button.map(|b| b.tint_icons(false))
}

/// Button atoms for a lone segment, used where the dataset is already implied by context (e.g. a
/// segment link shown within its own dataset's table, where repeating the dataset name is redundant).
pub fn segment_button_atoms(segment_id: &str, theme: Theme) -> egui::Atoms<'static> {
    egui::Atoms::new((LinkKind::Recording.icon(theme), segment_id.to_owned()))
}

fn dataset_segment_button(dataset: &str, segment: &str, theme: Theme) -> egui::Atoms<'static> {
    let tint = icon_tint(theme);
    egui::Atoms::new((
        icons::DATASET.as_image().tint(tint),
        dataset.to_owned(),
        icons::BREADCRUMBS_SEPARATOR.as_image().tint(tint),
        LinkKind::Recording.icon(theme).as_image(),
        segment.to_owned().atom_shrink(true),
    ))
}

/// Resolve an entry to its `(link kind, label)`, or a dataset-kind + short-id placeholder on a miss.
fn resolve(
    lookup: &UrlNameLookup,
    origin: &re_uri::Origin,
    entry_id: EntryId,
) -> (LinkKind, String) {
    if let Some(resolved) = lookup.get(&(origin.clone(), entry_id)) {
        (resolved.kind, resolved.name.to_string())
    } else {
        (LinkKind::Dataset, short_id(&entry_id.to_string()))
    }
}

/// First few characters of a hex id — enough to recognize, while the full URL is shown on hover.
fn short_id(id: &str) -> String {
    const N: usize = 8;
    if id.len() > N {
        format!("{}…", &id[..N])
    } else {
        id.to_owned()
    }
}

/// The leaf of a dotted dataset-hierarchy folder path (e.g. `perception.detection` → `detection`).
fn folder_leaf(path: &str) -> String {
    path.rsplit('.')
        .next()
        .filter(|leaf| !leaf.is_empty())
        .unwrap_or(path)
        .to_owned()
}
