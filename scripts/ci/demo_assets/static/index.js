function show_center_html(html) {
  center_text_elem = document.getElementById("center_text");
  center_text_elem.innerHTML = html;
  center_text_elem.classList.remove("hidden");
  center_text_elem.classList.add("visible");
}
function hide_center_html(html) {
  center_text_elem = document.getElementById("center_text");
  center_text_elem.innerHTML = html;
  center_text_elem.classList.remove("visible");
  center_text_elem.classList.add("hidden");
}
function show_canvas(html) {
  canvas_elem = document.getElementById("the_canvas_id");
  canvas_elem.classList.remove("hidden");
  canvas_elem.classList.add("visible");
  demo_header_elem = document.getElementById("header_bar");
  demo_header_elem.classList.add("visible");
  demo_header_elem.classList.remove("hidden");
}
function hide_canvas(html) {
  canvas_elem = document.getElementById("the_canvas_id");
  canvas_elem.classList.remove("visible");
  canvas_elem.classList.add("hidden");
  demo_header_elem = document.getElementById("header_bar");
  demo_header_elem.classList.add("hidden");
  demo_header_elem.classList.remove("visible");
}

// On mobile platforms show a warning, but provide a link to try anyways
if (
  /Android|webOS|iPhone|iPad|iPod|BlackBerry|IEMobile|Opera Mini/i.test(
    navigator.userAgent,
  )
) {
  show_center_html(`
  <p>
      Rerun is not yet supported on mobile browsers.
  </p>
  <p>
      <a href="#" id="try_anyways">Try anyways</a>
  </p>`);
  document
    .querySelector("#try_anyways")
    .addEventListener("click", function (event) {
      event.preventDefault();
      load_wasm();
    });
} else {
  load_wasm();
}

function load_wasm() {
  // We'll defer our execution until the wasm is ready to go.
  // Here we tell bindgen the path to the wasm file so it can start
  // initialization and return to us a promise when it's done.

  document.getElementById("center_text").innerHTML = `
  <p class="strong">
      Loading Application Bundle…
  </p>
  <p class="subdued" id="status">
  </p>`;

  const status_element = document.getElementById("status");
  function progress({ loaded, total_bytes }) {
    if (total_bytes != null) {
      status_element.innerHTML =
        Math.round(Math.min((loaded / total_bytes) * 100, 100)) + "%";
    } else {
      status_element.innerHTML = (loaded / (1024 * 1024)).toFixed(1) + "MiB";
    }
  }

  var timeoutId = setTimeout(function () {
    document.getElementById("center_text").classList.remove("hidden");
    document.getElementById("center_text").classList.add("visible");
  }, 1500);

  async function wasm_with_progress() {
    const response = await fetch("./re_viewer_bg.wasm");
    // Use the uncompressed size
    var content_length;
    var content_multiplier = 1;
    // If the content is gzip encoded, try to get the uncompressed size.
    if (response.headers.get("content-encoding") == "gzip") {
      content_length = response.headers.get("x-goog-meta-uncompressed-size");

      // If the uncompressed size wasn't found 3 seems to be a very good approximation
      if (content_length == null) {
        content_length = response.headers.get("content-length");
        content_multiplier = 3;
      }
    } else {
      content_length = response.headers.get("content-length");
    }

    const total_bytes = parseInt(content_length, 10) * content_multiplier;
    let loaded = 0;

    const res = new Response(
      new ReadableStream({
        async start(controller) {
          const reader = response.body.getReader();
          for (;;) {
            const { done, value } = await reader.read();
            if (done) break;
            loaded += value.byteLength;
            progress({ loaded, total_bytes });
            controller.enqueue(value);
          }
          controller.close();
        },
      }),
      {
        status: response.status,
        statusText: response.statusText,
      },
    );

    for (const [key, value] of response.headers.entries()) {
      res.headers.set(key, value);
    }

    wasm_bindgen(res)
      .then(() => (clearTimeout(timeoutId), on_wasm_loaded()))
      .catch(on_wasm_error);
  }

  wasm_with_progress();
}

function on_wasm_loaded() {
  window.set_email = (value) => wasm_bindgen.set_email(value);

  // WebGPU version is currently only supported on browsers with WebGPU support, there is no dynamic fallback to WebGL.
  if (wasm_bindgen.is_webgpu_build() && typeof navigator.gpu === "undefined") {
    console.debug(
      "`navigator.gpu` is undefined. This indicates lack of WebGPU support.",
    );
    show_center_html(`
                  <p class="strong">
                      Missing WebGPU support.
                  </p>
                  <p class="subdued">
                      This version of Rerun requires WebGPU support which is not available in your browser.
                      Either try a different browser or use the WebGL version of Rerun.
                  </p>`);
    return;
  }

  console.debug("Wasm loaded. Starting app…");

  let handle = new wasm_bindgen.WebHandle();

  function check_for_panic() {
    if (handle.has_panicked()) {
      console.error("Rerun has crashed");

      document.getElementById("the_canvas_id").remove();

      show_center_html(`
                      <p class="strong">
                          Rerun has crashed.
                      </p>
                      <pre align="left">${handle.panic_message()}</pre>
                      <p>
                          See the console for details.
                      </p>
                      <p>
                          Reload the page to try again.
                      </p>`);
    } else {
      let delay_ms = 1000;
      setTimeout(check_for_panic, delay_ms);
    }
  }

  check_for_panic();

  let url = determine_url();
  handle.start("the_canvas_id", url).then(on_app_started).catch(on_wasm_error);
}

function on_app_started(handle) {
  // Call `handle.destroy()` to stop. Uncomment to quick result:
  // setTimeout(() => { handle.destroy(); handle.free()) }, 2000)

  console.debug("App started.");

  hide_center_html();
  show_canvas();

  if (window.location !== window.parent.location) {
    window.parent.postMessage("READY", "*");
  }
}

function determine_url() {
  const base = window.location.pathname.endsWith("/")
    ? window.location.pathname.slice(0, -1)
    : window.location.pathname;
  return base + "/data.rrd";
}

function on_wasm_error(error) {
  console.error("Failed to start: " + error);

  let render_backend_name = "WebGPU/WebGL";
  try {
    render_backend_name = wasm_bindgen.is_webgpu_build() ? "WebGPU" : "WebGL";
  } catch (e) {
    // loading the wasm probably failed.
  }

  hide_canvas();
  show_center_html(`
      <p>
          An error occurred during loading:
      </p>
      <p style="font-family:Courier New">
          ${error}
      </p>
      <p style="font-size:14px">
              Make sure you use a modern browser with ${render_backend_name} and Wasm enabled.
      </p>`);
}

// open/close dropdown
document.querySelector("#examples").addEventListener("click", () => {
  const body = document.querySelector(".dropdown-body");
  if (!body) return;
  if (body.classList.contains("visible")) {
    body.classList.remove("visible");
  } else {
    body.classList.add("visible");
  }
});

// close dropdowns by clicking outside of it
document.body.addEventListener("click", (event) => {
  const body = document.querySelector(".dropdown-body");
  if (!body) return;

  const is_dropdown = (element) =>
    element instanceof HTMLElement && element.classList.contains("dropdown");

  if (!event.composedPath().find(is_dropdown)) {
    body.classList.remove("visible");
  }
});
