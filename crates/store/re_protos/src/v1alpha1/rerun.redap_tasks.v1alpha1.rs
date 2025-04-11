// This file is @generated by prost-build.
/// A task is a unit of work that can be submitted to the system
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Task {
    /// Unique identifier for the task
    #[prost(message, optional, tag = "1")]
    pub id: ::core::option::Option<super::super::common::v1alpha1::TaskId>,
    /// Type of the task
    #[prost(string, tag = "2")]
    pub task_type: ::prost::alloc::string::String,
    /// Task-type dependant data necessary to de-serialize the task
    #[prost(bytes = "vec", tag = "3")]
    pub task_data: ::prost::alloc::vec::Vec<u8>,
}
impl ::prost::Name for Task {
    const NAME: &'static str = "Task";
    const PACKAGE: &'static str = "rerun.redap_tasks.v1alpha1";
    fn full_name() -> ::prost::alloc::string::String {
        "rerun.redap_tasks.v1alpha1.Task".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/rerun.redap_tasks.v1alpha1.Task".into()
    }
}
/// `SubmitTasksRequest` is the request message for submitting tasks
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SubmitTasksRequest {
    #[prost(message, repeated, tag = "1")]
    pub tasks: ::prost::alloc::vec::Vec<Task>,
}
impl ::prost::Name for SubmitTasksRequest {
    const NAME: &'static str = "SubmitTasksRequest";
    const PACKAGE: &'static str = "rerun.redap_tasks.v1alpha1";
    fn full_name() -> ::prost::alloc::string::String {
        "rerun.redap_tasks.v1alpha1.SubmitTasksRequest".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/rerun.redap_tasks.v1alpha1.SubmitTasksRequest".into()
    }
}
/// `SubmitTaskResponse` contains, for each submitted task
/// its submission outcome, encoded as a `RecordBatch`
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SubmitTasksResponse {
    #[prost(message, optional, tag = "1")]
    pub data: ::core::option::Option<super::super::common::v1alpha1::DataframePart>,
}
impl ::prost::Name for SubmitTasksResponse {
    const NAME: &'static str = "SubmitTasksResponse";
    const PACKAGE: &'static str = "rerun.redap_tasks.v1alpha1";
    fn full_name() -> ::prost::alloc::string::String {
        "rerun.redap_tasks.v1alpha1.SubmitTasksResponse".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/rerun.redap_tasks.v1alpha1.SubmitTasksResponse".into()
    }
}
/// `QueryRequest` is the request message for querying tasks status
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct QueryRequest {
    /// Empty queries for all tasks if the server allows it.
    #[prost(message, repeated, tag = "1")]
    pub ids: ::prost::alloc::vec::Vec<super::super::common::v1alpha1::TaskId>,
}
impl ::prost::Name for QueryRequest {
    const NAME: &'static str = "QueryRequest";
    const PACKAGE: &'static str = "rerun.redap_tasks.v1alpha1";
    fn full_name() -> ::prost::alloc::string::String {
        "rerun.redap_tasks.v1alpha1.QueryRequest".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/rerun.redap_tasks.v1alpha1.QueryRequest".into()
    }
}
/// `QueryResponse` is the response message for querying tasks status
/// encoded as a record batch
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct QueryResponse {
    #[prost(message, optional, tag = "1")]
    pub data: ::core::option::Option<super::super::common::v1alpha1::DataframePart>,
}
impl ::prost::Name for QueryResponse {
    const NAME: &'static str = "QueryResponse";
    const PACKAGE: &'static str = "rerun.redap_tasks.v1alpha1";
    fn full_name() -> ::prost::alloc::string::String {
        "rerun.redap_tasks.v1alpha1.QueryResponse".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/rerun.redap_tasks.v1alpha1.QueryResponse".into()
    }
}
/// `QueryOnCompletionRequest` is the request message for querying tasks status.
/// This is close-to-a-copy of `QueryRequest`, with the addition of a timeout.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct QueryOnCompletionRequest {
    /// Empty queries for all tasks if the server allows it.
    #[prost(message, repeated, tag = "1")]
    pub ids: ::prost::alloc::vec::Vec<super::super::common::v1alpha1::TaskId>,
    /// Time limit for the server to wait for task completion.
    /// The actual maximum time may be arbitrarily capped by the server.
    #[prost(message, optional, tag = "2")]
    pub timeout: ::core::option::Option<::prost_types::Duration>,
}
impl ::prost::Name for QueryOnCompletionRequest {
    const NAME: &'static str = "QueryOnCompletionRequest";
    const PACKAGE: &'static str = "rerun.redap_tasks.v1alpha1";
    fn full_name() -> ::prost::alloc::string::String {
        "rerun.redap_tasks.v1alpha1.QueryOnCompletionRequest".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/rerun.redap_tasks.v1alpha1.QueryOnCompletionRequest".into()
    }
}
/// `QueryOnCompletionResponse` is the response message for querying tasks status
/// encoded as a record batch. This is a copy of `QueryResponse`.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct QueryOnCompletionResponse {
    #[prost(message, optional, tag = "1")]
    pub data: ::core::option::Option<super::super::common::v1alpha1::DataframePart>,
}
impl ::prost::Name for QueryOnCompletionResponse {
    const NAME: &'static str = "QueryOnCompletionResponse";
    const PACKAGE: &'static str = "rerun.redap_tasks.v1alpha1";
    fn full_name() -> ::prost::alloc::string::String {
        "rerun.redap_tasks.v1alpha1.QueryOnCompletionResponse".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/rerun.redap_tasks.v1alpha1.QueryOnCompletionResponse".into()
    }
}
/// `FetchOutputRequest` is the request message for fetching task output
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FetchOutputRequest {
    /// Unique identifier for the task
    #[prost(message, optional, tag = "1")]
    pub id: ::core::option::Option<super::super::common::v1alpha1::TaskId>,
}
impl ::prost::Name for FetchOutputRequest {
    const NAME: &'static str = "FetchOutputRequest";
    const PACKAGE: &'static str = "rerun.redap_tasks.v1alpha1";
    fn full_name() -> ::prost::alloc::string::String {
        "rerun.redap_tasks.v1alpha1.FetchOutputRequest".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/rerun.redap_tasks.v1alpha1.FetchOutputRequest".into()
    }
}
/// / `FetchOutputResponse` is the response message for fetching task output
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FetchOutputResponse {
    /// The output of the task, encoded as a record batch
    #[prost(message, optional, tag = "1")]
    pub data: ::core::option::Option<super::super::common::v1alpha1::DataframePart>,
}
impl ::prost::Name for FetchOutputResponse {
    const NAME: &'static str = "FetchOutputResponse";
    const PACKAGE: &'static str = "rerun.redap_tasks.v1alpha1";
    fn full_name() -> ::prost::alloc::string::String {
        "rerun.redap_tasks.v1alpha1.FetchOutputResponse".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/rerun.redap_tasks.v1alpha1.FetchOutputResponse".into()
    }
}
/// Generated client implementations.
pub mod tasks_service_client {
    #![allow(
        unused_variables,
        dead_code,
        missing_docs,
        clippy::wildcard_imports,
        clippy::let_unit_value
    )]
    use tonic::codegen::http::Uri;
    use tonic::codegen::*;
    /// `TasksService` is the service for submitting and querying persistent redap tasks.
    #[derive(Debug, Clone)]
    pub struct TasksServiceClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl<T> TasksServiceClient<T>
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
        ) -> TasksServiceClient<InterceptedService<T, F>>
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
            TasksServiceClient::new(InterceptedService::new(inner, interceptor))
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
        /// Submit new tasks
        pub async fn submit_tasks(
            &mut self,
            request: impl tonic::IntoRequest<super::SubmitTasksRequest>,
        ) -> std::result::Result<tonic::Response<super::SubmitTasksResponse>, tonic::Status>
        {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::unknown(format!("Service was not ready: {}", e.into()))
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/rerun.redap_tasks.v1alpha1.TasksService/SubmitTasks",
            );
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new(
                "rerun.redap_tasks.v1alpha1.TasksService",
                "SubmitTasks",
            ));
            self.inner.unary(req, path, codec).await
        }
        /// Query the status of submitted tasks
        pub async fn query(
            &mut self,
            request: impl tonic::IntoRequest<super::QueryRequest>,
        ) -> std::result::Result<tonic::Response<super::QueryResponse>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::unknown(format!("Service was not ready: {}", e.into()))
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/rerun.redap_tasks.v1alpha1.TasksService/Query",
            );
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new(
                "rerun.redap_tasks.v1alpha1.TasksService",
                "Query",
            ));
            self.inner.unary(req, path, codec).await
        }
        /// Fetch the output of a completed task
        pub async fn fetch_output(
            &mut self,
            request: impl tonic::IntoRequest<super::FetchOutputRequest>,
        ) -> std::result::Result<tonic::Response<super::FetchOutputResponse>, tonic::Status>
        {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::unknown(format!("Service was not ready: {}", e.into()))
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/rerun.redap_tasks.v1alpha1.TasksService/FetchOutput",
            );
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new(
                "rerun.redap_tasks.v1alpha1.TasksService",
                "FetchOutput",
            ));
            self.inner.unary(req, path, codec).await
        }
        /// Query the status of submitted tasks, waiting for their completion.
        ///
        /// The method returns a stream of QueryResult. Each item in the stream contains
        /// the status of a subset of the tasks, as they complete.
        /// The server does not guarantee to immediately send one stream item as soon as a task
        /// completes, but may decide to arbitrarily aggregate results into larger batches.
        pub async fn query_on_completion(
            &mut self,
            request: impl tonic::IntoRequest<super::QueryOnCompletionRequest>,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::QueryOnCompletionResponse>>,
            tonic::Status,
        > {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::unknown(format!("Service was not ready: {}", e.into()))
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/rerun.redap_tasks.v1alpha1.TasksService/QueryOnCompletion",
            );
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new(
                "rerun.redap_tasks.v1alpha1.TasksService",
                "QueryOnCompletion",
            ));
            self.inner.server_streaming(req, path, codec).await
        }
    }
}
/// Generated server implementations.
pub mod tasks_service_server {
    #![allow(
        unused_variables,
        dead_code,
        missing_docs,
        clippy::wildcard_imports,
        clippy::let_unit_value
    )]
    use tonic::codegen::*;
    /// Generated trait containing gRPC methods that should be implemented for use with TasksServiceServer.
    #[async_trait]
    pub trait TasksService: std::marker::Send + std::marker::Sync + 'static {
        /// Submit new tasks
        async fn submit_tasks(
            &self,
            request: tonic::Request<super::SubmitTasksRequest>,
        ) -> std::result::Result<tonic::Response<super::SubmitTasksResponse>, tonic::Status>;
        /// Query the status of submitted tasks
        async fn query(
            &self,
            request: tonic::Request<super::QueryRequest>,
        ) -> std::result::Result<tonic::Response<super::QueryResponse>, tonic::Status>;
        /// Fetch the output of a completed task
        async fn fetch_output(
            &self,
            request: tonic::Request<super::FetchOutputRequest>,
        ) -> std::result::Result<tonic::Response<super::FetchOutputResponse>, tonic::Status>;
        /// Server streaming response type for the QueryOnCompletion method.
        type QueryOnCompletionStream: tonic::codegen::tokio_stream::Stream<
                Item = std::result::Result<super::QueryOnCompletionResponse, tonic::Status>,
            > + std::marker::Send
            + 'static;
        /// Query the status of submitted tasks, waiting for their completion.
        ///
        /// The method returns a stream of QueryResult. Each item in the stream contains
        /// the status of a subset of the tasks, as they complete.
        /// The server does not guarantee to immediately send one stream item as soon as a task
        /// completes, but may decide to arbitrarily aggregate results into larger batches.
        async fn query_on_completion(
            &self,
            request: tonic::Request<super::QueryOnCompletionRequest>,
        ) -> std::result::Result<tonic::Response<Self::QueryOnCompletionStream>, tonic::Status>;
    }
    /// `TasksService` is the service for submitting and querying persistent redap tasks.
    #[derive(Debug)]
    pub struct TasksServiceServer<T> {
        inner: Arc<T>,
        accept_compression_encodings: EnabledCompressionEncodings,
        send_compression_encodings: EnabledCompressionEncodings,
        max_decoding_message_size: Option<usize>,
        max_encoding_message_size: Option<usize>,
    }
    impl<T> TasksServiceServer<T> {
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
    impl<T, B> tonic::codegen::Service<http::Request<B>> for TasksServiceServer<T>
    where
        T: TasksService,
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
                "/rerun.redap_tasks.v1alpha1.TasksService/SubmitTasks" => {
                    #[allow(non_camel_case_types)]
                    struct SubmitTasksSvc<T: TasksService>(pub Arc<T>);
                    impl<T: TasksService> tonic::server::UnaryService<super::SubmitTasksRequest> for SubmitTasksSvc<T> {
                        type Response = super::SubmitTasksResponse;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::SubmitTasksRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as TasksService>::submit_tasks(&inner, request).await
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
                        let method = SubmitTasksSvc(inner);
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
                "/rerun.redap_tasks.v1alpha1.TasksService/Query" => {
                    #[allow(non_camel_case_types)]
                    struct QuerySvc<T: TasksService>(pub Arc<T>);
                    impl<T: TasksService> tonic::server::UnaryService<super::QueryRequest> for QuerySvc<T> {
                        type Response = super::QueryResponse;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::QueryRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut =
                                async move { <T as TasksService>::query(&inner, request).await };
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
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/rerun.redap_tasks.v1alpha1.TasksService/FetchOutput" => {
                    #[allow(non_camel_case_types)]
                    struct FetchOutputSvc<T: TasksService>(pub Arc<T>);
                    impl<T: TasksService> tonic::server::UnaryService<super::FetchOutputRequest> for FetchOutputSvc<T> {
                        type Response = super::FetchOutputResponse;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::FetchOutputRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as TasksService>::fetch_output(&inner, request).await
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
                        let method = FetchOutputSvc(inner);
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
                "/rerun.redap_tasks.v1alpha1.TasksService/QueryOnCompletion" => {
                    #[allow(non_camel_case_types)]
                    struct QueryOnCompletionSvc<T: TasksService>(pub Arc<T>);
                    impl<T: TasksService>
                        tonic::server::ServerStreamingService<super::QueryOnCompletionRequest>
                        for QueryOnCompletionSvc<T>
                    {
                        type Response = super::QueryOnCompletionResponse;
                        type ResponseStream = T::QueryOnCompletionStream;
                        type Future =
                            BoxFuture<tonic::Response<Self::ResponseStream>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::QueryOnCompletionRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as TasksService>::query_on_completion(&inner, request).await
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
                        let method = QueryOnCompletionSvc(inner);
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
    impl<T> Clone for TasksServiceServer<T> {
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
    pub const SERVICE_NAME: &str = "rerun.redap_tasks.v1alpha1.TasksService";
    impl<T> tonic::server::NamedService for TasksServiceServer<T> {
        const NAME: &'static str = SERVICE_NAME;
    }
}
