#!/usr/bin/env node
// web_search: deterministic top-N web search via DuckDuckGo HTML endpoint.
// Params arrive via argv: [node, web_search.js, tool, params_json, cfg].
// Safety: fixed endpoint + URL-encoded query only; results are extracted
// from the HTML with a whitelist regex, never executed.
const { execFileSync } = require('child_process');
let input = {};
try { input = JSON.parse(process.argv[3] || '{}'); } catch (_) { input = {}; }
const q = String(input.q || input.query || '').trim();
const max = Math.min(Number(input.max) || 5, 8);
if (!q || q.length > 200) {
  console.log(JSON.stringify({ ok: false, error: 'empty or too-long query' }));
  process.exit(0);
}
const url = 'https://www.bing.com/search?q=' + encodeURIComponent(q) + '&count=' + max;
try {
  const html = execFileSync('curl', ['-sL', '--max-time', '15', '-A', 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)', url]).toString();
  // bing html: <li class="b_algo"><h2><a href="...">Title</a></h2>
  const re = /<li class="b_algo"[^>]*>[\s\S]*?<h2[^>]*><a[^>]+href="([^"]+)"[^>]*>([\s\S]*?)<\/a>/g;
  const out = [];
  let m;
  while ((m = re.exec(html)) !== null && out.length < max) {
    const title = m[2].replace(/<[^>]+>/g, '').replace(/&amp;/g, '&').replace(/&#x27;/g, "'").trim();
    if (title) out.push({ title, url: m[1] });
  }
  if (out.length === 0) {
    console.log(JSON.stringify({ ok: true, results: [], note: 'no results parsed (endpoint shape may have changed)' }));
  } else {
    console.log(JSON.stringify({ ok: true, results: out }));
  }
} catch (e) {
  console.log(JSON.stringify({ ok: false, error: String(e.message || e) }));
}
