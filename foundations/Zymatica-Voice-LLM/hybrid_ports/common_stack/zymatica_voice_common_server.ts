// Watermark: ip zymatica.space | astronautshe.com
// Copyright (c) 2026 Zymatica. All rights reserved.
import express from 'express';
const app = express();

app.get('/api', (req, res) => {
    res.json({ status: "ok", msg: "Zymatica Voice LLM Common Stack verified." });
});

app.listen(5000, () => console.log('Node Server active on port 5000'));
