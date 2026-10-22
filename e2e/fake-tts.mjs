// A stand-in speech engine for the e2e suite: the one route the server calls
// (`POST /v1/audio/speech`), answered with about a second of SILENT MP3 per unit, plus `/health`.
//
// Silence is generated, not committed. An MPEG-1 Layer III frame whose side info and main data are
// all zero decodes as silence, so a run of bare headers is a valid, playable MP3 — which is all the
// suite needs to prove a listen streams and the browser plays it. The suite never asserts on what
// was said; the real engine is measured separately.
//
// Usage: node e2e/fake-tts.mjs <port>
import { createServer } from "node:http";

const port = Number(process.argv[2] ?? 8392);

// 128 kbit/s, 44.1 kHz, stereo, no CRC, no padding: 144 * 128000 / 44100 = 417 bytes per frame.
const FRAME_BYTES = 417;
const FRAMES = 40; // 40 × 1152 samples ≈ 1.04 s
const frame = Buffer.alloc(FRAME_BYTES);
frame.set([0xff, 0xfb, 0x90, 0x64]);
const silence = Buffer.concat(Array.from({ length: FRAMES }, () => frame));

let units = 0;
createServer((req, res) => {
  if (req.method === "GET" && req.url === "/health") {
    res.writeHead(200, { "content-type": "application/json" });
    res.end(JSON.stringify({ units }));
    return;
  }
  if (req.method === "POST" && req.url === "/v1/audio/speech") {
    let body = "";
    req.on("data", (chunk) => (body += chunk));
    req.on("end", () => {
      const { input = "", voice = "" } = JSON.parse(body || "{}");
      units += 1;
      console.log(`fake-tts: unit ${units} (${voice}, ${input.length} chars)`);
      res.writeHead(200, { "content-type": "audio/mpeg", "content-length": silence.length });
      res.end(silence);
    });
    return;
  }
  res.writeHead(404).end();
}).listen(port, "127.0.0.1", () => console.log(`fake-tts: listening on :${port}`));
