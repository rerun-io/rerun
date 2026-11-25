use std::sync::Arc;

use re_protos::capabilities::ServerCapabilities;
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::{
    RerunCloudService, RerunCloudServiceServer,
};
use re_protos::protocol_level::ProtocolLevel;

use super::RedapClient;

type RedapHttpRequest = tonic::codegen::http::Request<tonic::body::Body>;
type RedapHttpResponse = tonic::codegen::http::Response<tonic::body::Body>;

pub type BoxedRedapClientStack =
    tower::util::BoxCloneSyncService<RedapHttpRequest, RedapHttpResponse, tonic::Status>;

/// Short-lived boxed-transport [`RedapClient`] for issuing RPCs.
///
/// Application code should acquire this from [`crate::ConnectionHandle::client`].
pub type ConnectionClient = RedapClient<BoxedRedapClientStack>;

/// An established connection to a catalog origin.
#[derive(Clone, Debug, re_byte_size::SizeBytes)]
pub struct Connection {
    pub client: ConnectionClient,
    pub analytics: Option<crate::ConnectionAnalyticsExporter>,

    /// What the server advertised on `/WhoAmI` when the connection was made.
    pub(crate) capabilities: ServerCapabilities,

    /// The protocol level of the server, as of when the connection was made.
    pub(crate) server_protocol_level: ProtocolLevel,
}

impl Connection {
    /// Create a connection backed by an in-process Rerun catalog implementation.
    ///
    /// No `/WhoAmI` call is made, so the handler's capabilities and protocol level are passed in
    /// directly.
    pub fn from_service<T>(
        origin: re_uri::Origin,
        handler: Arc<T>,
        capabilities: ServerCapabilities,
        server_protocol_level: ProtocolLevel,
    ) -> Self
    where
        T: RerunCloudService,
    {
        let service = <RerunCloudServiceServer<T> as tower::ServiceExt<RedapHttpRequest>>::map_err(
            RerunCloudServiceServer::from_arc(handler)
                .max_decoding_message_size(crate::MAX_DECODING_MESSAGE_SIZE),
            |err: std::convert::Infallible| match err {},
        );
        let client = crate::grpc::boxed_redap_grpc_client(service);

        Self {
            client: RedapClient::new(origin, client, None),
            analytics: None,
            capabilities,
            server_protocol_level,
        }
    }

    /// The server we are connected to
    pub fn origin(&self) -> &re_uri::Origin {
        self.client.origin()
    }

    /// What this server implements and supports, for any caller.
    ///
    /// This says nothing about what the caller is allowed to do.
    pub fn capabilities(&self) -> &ServerCapabilities {
        &self.capabilities
    }

    /// The protocol level this server implements.
    ///
    /// [`ProtocolLevel::BASELINE`] for a server that does not advertise one.
    pub fn server_protocol_level(&self) -> ProtocolLevel {
        self.server_protocol_level
    }
}
