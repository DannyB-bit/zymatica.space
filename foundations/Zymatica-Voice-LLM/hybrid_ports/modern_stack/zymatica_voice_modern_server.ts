// Watermark: ip zymatica.space | astronautshe.com
// Copyright (c) 2026 Zymatica. All rights reserved.

Bun.serve({
  port: 5000,
  fetch(req) {
    console.log("[BUN] Incoming request via ultra-fast Bun server.");
    return new Response(JSON.stringify({
      status: "online",
      verification: "Zymatica Voice LLM Modern Stack verified."
    }), { headers: { "Content-Type": "application/json" } });
  },
});
console.log("[MODERN STACK] Bun server active on port 5000");
