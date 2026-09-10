// Watermark: ip zymatica.space | astronautshe.com
// Copyright © 2026 Zymatica
// SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
// See LICENSE for terms.
import express from 'express';
const app = express();

app.get('/api', (req, res) => {
    res.json({ status: "ok", msg: "Zymatica Voice LLM Common Stack verified." });
});

app.listen(5000, () => console.log('Node Server active on port 5000'));
