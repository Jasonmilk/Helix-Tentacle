#!/usr/bin/env node
// fixture tool: returns a numerator/denominator pair (deterministic)
//
// Shape (fixture-data-shapes.md): {"numerator": f64, "denominator": f64}
// - Default: {10, 10} (ratio 1.0) -> all rate criteria pass
//   (threshold [0,100], ratio_band>=0.5, cross_check |10-10|=0) -> MET path.
//   Mirrors the M1 inline mock shape {"numerator":10.0,"denominator":10.0}.
// - Override: params.numerator / params.denominator -> caller can force any
//   ratio to exercise the UNMET + retry_due path.
//
// Invoked by Tentacle ProcessTool as:
//   node <exe> <tool> <params_json> "{}"
const tool = process.argv[2];
const raw = process.argv[3] || "{}";
let params = {};
try {
  params = JSON.parse(raw);
} catch (e) {
  console.error("fixture rate: invalid params JSON:", raw);
  process.exit(1);
}

const numerator = typeof params.numerator === "number" ? params.numerator : 10;
const denominator = typeof params.denominator === "number" ? params.denominator : 10;

console.log(JSON.stringify({ numerator, denominator }));
