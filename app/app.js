const element = (id) => document.getElementById(id);
const set = (id, text) => { element(id).textContent = text; };
let previousCount = -1;
let receivedAt = null;
let chartLoadedCount = -1;
let chartLoading = false;
let previousRun = null;

function updateChart(count) {
  if (chartLoading || chartLoadedCount === count) return;
  chartLoading = true;
  const run = previousRun;
  const next = new Image();
  next.onload = () => {
    if (run !== previousRun) return;
    element('curve').src = next.src;
    element('curve').hidden = false;
    element('empty').hidden = true;
    chartLoadedCount = count;
    chartLoading = false;
  };
  next.onerror = () => { if (run === previousRun) chartLoading = false; };
  next.src = `/curve.svg?reading=${count}&t=${Date.now()}`;
}

async function refresh() {
  try {
    const response = await fetch('/api/status', {cache: 'no-store', signal: AbortSignal.timeout(4000)});
    if (!response.ok) throw new Error(`Server returned ${response.status}`);
    const data = await response.json();
    if (previousRun !== data.run_id) {
      previousRun = data.run_id;
      previousCount = -1;
      chartLoadedCount = -1;
      chartLoading = false;
      element('curve').hidden = true;
      element('empty').hidden = false;
      for (const id of ['dissolved', 'elapsed', 'transmission', 'absorbance', 'concentration', 'mass', 'reference', 'sample', 'dark']) set(id, '—');
      set('flags', 'Quality flags will appear here.');
      set('freshness', 'No readings received');
    }
    set('status', {waiting: 'Waiting for sensor', running: 'Receiving readings', complete: 'Experiment complete', error: 'Acquisition stopped'}[data.status] || data.status);
    element('status').dataset.state = data.status;
    set('source', data.source === 'SIMULATED DATA'
      ? 'Simulated sensor data · Invented optical calibration and dissolution behavior.'
      : 'External sensor stream · Estimates use the calibration supplied for this run.');
    set('notice', data.source === 'SIMULATED DATA' ? 'Synthetic data, not measured drug behavior.' : 'Inspect calibration and quality flags before interpreting results.');
    element('error').hidden = !data.error;
    set('error', data.error ? `${data.error} — The curve shows only the last successfully saved readings.` : '');
    set('count', `${data.count} reading${data.count === 1 ? '' : 's'}`);
    set('calibration', `${data.config.pill_mass_mg} mg active ingredient · ${data.config.vessel_volume_ml} mL vessel`);
    element('downloads').hidden = data.count === 0;
    if (data.latest) {
      const p = data.latest;
      if (data.count !== previousCount) receivedAt = Date.now();
      set('dissolved', p.dissolved_percent.toFixed(2));
      set('elapsed', `${(p.time_s / 60).toFixed(2)} min`);
      set('transmission', `${p.transmission_percent.toFixed(2)}%`);
      set('absorbance', `${p.absorbance_au.toFixed(4)} AU`);
      set('concentration', `${p.concentration_mg_ml.toFixed(4)} mg/mL`);
      set('mass', `${p.dissolved_mass_mg.toFixed(2)} mg`);
      for (const key of ['reference', 'sample', 'dark']) set(key, p[`${key}_intensity`].toFixed(2));
      set('flags', `${data.flagged} flagged reading(s). ${p.quality_flags ? `Latest: ${p.quality_flags.replaceAll('_', ' ')}` : 'Latest reading has no quality flags.'}`);
      set('freshness', data.status === 'running' ? `Last update seen ${Math.floor((Date.now() - receivedAt) / 1000)}s ago` : 'Last saved reading');
      updateChart(data.count);
    }
    previousCount = data.count;
  } catch (error) {
    set('status', 'Disconnected');
    element('status').dataset.state = 'offline';
    element('error').hidden = false;
    set('error', `Cannot reach the local sensor server. Retrying automatically. ${error.message}`);
  } finally {
    setTimeout(refresh, 500);
  }
}
refresh();
