"""Use a blueprint to show a web page."""

import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_web_view", spawn=True)

html = '''
<!doctype html>
<html>
  <body>
    <h1>Hello from a web view!</h1>
    <p>This page is raw HTML, stored in the blueprint.</p>
    <button>Click me</button>
    <script>
      const button = document.querySelector("button");
      let count = 0;
      button.onclick = () => {
        button.textContent = `Clicked ${++count} times`;
      };
    </script>
  </body>
</html>
'''

# Create a web view that shows the page.
blueprint = rrb.Blueprint(
    rrb.WebView(name="Web page", content=rrb.WebViewContent(html=html)),
    collapse_panels=True,
)

rr.send_blueprint(blueprint)
