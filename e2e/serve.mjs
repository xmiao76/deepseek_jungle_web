// Static file server for the published Blazor bundle, applying the same headers that
// Cloudflare Pages applies via wwwroot/_headers. Used for local verification and Playwright
// E2E: without COOP/COEP the page is not cross-origin isolated and .NET WASM threading
// (the background-thread AI search) cannot start.
import http from 'node:http';
import { readFile, stat } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = process.env.WWWROOT
    ?? path.join(__dirname, '..', 'src', 'JungleGame.Web', 'bin', 'Release', 'net8.0', 'publish', 'wwwroot');
const port = Number(process.env.PORT ?? 8123);

const MIME = {
    '.html': 'text/html; charset=utf-8',
    '.js': 'text/javascript; charset=utf-8',
    '.mjs': 'text/javascript; charset=utf-8',
    '.json': 'application/json',
    '.wasm': 'application/wasm',
    '.css': 'text/css; charset=utf-8',
    '.png': 'image/png',
    '.svg': 'image/svg+xml',
    '.ico': 'image/x-icon',
    '.pdb': 'application/octet-stream',
    '.dat': 'application/octet-stream',
};

const server = http.createServer(async (req, res) => {
    try {
        const urlPath = decodeURIComponent(new URL(req.url, 'http://localhost').pathname);
        const rel = urlPath === '/' ? 'index.html' : urlPath.replace(/^\/+/, '');
        const filePath = path.normalize(path.join(root, rel));
        if (!filePath.startsWith(path.normalize(root))) {
            res.writeHead(403).end();
            return;
        }

        const headers = {
            // Required for SharedArrayBuffer / .NET WASM threading:
            'Cross-Origin-Opener-Policy': 'same-origin',
            'Cross-Origin-Embedder-Policy': 'require-corp',
        };

        const info = await stat(filePath).catch(() => null);
        if (!info || !info.isFile()) {
            res.writeHead(404, headers).end('not found');
            return;
        }

        // Serve the pre-compressed variant when the client accepts it and publish produced
        // one (mirrors Cloudflare's content negotiation; keeps E2E downloads small).
        let servedPath = filePath;
        const acceptEncoding = String(req.headers['accept-encoding'] ?? '');
        for (const encoding of ['br', 'gz']) {
            if (acceptEncoding.includes(encoding)) {
                const compressed = await stat(`${filePath}.${encoding}`).catch(() => null);
                if (compressed?.isFile()) {
                    servedPath = `${filePath}.${encoding}`;
                    headers['Content-Encoding'] = encoding;
                    headers['Vary'] = 'Accept-Encoding';
                    break;
                }
            }
        }

        const ext = path.extname(filePath).toLowerCase();
        headers['Content-Type'] = MIME[ext] ?? 'application/octet-stream';
        if (rel === 'index.html' || rel.endsWith('blazor.boot.json')) {
            headers['Cache-Control'] = 'no-cache, no-store, must-revalidate';
        }

        const body = await readFile(servedPath);
        res.writeHead(200, headers).end(body);
    } catch (err) {
        console.error(err);
        res.writeHead(500).end('server error');
    }
});

server.listen(port, '127.0.0.1', () => {
    console.log(`Serving ${root} at http://127.0.0.1:${port} (COOP/COEP enabled)`);
});
