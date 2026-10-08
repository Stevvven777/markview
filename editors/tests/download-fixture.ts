import { createServer } from "node:http";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";

export async function downloadFixture(directory: string, font: string) {
  const bytes = await readFile(font);
  let mode = "invalid",
    cancellation: AbortController | undefined;
  const server = createServer((request, response) => {
    response.setHeader("Connection", "close");
    response.setHeader("Content-Length", bytes.length);
    if (request.method === "HEAD") return response.end();
    if (mode === "slow") {
      response.write(bytes.subarray(0, 10));
      cancellation!.abort();
    } else response.end(mode === "valid" ? bytes : Buffer.alloc(bytes.length));
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const proxyKeys = [
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "http_proxy",
    "https_proxy",
    "all_proxy",
    "NO_PROXY",
    "no_proxy",
  ];
  const previous = Object.fromEntries(
    proxyKeys.map((key) => [key, process.env[key]]),
  );
  const port = (server.address() as { port: number }).port;
  // The local proxy serves every byte; the public URL exercises native URL validation.
  for (const key of proxyKeys)
    process.env[key] =
      key.toLowerCase() === "no_proxy" ? "" : `http://127.0.0.1:${port}`;
  const template = path.join(directory, "download.mvss.toml");
  await writeFile(
    template,
    `format_version=2\nversion=1\ntargets=['pdf','ui']\n[[fontdef]]\nid='serif'\nlookfor=['Noto Serif']\n[[font-family]]\nid='fixture'\nlookfor=['Noto Serif']\nlicense='OFL-1.1'\n[[font-family.source]]\nfiles=['http://1.1.1.1/fixture.otf']\n`,
  );
  return {
    template,
    bytes,
    serve(next: string, controller?: AbortController) {
      mode = next;
      cancellation = controller;
    },
    async dispose() {
      for (const key of proxyKeys) {
        if (previous[key] === undefined) delete process.env[key];
        else process.env[key] = previous[key];
      }
      server.closeAllConnections();
      await new Promise<void>((resolve) => server.close(() => resolve()));
    },
  };
}
