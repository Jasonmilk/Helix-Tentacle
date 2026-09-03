#!/usr/bin/env node
// fixture tool: returns a numeric series (deterministic)
//
// Shape (fixture-data-shapes.md): {"series": [f64]}
// - Default: 10 ascending numbers 1..=10 (sum=55, len=10) -> all numbers
//   criteria pass (threshold [0,100], sequence_length>=3, sample_size>=2)
//   -> MET path.
// - Override: params.series (array of numbers) -> caller can force a short
//   or out-of-band series to exercise the UNMET + retry_due path.
//
// Invoked by Tentacle ProcessTool as:
//   node <exe> <tool> <params_json> "{}"
const tool = process.argv[2];
const raw = process.argv[3] || "{}";
let params = {};
try {
  params = JSON.parse(raw);
} catch (e) {
  console.error("fixture numbers: invalid params JSON:", raw);
  process.exit(1);
}

const series = Array.isArray(params.series)
  ? params.series
  : Array.from({ length: 10 }, (_, i) => i + 1);

console.log(JSON.stringify({ series }));
