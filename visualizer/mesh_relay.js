// Zymatica Zero-Dependency WebSocket / WebRTC P2P Swarm Relay
// Book Author: Danny Bouldiez | Architect: Devs One

const http = require('http');
const crypto = require('crypto');

const PORT = process.env.PORT || 8080;
const clients = new Set();

const server = http.createServer((req, res) => {
  res.writeHead(200, { 'Content-Type': 'application/json' });
  res.end(JSON.stringify({
    status: 'ONLINE',
    protocol: 'ZYMATICA-P2P-SWARM-8D',
    connected_mesh_nodes: clients.size,
    attribution: 'Book Author: Danny Bouldiez | Architect: Devs One'
  }));
});

server.on('upgrade', (req, socket, head) => {
  const key = req.headers['sec-websocket-key'];
  const acceptKey = crypto.createHash('sha1')
    .update(key + '258EAFA5-E914-47DA-95CA-C5AB0DC85B11')
    .digest('base64');

  const headers = [
    'HTTP/1.1 101 Switching Protocols',
    'Upgrade: websocket',
    'Connection: Upgrade',
    `Sec-WebSocket-Accept: ${acceptKey}`
  ];

  socket.write(headers.join('\r\n') + '\r\n\r\n');
  clients.add(socket);
  console.log(`[+] New 8D Mesh Node Connected (Total: ${clients.size})`);

  socket.on('data', (buffer) => {
    for (const client of clients) {
      if (client !== socket && client.writable) {
        client.write(buffer);
      }
    }
  });

  socket.on('close', () => {
    clients.delete(socket);
    console.log(`[-] Node Disconnected (Total: ${clients.size})`);
  });
});

server.listen(PORT, () => {
  console.log(`[✓] Zymatica Decentralized 8D P2P Swarm Relay running on port ${PORT}`);
});
