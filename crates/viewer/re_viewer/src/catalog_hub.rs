use ahash::HashMap;
use url::Url;

use re_chunk::external::arrow::array::RecordBatch;

/// An individual catalog.
pub struct Catalog {
    data: RecordBatch,
}

/// All catalogs known to the viewer.
#[derive(Default)]
pub struct CatalogHub {
    catalogs: HashMap<Url, Catalog>,
    //in_flight_requests: HashMap<Uri, Future<Result<Catalog, Error>>>,
}

impl CatalogHub {
    /// Asynchronously fetches a catalog from a URL and adds it to the hub.
    ///
    /// If this url was used before, it will refresh the existing catalog in the hub.
    pub fn fetch_catalog(&mut self, url: Url) {
        /// TODO : async things.
        ///
        ///
        re_log::debug!("Catalog data source: {url}");

        // TODO:
        // * create grpc client
        // * spawn a future that loads the catalog
        // * that future puts it into a global accessible catalog hub
        // TODO:
        // * some indication of background progress
    }
}
