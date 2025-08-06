import { script_dir } from "./common.mjs";
import { createServer } from "node:http";
import { createReadStream, promises as fsPromises } from "node:fs";
import { extname, join, normalize, resolve } from "node:path";
import { URL } from "node:url";

// Map file extensions to MIME types
const mimeTypes = {
  ".html": "text/html",
  ".css": "text/css",
  ".js": "text/javascript",
  ".json": "application/json",
  ".png": "image/png",
  ".jpg": "image/jpeg",
  ".jpeg": "image/jpeg",
  ".gif": "image/gif",
  ".svg": "image/svg+xml",
  ".txt": "text/plain"
};

// Root directory to serve (default: current working directory)
const root = resolve(script_dir, "../docs");

const port = process.env.PORT || 8001;

const server = createServer(async (req, res) => {
  try {
    // Parse URL and get pathname
    const requestUrl = new URL(req.url, `http://${req.headers.host}`);
    let pathname = decodeURIComponent(requestUrl.pathname);

    // Prevent directory traversal
    const safePath = normalize(pathname).replace(/^\.\.(\/|$)/, "");
    let filePath = join(root, safePath);

    // Check if path exists
    let stat;
    try {
      stat = await fsPromises.stat(filePath);
    } catch (err) {
      res.writeHead(404, { "Content-Type": "text/plain" });
      res.end("404 Not Found\n");
      return;
    }

    // If directory, serve index.html
    if (stat.isDirectory()) {
      filePath = join(filePath, "index.html");
      try {
        stat = await fsPromises.stat(filePath);
      } catch (err) {
        res.writeHead(403, { "Content-Type": "text/plain" });
        res.end("403 Forbidden\n");
        return;
      }
    }

    // Determine content type
    const ext = extname(filePath).toLowerCase();
    const contentType = mimeTypes[ext] || "application/octet-stream";

    // Stream file
    res.writeHead(200, { "Content-Type": contentType });
    const stream = createReadStream(filePath);
    stream.pipe(res);
    stream.on("error", (streamErr) => {
      console.error(streamErr);
      res.writeHead(500, { "Content-Type": "text/plain" });
      res.end("500 Internal Server Error\n");
    });
  } catch (err) {
    console.error(err);
    res.writeHead(500, { "Content-Type": "text/plain" });
    res.end("500 Internal Server Error\n");
  }
});

server.listen(port, () => {
  console.log(`Serving rerun_js/docs at http://localhost:${port}/`);
});
