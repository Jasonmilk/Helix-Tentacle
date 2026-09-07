#!/usr/bin/env node
// calc: safe arithmetic evaluation (whitelist-validated, deterministic).
// Only [0-9 + - * / ( ) . % ^ whitespace] may reach python3 — no letters,
// underscores, quotes or semicolons, so no import/exec/attribute access.
const fs = require('fs');
const { execFileSync } = require('child_process');
// args arrive via argv: [node, calc.js, tool_name, params_json, server_config]
let input = {};
try { input = JSON.parse(process.argv[3] || '{}'); } catch (_) { input = {}; }
const expr = String(input.expression || input.expr || '').trim();
if (!/^[0-9+\-*/().%^ \t]+$/.test(expr) || expr.length > 200) {
  console.log(JSON.stringify({ ok: false, error: 'expression not allowed' }));
  process.exit(0);
}
const py = expr.replace(/\^/g, '**');
try {
  const out = execFileSync('python3', ['-c', 'print(' + py + ')'], { timeout: 5000 }).toString().trim();
  console.log(JSON.stringify({ ok: true, result: out }));
} catch (e) {
  console.log(JSON.stringify({ ok: false, error: String(e.message || e) }));
}
