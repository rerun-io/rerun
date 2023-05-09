// On mobile platforms show a warning, but provide a link to try anyways
if (/Android|webOS|iPhone|iPad|iPod|BlackBerry|IEMobile|Opera Mini/i.test(navigator.userAgent)) {
  document.querySelector("#center_text").style.visibility = "hidden";
  document.querySelector("#mobile_text").style.visibility = "visible";
  document.querySelector("#try_anyways").addEventListener("click", function (event) {
    event.preventDefault();
    document.querySelector("#center_text").style.visibility = "visible";
    document.querySelector("#mobile_text").style.visibility = "hidden";
    load_wasm();
  });
} else {
  load_wasm();
}

function load_wasm() {
  // We'll defer our execution until the wasm is ready to go.
  // Here we tell bindgen the path to the wasm file so it can start
  // initialization and return to us a promise when it's done.

  console.debug("loading wasm…");
  wasm_bindgen("./re_viewer_bg.wasm").then(on_wasm_loaded).catch(on_wasm_error);
}

function on_wasm_loaded() {
  // WebGPU version is currently only supported on browsers with WebGPU support, there is no dynamic fallback to WebGL.
  if (wasm_bindgen.is_webgpu_build() && typeof navigator.gpu === "undefined") {
    console.debug("`navigator.gpu` is undefined. This indicates lack of WebGPU support.");
    document.getElementById("center_text").innerHTML = `
      <p>
          Missing WebGPU support.
      </p>
      <p style="font-size:18px">
          This version of Rerun requires WebGPU support which is not available in your browser.
          Either try a different browser or use the WebGL version of Rerun.
      </p>`;
    return;
  }

  console.debug("Wasm loaded. Starting app…");

  // This call installs a bunch of callbacks and then returns:
  let handle = new wasm_bindgen.WebHandle("the_canvas_id", determine_url());
  handle.then(on_app_started).catch(on_wasm_error);
}

function on_app_started(handle) {
  // Call `handle.destroy()` to stop. Uncomment to quick result:
  // setTimeout(() => { handle.destroy(); handle.free()) }, 2000)

  console.debug("App started.");
  document.getElementById("center_text").innerHTML = "";

  if (window.location !== window.parent.location) {
    window.parent.postMessage("READY", "*");
  }

  function check_for_panic() {
    if (handle.has_panicked()) {
      console.error("Rerun has crashed");

      // Rerun already logs the panic message and callstack, but you
      // can access them like this if you want to show them in the html:
      // console.error(`${handle.panic_message()}`);
      // console.error(`${handle.panic_callstack()}`);

      document.getElementById("the_canvas_id").remove();
      document.getElementById("center_text").innerHTML = `
        <p>
            Rerun has crashed.
        </p>
        <p style="font-size:14px">
            See the console for details.
        </p>
        <p style="font-size:14px">
            Reload the page to try again.
        </p>`;
    } else {
      let delay_ms = 1000;
      setTimeout(check_for_panic, delay_ms);
    }
  }

  check_for_panic();
}

function determine_url() {
  // If a 'url' is provided as a url-param, use it.
  // Although `web.rs` can also parse the url-param itself,
  // it won't do so if we pass in a non-null url. We could
  // arguably return null here instead and achieve the same
  // behavior, but as long as we've queried it anyways, we
  // may as well just pass it in for consistency.

  const url_params = new URLSearchParams(window.location.search);

  let url = url_params.get("url");

  if (url) {
    return url;
  }

  // Otherwise, look up an rrd in the data path.

  // The expected data path is the current pathname relocated to inside of "/data" with the
  // index.html stripped off if it's present.
  // exa: 'https://app.rerun.io/version/v4.0.0/index.html' -> '/data/version/v4.0.0/'
  let data_path = "/data/" + window.location.pathname.replace(/index\.html$/, "") + "/";

  const rrd_file = url_params.get("file") || "colmap_fiat.rrd";

  // Normalize the extra slashes from the url
  return (data_path + rrd_file).replace(/\/{2,}/g, "/");
}

function on_wasm_error(error) {
  console.error("Failed to start: " + error);

  let render_backend_name = "WebGPU/WebGL";
  try {
    render_backend_name = wasm_bindgen.is_webgpu_build() ? "WebGPU" : "WebGL";
  } catch (e) {
    // loading the wasm probably failed.
  }

  document.getElementById("center_text").innerHTML = `
    <p>
        An error occurred during loading:
    </p>
    <p style="font-family:Courier New">
        ${error}
    </p>
    <p style="font-size:14px">
            Make sure you use a modern browser with ${render_backend_name} and Wasm enabled.
    </p>`;
}

