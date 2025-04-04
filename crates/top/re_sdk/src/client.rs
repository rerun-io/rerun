// TODO: fix import
use re_chunk::external::arrow;
use re_grpc_client::message_proxy::TableClient;
use re_log_types::{TableId, TableMsg};

pub struct RerunClient {
    client: TableClient,
}

impl RerunClient {
    pub fn new(endpoint: re_uri::ProxyEndpoint) -> Self {
        Self {
            client: TableClient::new(endpoint),
        }
    }

    pub fn send_table(&self, id: impl Into<String>, dataframe: arrow::array::RecordBatch) {
        self.client.send_msg(TableMsg {
            id: TableId::from(id.into()),
            data: dataframe,
        });
    }
}
