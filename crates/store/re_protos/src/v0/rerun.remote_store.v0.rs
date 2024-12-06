// This file is @generated by prost-build.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RegisterRecordingRequest {
    /// human readable description of the recording
    #[prost(string, tag = "1")]
    pub description: ::prost::alloc::string::String,
    /// recording storage url (e.g. s3://bucket/file or file:///path/to/file)
    #[prost(string, tag = "2")]
    pub storage_url: ::prost::alloc::string::String,
    /// type of recording
    #[prost(enumeration = "RecordingType", tag = "3")]
    pub typ: i32,
    /// (optional) any additional metadata that should be associated with the recording
    /// You can associate any arbtrirary number of columns with a specific recording
    #[prost(message, optional, tag = "4")]
    pub metadata: ::core::option::Option<RecordingMetadata>,
}
/// Recording metadata is single row arrow record batch
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RecordingMetadata {
    #[prost(enumeration = "EncoderVersion", tag = "1")]
    pub encoder_version: i32,
    #[prost(bytes = "vec", tag = "2")]
    pub payload: ::prost::alloc::vec::Vec<u8>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RegisterRecordingResponse {
    #[prost(message, optional, tag = "1")]
    pub id: ::core::option::Option<super::super::common::v0::RecordingId>,
    /// Note / TODO(zehiko): this implies we read the record (for example go through entire .rrd file
    /// chunk by chunk) and extract the metadata. So we might want to 1/ not do this i.e.
    /// only do it as part of explicit GetMetadata request or 2/ do it if Request has "include_metadata=true"
    /// or 3/ do it always
    #[prost(message, optional, tag = "2")]
    pub metadata: ::core::option::Option<RecordingMetadata>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateCatalogRequest {
    #[prost(message, optional, tag = "1")]
    pub recording_id: ::core::option::Option<super::super::common::v0::RecordingId>,
    #[prost(message, optional, tag = "2")]
    pub metadata: ::core::option::Option<RecordingMetadata>,
}
#[derive(Clone, Copy, PartialEq, ::prost::Message)]
pub struct UpdateCatalogResponse {}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct QueryRequest {
    /// unique identifier of the recording
    #[prost(message, optional, tag = "1")]
    pub recording_id: ::core::option::Option<super::super::common::v0::RecordingId>,
    /// query to execute
    #[prost(message, optional, tag = "3")]
    pub query: ::core::option::Option<super::super::common::v0::Query>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct QueryResponse {
    /// TODO(zehiko) we need to expand this to become something like 'encoder options'
    /// as we will need to specify additional options like compression, including schema
    /// in payload, etc.
    #[prost(enumeration = "EncoderVersion", tag = "1")]
    pub encoder_version: i32,
    /// payload is raw bytes that the relevant codec can interpret
    #[prost(bytes = "vec", tag = "2")]
    pub payload: ::prost::alloc::vec::Vec<u8>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct QueryCatalogRequest {
    /// Column projection - define which columns should be returned.
    /// Providing it is optional, if not provided, all columns should be returned
    #[prost(message, optional, tag = "1")]
    pub column_projection: ::core::option::Option<ColumnProjection>,
    /// Filter specific recordings that match the criteria (selection)
    #[prost(message, optional, tag = "2")]
    pub filter: ::core::option::Option<CatalogFilter>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ColumnProjection {
    #[prost(string, repeated, tag = "1")]
    pub columns: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CatalogFilter {
    /// Filtering is very simple right now, we can only select
    /// recordings by their ids.
    #[prost(message, repeated, tag = "1")]
    pub recording_ids: ::prost::alloc::vec::Vec<super::super::common::v0::RecordingId>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct QueryCatalogResponse {
    #[prost(enumeration = "EncoderVersion", tag = "1")]
    pub encoder_version: i32,
    /// raw bytes are TransportChunks (i.e. RecordBatches) encoded with the relevant codec
    #[prost(bytes = "vec", tag = "2")]
    pub payload: ::prost::alloc::vec::Vec<u8>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FetchRecordingRequest {
    #[prost(message, optional, tag = "1")]
    pub recording_id: ::core::option::Option<super::super::common::v0::RecordingId>,
}
/// TODO(jleibs): Eventually this becomes either query-mediated in some way, but for now
/// it's useful to be able to just get back the whole RRD somehow.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FetchRecordingResponse {
    /// TODO(zehiko) we need to expand this to become something like 'encoder options'
    /// as we will need to specify additional options like compression, including schema
    /// in payload, etc.
    #[prost(enumeration = "EncoderVersion", tag = "1")]
    pub encoder_version: i32,
    /// payload is raw bytes that the relevant codec can interpret
    #[prost(bytes = "vec", tag = "2")]
    pub payload: ::prost::alloc::vec::Vec<u8>,
}
/// Application level error - used as `details` in the `google.rpc.Status` message
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RemoteStoreError {
    /// error code
    #[prost(enumeration = "ErrorCode", tag = "1")]
    pub code: i32,
    /// unique identifier associated with the request (e.g. recording id, recording storage url)
    #[prost(string, tag = "2")]
    pub id: ::prost::alloc::string::String,
    /// human readable details about the error
    #[prost(string, tag = "3")]
    pub message: ::prost::alloc::string::String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum EncoderVersion {
    V0 = 0,
}
impl EncoderVersion {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Self::V0 => "V0",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "V0" => Some(Self::V0),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum RecordingType {
    Rrd = 0,
}
impl RecordingType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Self::Rrd => "RRD",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "RRD" => Some(Self::Rrd),
            _ => None,
        }
    }
}
/// Error codes for application level errors
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum ErrorCode {
    /// unused
    Unused = 0,
    /// object store access error
    ObjectStoreError = 1,
    /// metadata database access error
    MetadataDbError = 2,
    /// Encoding / decoding error
    CodecError = 3,
}
impl ErrorCode {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Self::Unused => "_UNUSED",
            Self::ObjectStoreError => "OBJECT_STORE_ERROR",
            Self::MetadataDbError => "METADATA_DB_ERROR",
            Self::CodecError => "CODEC_ERROR",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "_UNUSED" => Some(Self::Unused),
            "OBJECT_STORE_ERROR" => Some(Self::ObjectStoreError),
            "METADATA_DB_ERROR" => Some(Self::MetadataDbError),
            "CODEC_ERROR" => Some(Self::CodecError),
            _ => None,
        }
    }
}
/// Generated client implementations.
pub mod storage_node_client {
    #![allow(
        unused_variables,
        dead_code,
        missing_docs,
        clippy::wildcard_imports,
        clippy::let_unit_value,
    )]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    #[derive(Debug, Clone)]
    pub struct StorageNodeClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl<T> StorageNodeClient<T>
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
        ) -> StorageNodeClient<InterceptedService<T, F>>
        where
            F: tonic::service::Interceptor,
            T::ResponseBody: Default,
            T: tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
                Response = http::Response<
                    <T as tonic::client::GrpcService<tonic::body::BoxBody>>::ResponseBody,
                >,
            >,
            <T as tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
            >>::Error: Into<StdError> + std::marker::Send + std::marker::Sync,
        {
            StorageNodeClient::new(InterceptedService::new(inner, interceptor))
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
        /// data API calls
        pub async fn query(
            &mut self,
            request: impl tonic::IntoRequest<super::QueryRequest>,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::QueryResponse>>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::unknown(
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/rerun.remote_store.v0.StorageNode/Query",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("rerun.remote_store.v0.StorageNode", "Query"));
            self.inner.server_streaming(req, path, codec).await
        }
        pub async fn fetch_recording(
            &mut self,
            request: impl tonic::IntoRequest<super::FetchRecordingRequest>,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::FetchRecordingResponse>>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::unknown(
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/rerun.remote_store.v0.StorageNode/FetchRecording",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new(
                        "rerun.remote_store.v0.StorageNode",
                        "FetchRecording",
                    ),
                );
            self.inner.server_streaming(req, path, codec).await
        }
        /// metadata API calls
        pub async fn query_catalog(
            &mut self,
            request: impl tonic::IntoRequest<super::QueryCatalogRequest>,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::QueryCatalogResponse>>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::unknown(
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/rerun.remote_store.v0.StorageNode/QueryCatalog",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("rerun.remote_store.v0.StorageNode", "QueryCatalog"),
                );
            self.inner.server_streaming(req, path, codec).await
        }
        pub async fn update_catalog(
            &mut self,
            request: impl tonic::IntoRequest<super::UpdateCatalogRequest>,
        ) -> std::result::Result<
            tonic::Response<super::UpdateCatalogResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::unknown(
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/rerun.remote_store.v0.StorageNode/UpdateCatalog",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("rerun.remote_store.v0.StorageNode", "UpdateCatalog"),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn register_recording(
            &mut self,
            request: impl tonic::IntoRequest<super::RegisterRecordingRequest>,
        ) -> std::result::Result<
            tonic::Response<super::RegisterRecordingResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::unknown(
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/rerun.remote_store.v0.StorageNode/RegisterRecording",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new(
                        "rerun.remote_store.v0.StorageNode",
                        "RegisterRecording",
                    ),
                );
            self.inner.unary(req, path, codec).await
        }
    }
}
/// Generated server implementations.
pub mod storage_node_server {
    #![allow(
        unused_variables,
        dead_code,
        missing_docs,
        clippy::wildcard_imports,
        clippy::let_unit_value,
    )]
    use tonic::codegen::*;
    /// Generated trait containing gRPC methods that should be implemented for use with StorageNodeServer.
    #[async_trait]
    pub trait StorageNode: std::marker::Send + std::marker::Sync + 'static {
        /// Server streaming response type for the Query method.
        type QueryStream: tonic::codegen::tokio_stream::Stream<
                Item = std::result::Result<super::QueryResponse, tonic::Status>,
            >
            + std::marker::Send
            + 'static;
        /// data API calls
        async fn query(
            &self,
            request: tonic::Request<super::QueryRequest>,
        ) -> std::result::Result<tonic::Response<Self::QueryStream>, tonic::Status>;
        /// Server streaming response type for the FetchRecording method.
        type FetchRecordingStream: tonic::codegen::tokio_stream::Stream<
                Item = std::result::Result<super::FetchRecordingResponse, tonic::Status>,
            >
            + std::marker::Send
            + 'static;
        async fn fetch_recording(
            &self,
            request: tonic::Request<super::FetchRecordingRequest>,
        ) -> std::result::Result<
            tonic::Response<Self::FetchRecordingStream>,
            tonic::Status,
        >;
        /// Server streaming response type for the QueryCatalog method.
        type QueryCatalogStream: tonic::codegen::tokio_stream::Stream<
                Item = std::result::Result<super::QueryCatalogResponse, tonic::Status>,
            >
            + std::marker::Send
            + 'static;
        /// metadata API calls
        async fn query_catalog(
            &self,
            request: tonic::Request<super::QueryCatalogRequest>,
        ) -> std::result::Result<
            tonic::Response<Self::QueryCatalogStream>,
            tonic::Status,
        >;
        async fn update_catalog(
            &self,
            request: tonic::Request<super::UpdateCatalogRequest>,
        ) -> std::result::Result<
            tonic::Response<super::UpdateCatalogResponse>,
            tonic::Status,
        >;
        async fn register_recording(
            &self,
            request: tonic::Request<super::RegisterRecordingRequest>,
        ) -> std::result::Result<
            tonic::Response<super::RegisterRecordingResponse>,
            tonic::Status,
        >;
    }
    #[derive(Debug)]
    pub struct StorageNodeServer<T> {
        inner: Arc<T>,
        accept_compression_encodings: EnabledCompressionEncodings,
        send_compression_encodings: EnabledCompressionEncodings,
        max_decoding_message_size: Option<usize>,
        max_encoding_message_size: Option<usize>,
    }
    impl<T> StorageNodeServer<T> {
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
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> InterceptedService<Self, F>
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
    impl<T, B> tonic::codegen::Service<http::Request<B>> for StorageNodeServer<T>
    where
        T: StorageNode,
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
                "/rerun.remote_store.v0.StorageNode/Query" => {
                    #[allow(non_camel_case_types)]
                    struct QuerySvc<T: StorageNode>(pub Arc<T>);
                    impl<
                        T: StorageNode,
                    > tonic::server::ServerStreamingService<super::QueryRequest>
                    for QuerySvc<T> {
                        type Response = super::QueryResponse;
                        type ResponseStream = T::QueryStream;
                        type Future = BoxFuture<
                            tonic::Response<Self::ResponseStream>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::QueryRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as StorageNode>::query(&inner, request).await
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
                        let method = QuerySvc(inner);
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
                        let res = grpc.server_streaming(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/rerun.remote_store.v0.StorageNode/FetchRecording" => {
                    #[allow(non_camel_case_types)]
                    struct FetchRecordingSvc<T: StorageNode>(pub Arc<T>);
                    impl<
                        T: StorageNode,
                    > tonic::server::ServerStreamingService<super::FetchRecordingRequest>
                    for FetchRecordingSvc<T> {
                        type Response = super::FetchRecordingResponse;
                        type ResponseStream = T::FetchRecordingStream;
                        type Future = BoxFuture<
                            tonic::Response<Self::ResponseStream>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::FetchRecordingRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as StorageNode>::fetch_recording(&inner, request).await
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
                        let method = FetchRecordingSvc(inner);
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
                        let res = grpc.server_streaming(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/rerun.remote_store.v0.StorageNode/QueryCatalog" => {
                    #[allow(non_camel_case_types)]
                    struct QueryCatalogSvc<T: StorageNode>(pub Arc<T>);
                    impl<
                        T: StorageNode,
                    > tonic::server::ServerStreamingService<super::QueryCatalogRequest>
                    for QueryCatalogSvc<T> {
                        type Response = super::QueryCatalogResponse;
                        type ResponseStream = T::QueryCatalogStream;
                        type Future = BoxFuture<
                            tonic::Response<Self::ResponseStream>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::QueryCatalogRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as StorageNode>::query_catalog(&inner, request).await
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
                        let method = QueryCatalogSvc(inner);
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
                        let res = grpc.server_streaming(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/rerun.remote_store.v0.StorageNode/UpdateCatalog" => {
                    #[allow(non_camel_case_types)]
                    struct UpdateCatalogSvc<T: StorageNode>(pub Arc<T>);
                    impl<
                        T: StorageNode,
                    > tonic::server::UnaryService<super::UpdateCatalogRequest>
                    for UpdateCatalogSvc<T> {
                        type Response = super::UpdateCatalogResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::UpdateCatalogRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as StorageNode>::update_catalog(&inner, request).await
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
                        let method = UpdateCatalogSvc(inner);
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
                "/rerun.remote_store.v0.StorageNode/RegisterRecording" => {
                    #[allow(non_camel_case_types)]
                    struct RegisterRecordingSvc<T: StorageNode>(pub Arc<T>);
                    impl<
                        T: StorageNode,
                    > tonic::server::UnaryService<super::RegisterRecordingRequest>
                    for RegisterRecordingSvc<T> {
                        type Response = super::RegisterRecordingResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::RegisterRecordingRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as StorageNode>::register_recording(&inner, request)
                                    .await
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
                        let method = RegisterRecordingSvc(inner);
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
                _ => {
                    Box::pin(async move {
                        let mut response = http::Response::new(empty_body());
                        let headers = response.headers_mut();
                        headers
                            .insert(
                                tonic::Status::GRPC_STATUS,
                                (tonic::Code::Unimplemented as i32).into(),
                            );
                        headers
                            .insert(
                                http::header::CONTENT_TYPE,
                                tonic::metadata::GRPC_CONTENT_TYPE,
                            );
                        Ok(response)
                    })
                }
            }
        }
    }
    impl<T> Clone for StorageNodeServer<T> {
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
    pub const SERVICE_NAME: &str = "rerun.remote_store.v0.StorageNode";
    impl<T> tonic::server::NamedService for StorageNodeServer<T> {
        const NAME: &'static str = SERVICE_NAME;
    }
}
