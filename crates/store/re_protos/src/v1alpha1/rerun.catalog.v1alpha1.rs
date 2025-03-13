// This file is @generated by prost-build.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FindEntriesRequest {
    #[prost(message, optional, tag = "1")]
    pub filter: ::core::option::Option<EntryFilter>,
}
impl ::prost::Name for FindEntriesRequest {
    const NAME: &'static str = "FindEntriesRequest";
    const PACKAGE: &'static str = "rerun.catalog.v1alpha1";
    fn full_name() -> ::prost::alloc::string::String {
        "rerun.catalog.v1alpha1.FindEntriesRequest".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/rerun.catalog.v1alpha1.FindEntriesRequest".into()
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FindEntriesResponse {
    #[prost(message, repeated, tag = "1")]
    pub entries: ::prost::alloc::vec::Vec<EntryDetails>,
}
impl ::prost::Name for FindEntriesResponse {
    const NAME: &'static str = "FindEntriesResponse";
    const PACKAGE: &'static str = "rerun.catalog.v1alpha1";
    fn full_name() -> ::prost::alloc::string::String {
        "rerun.catalog.v1alpha1.FindEntriesResponse".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/rerun.catalog.v1alpha1.FindEntriesResponse".into()
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CreateDatasetEntryRequest {
    #[prost(message, optional, tag = "1")]
    pub dataset: ::core::option::Option<DatasetEntry>,
}
impl ::prost::Name for CreateDatasetEntryRequest {
    const NAME: &'static str = "CreateDatasetEntryRequest";
    const PACKAGE: &'static str = "rerun.catalog.v1alpha1";
    fn full_name() -> ::prost::alloc::string::String {
        "rerun.catalog.v1alpha1.CreateDatasetEntryRequest".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/rerun.catalog.v1alpha1.CreateDatasetEntryRequest".into()
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CreateDatasetEntryResponse {
    #[prost(message, optional, tag = "1")]
    pub dataset: ::core::option::Option<DatasetEntry>,
}
impl ::prost::Name for CreateDatasetEntryResponse {
    const NAME: &'static str = "CreateDatasetEntryResponse";
    const PACKAGE: &'static str = "rerun.catalog.v1alpha1";
    fn full_name() -> ::prost::alloc::string::String {
        "rerun.catalog.v1alpha1.CreateDatasetEntryResponse".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/rerun.catalog.v1alpha1.CreateDatasetEntryResponse".into()
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ReadDatasetEntryRequest {
    #[prost(message, optional, tag = "1")]
    pub key: ::core::option::Option<EntryKey>,
}
impl ::prost::Name for ReadDatasetEntryRequest {
    const NAME: &'static str = "ReadDatasetEntryRequest";
    const PACKAGE: &'static str = "rerun.catalog.v1alpha1";
    fn full_name() -> ::prost::alloc::string::String {
        "rerun.catalog.v1alpha1.ReadDatasetEntryRequest".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/rerun.catalog.v1alpha1.ReadDatasetEntryRequest".into()
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ReadDatasetEntryResponse {
    #[prost(message, optional, tag = "1")]
    pub dataset: ::core::option::Option<DatasetEntry>,
}
impl ::prost::Name for ReadDatasetEntryResponse {
    const NAME: &'static str = "ReadDatasetEntryResponse";
    const PACKAGE: &'static str = "rerun.catalog.v1alpha1";
    fn full_name() -> ::prost::alloc::string::String {
        "rerun.catalog.v1alpha1.ReadDatasetEntryResponse".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/rerun.catalog.v1alpha1.ReadDatasetEntryResponse".into()
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteDatasetEntryRequest {
    #[prost(message, optional, tag = "1")]
    pub key: ::core::option::Option<EntryKey>,
}
impl ::prost::Name for DeleteDatasetEntryRequest {
    const NAME: &'static str = "DeleteDatasetEntryRequest";
    const PACKAGE: &'static str = "rerun.catalog.v1alpha1";
    fn full_name() -> ::prost::alloc::string::String {
        "rerun.catalog.v1alpha1.DeleteDatasetEntryRequest".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/rerun.catalog.v1alpha1.DeleteDatasetEntryRequest".into()
    }
}
#[derive(Clone, Copy, PartialEq, ::prost::Message)]
pub struct DeleteDatasetEntryResponse {}
impl ::prost::Name for DeleteDatasetEntryResponse {
    const NAME: &'static str = "DeleteDatasetEntryResponse";
    const PACKAGE: &'static str = "rerun.catalog.v1alpha1";
    fn full_name() -> ::prost::alloc::string::String {
        "rerun.catalog.v1alpha1.DeleteDatasetEntryResponse".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/rerun.catalog.v1alpha1.DeleteDatasetEntryResponse".into()
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct EntryFilter {
    #[prost(message, optional, tag = "1")]
    pub id: ::core::option::Option<super::super::common::v1alpha1::EntryId>,
    #[prost(string, optional, tag = "2")]
    pub name: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(enumeration = "EntryType", optional, tag = "3")]
    pub entry_type: ::core::option::Option<i32>,
}
impl ::prost::Name for EntryFilter {
    const NAME: &'static str = "EntryFilter";
    const PACKAGE: &'static str = "rerun.catalog.v1alpha1";
    fn full_name() -> ::prost::alloc::string::String {
        "rerun.catalog.v1alpha1.EntryFilter".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/rerun.catalog.v1alpha1.EntryFilter".into()
    }
}
/// Minimal info about an Entry for high-level catalog summary
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct EntryDetails {
    /// The EntryId is immutable
    #[prost(message, optional, tag = "1")]
    pub id: ::core::option::Option<super::super::common::v1alpha1::EntryId>,
    /// The name is a short human-readable string
    /// TODO(jleibs): Define valid name constraints
    #[prost(string, tag = "2")]
    pub name: ::prost::alloc::string::String,
    /// The type of entry
    #[prost(enumeration = "EntryType", tag = "3")]
    pub entry_type: i32,
    #[prost(message, optional, tag = "4")]
    pub created_at: ::core::option::Option<::prost_types::Timestamp>,
    #[prost(message, optional, tag = "5")]
    pub updated_at: ::core::option::Option<::prost_types::Timestamp>,
}
impl ::prost::Name for EntryDetails {
    const NAME: &'static str = "EntryDetails";
    const PACKAGE: &'static str = "rerun.catalog.v1alpha1";
    fn full_name() -> ::prost::alloc::string::String {
        "rerun.catalog.v1alpha1.EntryDetails".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/rerun.catalog.v1alpha1.EntryDetails".into()
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DatasetEntry {
    #[prost(message, optional, tag = "1")]
    pub details: ::core::option::Option<EntryDetails>,
    /// Read-only
    #[prost(message, optional, tag = "2")]
    pub dataset_handle: ::core::option::Option<super::super::common::v1alpha1::DatasetHandle>,
}
impl ::prost::Name for DatasetEntry {
    const NAME: &'static str = "DatasetEntry";
    const PACKAGE: &'static str = "rerun.catalog.v1alpha1";
    fn full_name() -> ::prost::alloc::string::String {
        "rerun.catalog.v1alpha1.DatasetEntry".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/rerun.catalog.v1alpha1.DatasetEntry".into()
    }
}
/// EntryKey is used to access an entry by either id or name.
/// All APIs that require specifying an entry should use this
/// message.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct EntryKey {
    #[prost(message, optional, tag = "1")]
    pub id: ::core::option::Option<super::super::common::v1alpha1::EntryId>,
    #[prost(string, optional, tag = "2")]
    pub name: ::core::option::Option<::prost::alloc::string::String>,
}
impl ::prost::Name for EntryKey {
    const NAME: &'static str = "EntryKey";
    const PACKAGE: &'static str = "rerun.catalog.v1alpha1";
    fn full_name() -> ::prost::alloc::string::String {
        "rerun.catalog.v1alpha1.EntryKey".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/rerun.catalog.v1alpha1.EntryKey".into()
    }
}
/// What type of entry. This has strong implication on which APIs are available for this entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum EntryType {
    /// Always reserve unspecified as default value
    Unspecified = 0,
    /// Order as TYPE, TYPE_VIEW so things stay consistent as we introduce new types.
    Dataset = 1,
    DatasetView = 2,
    Table = 3,
    TableView = 4,
}
impl EntryType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Self::Unspecified => "ENTRY_TYPE_UNSPECIFIED",
            Self::Dataset => "ENTRY_TYPE_DATASET",
            Self::DatasetView => "ENTRY_TYPE_DATASET_VIEW",
            Self::Table => "ENTRY_TYPE_TABLE",
            Self::TableView => "ENTRY_TYPE_TABLE_VIEW",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "ENTRY_TYPE_UNSPECIFIED" => Some(Self::Unspecified),
            "ENTRY_TYPE_DATASET" => Some(Self::Dataset),
            "ENTRY_TYPE_DATASET_VIEW" => Some(Self::DatasetView),
            "ENTRY_TYPE_TABLE" => Some(Self::Table),
            "ENTRY_TYPE_TABLE_VIEW" => Some(Self::TableView),
            _ => None,
        }
    }
}
/// Generated client implementations.
pub mod catalog_service_client {
    #![allow(
        unused_variables,
        dead_code,
        missing_docs,
        clippy::wildcard_imports,
        clippy::let_unit_value
    )]
    use tonic::codegen::http::Uri;
    use tonic::codegen::*;
    #[derive(Debug, Clone)]
    pub struct CatalogServiceClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl<T> CatalogServiceClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::Error: Into<StdError>,
        T::ResponseBody: Body<Data = Bytes> + std::marker::Send + 'static,
        <T::ResponseBody as Body>::Error: Into<StdError> + std::marker::Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }
        pub fn with_origin(inner: T, origin: Uri) -> Self {
            let inner = tonic::client::Grpc::with_origin(inner, origin);
            Self { inner }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> CatalogServiceClient<InterceptedService<T, F>>
        where
            F: tonic::service::Interceptor,
            T::ResponseBody: Default,
            T: tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
                Response = http::Response<
                    <T as tonic::client::GrpcService<tonic::body::BoxBody>>::ResponseBody,
                >,
            >,
            <T as tonic::codegen::Service<http::Request<tonic::body::BoxBody>>>::Error:
                Into<StdError> + std::marker::Send + std::marker::Sync,
        {
            CatalogServiceClient::new(InterceptedService::new(inner, interceptor))
        }
        /// Compress requests with the given encoding.
        ///
        /// This requires the server to support it otherwise it might respond with an
        /// error.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.send_compressed(encoding);
            self
        }
        /// Enable decompressing responses.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.accept_compressed(encoding);
            self
        }
        /// Limits the maximum size of a decoded message.
        ///
        /// Default: `4MB`
        #[must_use]
        pub fn max_decoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_decoding_message_size(limit);
            self
        }
        /// Limits the maximum size of an encoded message.
        ///
        /// Default: `usize::MAX`
        #[must_use]
        pub fn max_encoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_encoding_message_size(limit);
            self
        }
        pub async fn find_entries(
            &mut self,
            request: impl tonic::IntoRequest<super::FindEntriesRequest>,
        ) -> std::result::Result<tonic::Response<super::FindEntriesResponse>, tonic::Status>
        {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::unknown(format!("Service was not ready: {}", e.into()))
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/rerun.catalog.v1alpha1.CatalogService/FindEntries",
            );
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new(
                "rerun.catalog.v1alpha1.CatalogService",
                "FindEntries",
            ));
            self.inner.unary(req, path, codec).await
        }
        pub async fn create_dataset_entry(
            &mut self,
            request: impl tonic::IntoRequest<super::CreateDatasetEntryRequest>,
        ) -> std::result::Result<tonic::Response<super::CreateDatasetEntryResponse>, tonic::Status>
        {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::unknown(format!("Service was not ready: {}", e.into()))
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/rerun.catalog.v1alpha1.CatalogService/CreateDatasetEntry",
            );
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new(
                "rerun.catalog.v1alpha1.CatalogService",
                "CreateDatasetEntry",
            ));
            self.inner.unary(req, path, codec).await
        }
        pub async fn read_dataset_entry(
            &mut self,
            request: impl tonic::IntoRequest<super::ReadDatasetEntryRequest>,
        ) -> std::result::Result<tonic::Response<super::ReadDatasetEntryResponse>, tonic::Status>
        {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::unknown(format!("Service was not ready: {}", e.into()))
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/rerun.catalog.v1alpha1.CatalogService/ReadDatasetEntry",
            );
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new(
                "rerun.catalog.v1alpha1.CatalogService",
                "ReadDatasetEntry",
            ));
            self.inner.unary(req, path, codec).await
        }
        pub async fn delete_dataset_entry(
            &mut self,
            request: impl tonic::IntoRequest<super::DeleteDatasetEntryRequest>,
        ) -> std::result::Result<tonic::Response<super::DeleteDatasetEntryResponse>, tonic::Status>
        {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::unknown(format!("Service was not ready: {}", e.into()))
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/rerun.catalog.v1alpha1.CatalogService/DeleteDatasetEntry",
            );
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new(
                "rerun.catalog.v1alpha1.CatalogService",
                "DeleteDatasetEntry",
            ));
            self.inner.unary(req, path, codec).await
        }
    }
}
/// Generated server implementations.
pub mod catalog_service_server {
    #![allow(
        unused_variables,
        dead_code,
        missing_docs,
        clippy::wildcard_imports,
        clippy::let_unit_value
    )]
    use tonic::codegen::*;
    /// Generated trait containing gRPC methods that should be implemented for use with CatalogServiceServer.
    #[async_trait]
    pub trait CatalogService: std::marker::Send + std::marker::Sync + 'static {
        async fn find_entries(
            &self,
            request: tonic::Request<super::FindEntriesRequest>,
        ) -> std::result::Result<tonic::Response<super::FindEntriesResponse>, tonic::Status>;
        async fn create_dataset_entry(
            &self,
            request: tonic::Request<super::CreateDatasetEntryRequest>,
        ) -> std::result::Result<tonic::Response<super::CreateDatasetEntryResponse>, tonic::Status>;
        async fn read_dataset_entry(
            &self,
            request: tonic::Request<super::ReadDatasetEntryRequest>,
        ) -> std::result::Result<tonic::Response<super::ReadDatasetEntryResponse>, tonic::Status>;
        async fn delete_dataset_entry(
            &self,
            request: tonic::Request<super::DeleteDatasetEntryRequest>,
        ) -> std::result::Result<tonic::Response<super::DeleteDatasetEntryResponse>, tonic::Status>;
    }
    #[derive(Debug)]
    pub struct CatalogServiceServer<T> {
        inner: Arc<T>,
        accept_compression_encodings: EnabledCompressionEncodings,
        send_compression_encodings: EnabledCompressionEncodings,
        max_decoding_message_size: Option<usize>,
        max_encoding_message_size: Option<usize>,
    }
    impl<T> CatalogServiceServer<T> {
        pub fn new(inner: T) -> Self {
            Self::from_arc(Arc::new(inner))
        }
        pub fn from_arc(inner: Arc<T>) -> Self {
            Self {
                inner,
                accept_compression_encodings: Default::default(),
                send_compression_encodings: Default::default(),
                max_decoding_message_size: None,
                max_encoding_message_size: None,
            }
        }
        pub fn with_interceptor<F>(inner: T, interceptor: F) -> InterceptedService<Self, F>
        where
            F: tonic::service::Interceptor,
        {
            InterceptedService::new(Self::new(inner), interceptor)
        }
        /// Enable decompressing requests with the given encoding.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.accept_compression_encodings.enable(encoding);
            self
        }
        /// Compress responses with the given encoding, if the client supports it.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.send_compression_encodings.enable(encoding);
            self
        }
        /// Limits the maximum size of a decoded message.
        ///
        /// Default: `4MB`
        #[must_use]
        pub fn max_decoding_message_size(mut self, limit: usize) -> Self {
            self.max_decoding_message_size = Some(limit);
            self
        }
        /// Limits the maximum size of an encoded message.
        ///
        /// Default: `usize::MAX`
        #[must_use]
        pub fn max_encoding_message_size(mut self, limit: usize) -> Self {
            self.max_encoding_message_size = Some(limit);
            self
        }
    }
    impl<T, B> tonic::codegen::Service<http::Request<B>> for CatalogServiceServer<T>
    where
        T: CatalogService,
        B: Body + std::marker::Send + 'static,
        B::Error: Into<StdError> + std::marker::Send + 'static,
    {
        type Response = http::Response<tonic::body::BoxBody>;
        type Error = std::convert::Infallible;
        type Future = BoxFuture<Self::Response, Self::Error>;
        fn poll_ready(
            &mut self,
            _cx: &mut Context<'_>,
        ) -> Poll<std::result::Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, req: http::Request<B>) -> Self::Future {
            match req.uri().path() {
                "/rerun.catalog.v1alpha1.CatalogService/FindEntries" => {
                    #[allow(non_camel_case_types)]
                    struct FindEntriesSvc<T: CatalogService>(pub Arc<T>);
                    impl<T: CatalogService> tonic::server::UnaryService<super::FindEntriesRequest>
                        for FindEntriesSvc<T>
                    {
                        type Response = super::FindEntriesResponse;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::FindEntriesRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as CatalogService>::find_entries(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let method = FindEntriesSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/rerun.catalog.v1alpha1.CatalogService/CreateDatasetEntry" => {
                    #[allow(non_camel_case_types)]
                    struct CreateDatasetEntrySvc<T: CatalogService>(pub Arc<T>);
                    impl<T: CatalogService>
                        tonic::server::UnaryService<super::CreateDatasetEntryRequest>
                        for CreateDatasetEntrySvc<T>
                    {
                        type Response = super::CreateDatasetEntryResponse;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::CreateDatasetEntryRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as CatalogService>::create_dataset_entry(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let method = CreateDatasetEntrySvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/rerun.catalog.v1alpha1.CatalogService/ReadDatasetEntry" => {
                    #[allow(non_camel_case_types)]
                    struct ReadDatasetEntrySvc<T: CatalogService>(pub Arc<T>);
                    impl<T: CatalogService>
                        tonic::server::UnaryService<super::ReadDatasetEntryRequest>
                        for ReadDatasetEntrySvc<T>
                    {
                        type Response = super::ReadDatasetEntryResponse;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ReadDatasetEntryRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as CatalogService>::read_dataset_entry(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let method = ReadDatasetEntrySvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/rerun.catalog.v1alpha1.CatalogService/DeleteDatasetEntry" => {
                    #[allow(non_camel_case_types)]
                    struct DeleteDatasetEntrySvc<T: CatalogService>(pub Arc<T>);
                    impl<T: CatalogService>
                        tonic::server::UnaryService<super::DeleteDatasetEntryRequest>
                        for DeleteDatasetEntrySvc<T>
                    {
                        type Response = super::DeleteDatasetEntryResponse;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::DeleteDatasetEntryRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as CatalogService>::delete_dataset_entry(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let method = DeleteDatasetEntrySvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                _ => Box::pin(async move {
                    let mut response = http::Response::new(empty_body());
                    let headers = response.headers_mut();
                    headers.insert(
                        tonic::Status::GRPC_STATUS,
                        (tonic::Code::Unimplemented as i32).into(),
                    );
                    headers.insert(
                        http::header::CONTENT_TYPE,
                        tonic::metadata::GRPC_CONTENT_TYPE,
                    );
                    Ok(response)
                }),
            }
        }
    }
    impl<T> Clone for CatalogServiceServer<T> {
        fn clone(&self) -> Self {
            let inner = self.inner.clone();
            Self {
                inner,
                accept_compression_encodings: self.accept_compression_encodings,
                send_compression_encodings: self.send_compression_encodings,
                max_decoding_message_size: self.max_decoding_message_size,
                max_encoding_message_size: self.max_encoding_message_size,
            }
        }
    }
    /// Generated gRPC service name
    pub const SERVICE_NAME: &str = "rerun.catalog.v1alpha1.CatalogService";
    impl<T> tonic::server::NamedService for CatalogServiceServer<T> {
        const NAME: &'static str = SERVICE_NAME;
    }
}
