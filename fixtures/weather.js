#!/usr/bin/env node
// weather: current + today conditions via wttr.in JSON (free, zero-key,
// deterministic). Output is trimmed to the minimum the LLM needs — full
// j1 payload is huge and would waste tokens (按需获取, 极致节能).
// Safety: fixed endpoint + URL-encoded city only; whitelist-validated
// (letters, digits, CJK, spaces, comma, dot, hyphen), never executed.
const { execFileSync } = require('child_process');
let input = {};
try { input = JSON.parse(process.argv[3] || '{}'); } catch (_) { input = {}; }
const city = String(input.city || input.q || '').trim();
if (!city || city.length > 60 || !/^[A-Za-z0-9\u4e00-\u9fa5\s,.\-]+$/.test(city)) {
  console.log(JSON.stringify({ ok: false, error: 'city not allowed' }));
  process.exit(0);
}
const url = 'https://wttr.in/' + encodeURIComponent(city) + '?format=j1';
try {
  const raw = execFileSync('curl', ['-sL', '--max-time', '12', '-A', 'curl/8', url]).toString();
  const j = JSON.parse(raw);
  const cc = j.current_condition && j.current_condition[0];
  if (!cc) {
    console.log(JSON.stringify({ ok: true, note: 'no observation for city (name may be wrong)' }));
    process.exit(0);
  }
  const area = j.nearest_area && j.nearest_area[0];
  const today = j.weather && j.weather[0];
  const desc = (v) => (v && v[0] && v[0].value) || 'unknown';
  const place = area ? [area.areaName && area.areaName[0] && area.areaName[0].value, area.country && area.country[0] && area.country[0].value].filter(Boolean).join(', ') : city;
  const out = {
    ok: true,
    weather: {
      city: place,
      obs: cc.localObsDateTime || '',
      now: {
        t: cc.temp_C + 'C',
        feel: cc.FeelsLikeC + 'C',
        desc: desc(cc.weatherDesc),
        hum: cc.humidity + '%',
        wind: cc.windspeedKmph + 'km/h'
      },
      today: today ? {
        max: today.maxtempC + 'C',
        min: today.mintempC + 'C',
        desc: desc(today.hourly && today.hourly[Math.min(5, today.hourly.length - 1)].weatherDesc)
      } : null
    }
  };
  console.log(JSON.stringify(out));
} catch (e) {
  console.log(JSON.stringify({ ok: false, error: 'weather service unavailable: ' + String(e.message || e) }));
}
