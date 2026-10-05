import { WebViewer } from "/web-viewer/index.js";

// Served by `custom_callback_app` through `serve_grpc`.
const RECORDING_URL = "rerun+http://127.0.0.1:9876/proxy";
const RECONNECT_DELAY_MS = 5000;

const $ = (id) => document.getElementById(id);
const num = (id) => Number($(id).value);

const viewer = new WebViewer();
await viewer.start(RECORDING_URL, $("viewer"), {
  width: "100%",
  height: "100%",
  hide_welcome_screen: true,
});

// Control channel. Each frame is one `Message` from `src/comms/protocol.rs`,
// in serde's externally tagged JSON form: `{ "Variant": { …fields } }`.
let socket = null;

function connect() {
  socket = new WebSocket(`ws://${location.host}/ws`);
  socket.onopen = () => setStatus(true);
  socket.onclose = () => {
    setStatus(false);
    setTimeout(connect, RECONNECT_DELAY_MS);
  };
}

function setStatus(connected) {
  $("status").textContent = connected ? "Connected" : "Disconnected, retrying…";
}

function send(message) {
  if (socket?.readyState === WebSocket.OPEN) {
    socket.send(JSON.stringify(message));
  }
}

connect();

// Message properties
$("kind").addEventListener("change", () => {
  for (const row of document.querySelectorAll("[data-kind]")) {
    row.hidden = row.dataset.kind !== $("kind").value;
  }
});

$("send").addEventListener("click", () => {
  const path = $("path").value;
  const position = [num("pos-x"), num("pos-y"), num("pos-z")];

  if ($("kind").value === "Point3d") {
    send({ Point3d: { path, position, radius: num("radius") } });
  } else {
    const half_size = [num("half-x"), num("half-y"), num("half-z")];
    send({ Box3d: { path, position, half_size } });
  }
});

// Dynamic position
function sendDynamic() {
  send({
    DynamicPosition: { radius: num("dyn-radius"), offset: num("offset") },
  });
}

for (const id of ["offset", "dyn-radius"]) {
  const slider = $(id);
  const output = document.querySelector(`output[for="${id}"]`);
  output.value = slider.value;
  slider.addEventListener("input", () => {
    output.value = slider.value;
    sendDynamic();
  });
}
