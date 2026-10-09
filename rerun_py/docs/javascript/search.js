// TODO(squidfunk/mkdocs-material#8610): Remove this script and its mkdocs.yml entry
// after upgrading to a fixed release.
// https://github.com/squidfunk/mkdocs-material/issues/8610
(() => {
  const query = document.querySelector('[data-md-component="search-query"]');
  if (!query) return;

  // Material for MkDocs observes keyup events to refresh the search query.
  const refresh = () => query.dispatchEvent(new Event("keyup"));
  query.addEventListener("input", refresh);

  query.form.addEventListener("reset", () => {
    // The browser clears the input after dispatching the reset event.
    setTimeout(refresh, 0);
  });
})();
