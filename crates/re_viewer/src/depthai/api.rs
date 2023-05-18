use super::depthai;
use super::ws::{BackWsMessage as WsMessage, WebSocket, WsMessageData, WsMessageType};

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ApiError {
    pub detail: String,
}

impl Default for ApiError {
    fn default() -> Self {
        Self {
            detail: "ApiError".to_string(),
        }
    }
}

#[derive(Default)]
pub struct BackendCommChannel {
    pub ws: WebSocket,
}

impl BackendCommChannel {
    pub fn shutdown(&mut self) {
        self.ws.shutdown();
    }

    pub fn set_subscriptions(&mut self, subscriptions: &Vec<depthai::ChannelId>) {
        self.ws.send(
            serde_json::to_string(
                &(WsMessage {
                    kind: WsMessageType::Subscriptions,
                    data: WsMessageData::Subscriptions(subscriptions.clone()),
                }),
            )
            .unwrap(),
        );
    }

    pub fn set_pipeline(&mut self, config: &depthai::DeviceConfig, runtime_only: bool) {
        self.ws.send(
            serde_json::to_string(
                &(WsMessage {
                    kind: WsMessageType::Pipeline,
                    data: WsMessageData::Pipeline((config.clone(), runtime_only)),
                }),
            )
            .unwrap(),
        );
    }

    pub fn receive(&mut self) -> Option<WsMessage> {
        self.ws.receive()
    }

    pub fn get_devices(&mut self) {
        self.ws.send(
            serde_json::to_string(
                &(WsMessage {
                    kind: WsMessageType::Devices,
                    data: WsMessageData::Devices(Vec::new()),
                }),
            )
            .unwrap(),
        );
    }

    pub fn set_device(&mut self, device_id: depthai::DeviceId) {
        self.ws.send(
            serde_json::to_string(
                &(WsMessage {
                    kind: WsMessageType::Device,
                    data: WsMessageData::Device(depthai::Device {
                        id: device_id,
                        ..Default::default()
                    }),
                }),
            )
            .unwrap(),
        );
    }
}
