(async function () {
  function template(urls) {
    return [
      `<div style="border:1px solid red;padding: 0 0.5em">`,
      `<span>Failed to fetch the following assets:</span>`,
      `<ul style="margin:0">`,
      ...urls.map((url) => `<li>${url}</li>`) /* NOLINT */,
      `</ul>`,
      `<div>`,
      `<span style="display:block">Please ensure they are accessible from your browser.</span>`,
      `<span style="display:block">To control this behavior, set the RERUN_NOTEBOOK_ASSET environment variable.</span>`,
      `<span style="display:block">Consult <a style="color:blue;text-decoration:underline" href="https://pypi.org/project/rerun-notebook/">https://pypi.org/project/rerun-notebook/</a> for more details.</span>`,
      `</div>`,
      `</div>`,
    ].join("");
  }

  async function url_exists(url) {
    try {
      const res = await fetch(url, { method: "HEAD" });
      return res.status >= 200 && res.status < 300;
    } catch (e) {
      console.debug(e);
      return false;
    }
  }

  function set_url_filename(url, filename) {
    url = new window.URL(url);
    const parts = url.pathname.split("/");
    parts[parts.length - 1] = filename;
    url.pathname = parts.join("/");
    return url;
  }

  async function check_if_widgets_exist() {
    console.log("check if widgets exists");

    const widget_url = new window.URL("{{widget_url}}");

    let urls = [
      widget_url,
      set_url_filename(widget_url, "re_viewer_bg.wasm"),
    ].map(async (url) => [url, await url_exists(url)]);

    let bad_urls = (await Promise.all(urls))
      .filter(([_, exists]) => !exists)
      .map(([url, _]) => url);

    if (bad_urls.length > 0) {
      const container = document.getElementById("{{widget_id}}");
      container.innerHTML = template(bad_urls);
    }
  }

  return check_if_widgets_exist();
})();
