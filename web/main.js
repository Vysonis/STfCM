// Basic globe setup
const globeEl = document.getElementById('globe');
const GlobeObj = Globe()
  (globeEl)
  .globeImageUrl('https://unpkg.com/three-globe/example/img/earth-blue-marble.jpg')
  .bumpImageUrl('https://unpkg.com/three-globe/example/img/earth-topology.png')
  .backgroundColor('#121212')
  .showAtmosphere(true)
  .atmosphereColor('#2a2a2a')
  .atmosphereAltitude(0.15)
  .pointAltitude(0.01)
  .pointColor(() => '#cccccc')
  .pointsData([])
  .pointLabel(d => `<div style="padding:4px 6px;border:1px solid #202020;border-radius:6px;background:#161616;color:#e5e5e5">${d.name || 'Unknown'}<br/>NORAD ${d.norad_id}<br/>(${d.lat.toFixed(2)}, ${d.lng.toFixed(2)})</div>`)
  .onPointClick(d => showSatInfo(d));

// Convert earth texture to grayscale without affecting satellites/atmosphere
function setGrayscaleEarth(src = 'https://unpkg.com/three-globe/example/img/earth-blue-marble.jpg') {
  const img = new Image();
  img.crossOrigin = 'anonymous';
  img.onload = () => {
    const canvas = document.createElement('canvas');
    canvas.width = img.width; canvas.height = img.height;
    const ctx = canvas.getContext('2d');
    ctx.drawImage(img, 0, 0);
    const imageData = ctx.getImageData(0, 0, canvas.width, canvas.height);
    const d = imageData.data;
    for (let i = 0; i < d.length; i += 4) {
      const r = d[i], g = d[i+1], b = d[i+2];
      const y = 0.299*r + 0.587*g + 0.114*b;
      d[i] = d[i+1] = d[i+2] = y;
    }
    ctx.putImageData(imageData, 0, 0);
    try {
      GlobeObj.globeImageUrl(canvas.toDataURL('image/png'));
    } catch {}
  };
  img.onerror = () => {
    // Fallback: leave original texture if CORS blocks processing
  };
  img.src = src;
}
setGrayscaleEarth();

// DOM refs
const healthEl = document.getElementById('health');
const stationsListEl = document.getElementById('stations-list');
const stationSelectEl = document.getElementById('station-select');
const stationFormEl = document.getElementById('station-form');
const passesListEl = document.getElementById('passes-list');
const noradEl = document.getElementById('norad-id');
const durationEl = document.getElementById('duration');
const stepEl = document.getElementById('step');
const minEl = document.getElementById('minel');
const predictBtnEl = document.getElementById('predict-btn');

const api = axios.create({ baseURL: '' }); // relative to same origin
const navGlobeBtn = document.getElementById('nav-globe');
const navStationsBtn = document.getElementById('nav-stations');
const tabGlobe = document.getElementById('tab-globe');
const tabStations = document.getElementById('tab-stations');
const satLimitEl = document.getElementById('sat-limit');
const refreshSatsBtn = document.getElementById('refresh-sats');
const satInfoEl = document.getElementById('sat-info');
const satFilterEl = document.getElementById('sat-filter');
const earthUrlEl = document.getElementById('earth-url');
const applyEarthBtn = document.getElementById('apply-earth');
const footerSummaryEl = document.getElementById('footer-summary');
const autoRefreshEl = document.getElementById('auto-refresh');
const refreshIntervalEl = document.getElementById('refresh-interval');
let autoRefreshHandle = null;
// Globe header summary
const globeSummaryEl = document.getElementById('globe-summary');

async function refreshHealth() {
  try {
    const { data } = await api.get('/health');
    healthEl.textContent = `OK · elements=${data.elements} · db=${data.db ? 'ok' : 'err'}`;
  } catch (e) {
    healthEl.textContent = 'Error querying /health';
  }
}

async function refreshSatellites() {
  try {
    const limit = parseInt(satLimitEl.value || '500', 10);
    const { data } = await api.get('/satellites/positions', { params: { limit } });
    let pts = data.map(s => ({
      norad_id: s.norad_id,
      name: s.name,
      lat: s.lat,
      lng: s.lon,
      alt_km: s.alt_km,
      speed_km_s: s.speed_km_s,
      epoch: s.epoch
    }));
    const filter = (satFilterEl?.value || '').trim().toLowerCase();
    if (filter) {
      pts = pts.filter(p => (p.name || '').toLowerCase().includes(filter));
    }
    GlobeObj.pointsData(pts);
    satInfoEl.innerHTML = `<div class="info">Loaded ${pts.length} satellites.</div>`;
  } catch (e) {
    satInfoEl.innerHTML = '<div class="info">Error loading satellite positions.</div>';
  }
}

function showSatInfo(d) {
  const name = d.name || 'Unknown';
  const parts = [
    `<strong>${name}</strong>`,
    `NORAD ${d.norad_id}`,
    `Lat ${d.lat.toFixed(4)}`,
    `Lon ${d.lng.toFixed(4)}`
  ];
  if (typeof d.alt_km === 'number') parts.push(`Alt ${d.alt_km.toFixed(1)} km`);
  if (typeof d.speed_km_s === 'number') parts.push(`Speed ${d.speed_km_s.toFixed(3)} km/s`);
  if (d.epoch) {
    try { parts.push(`Epoch ${new Date(d.epoch).toLocaleString()}`); } catch {}
  }
  globeSummaryEl && (globeSummaryEl.innerHTML = parts.join(' · '));
  footerSummaryEl && (footerSummaryEl.innerHTML = parts.join(' · '));
}

async function refreshStations() {
  try {
    const { data } = await api.get('/stations');
    // Update list
    stationsListEl.innerHTML = data.length === 0
      ? '<div class="info">No stations yet. Add one above.</div>'
      : data.map(s => `
        <div class="row" style="align-items:center;grid-template-columns:1fr auto auto;gap:8px">
          <div>#${s.id} · ${s.name ?? '—'} · (${s.lat.toFixed(4)}, ${s.lon.toFixed(4)})</div>
          <button class="edit-station" data-id="${s.id}">Edit</button>
          <button class="delete-station" data-id="${s.id}">Delete</button>
        </div>
      `).join('');

    // Update select
    stationSelectEl.innerHTML = '<option value="">— choose station —</option>' +
      data.map(s => `<option value="${s.id}">${s.name ?? `Station ${s.id}`}</option>`).join('');

    // Keep globe for satellites; do not override satellite markers with station positions.
  } catch (e) {
    stationsListEl.innerHTML = '<div class="info">Error loading stations</div>';
  }
}

stationFormEl.addEventListener('submit', async (ev) => {
  ev.preventDefault();
  const name = document.getElementById('station-name').value || null;
  const lat = parseFloat(document.getElementById('station-lat').value);
  const lon = parseFloat(document.getElementById('station-lon').value);
  try {
    await api.post('/stations', { name, lat, lon });
    document.getElementById('station-name').value = '';
    await refreshStations();
  } catch (e) {
    alert('Failed to add station. Check lat/lon ranges.');
  }
});

predictBtnEl.addEventListener('click', async () => {
  const stationId = stationSelectEl.value;
  const noradId = parseInt(noradEl.value || '25544', 10);
  const duration = parseInt(durationEl.value || '120', 10);
  const step = parseInt(stepEl.value || '15', 10);
  const min_elevation = parseFloat(minEl.value || '10');

  if (!stationId) {
    alert('Choose a station first.');
    return;
  }

  try {
    const { data } = await api.get(`/satellites/${noradId}/passes`, {
      params: { station_id: stationId, duration, step, min_el: min_elevation }
    });
    if (!Array.isArray(data) || data.length === 0) {
      passesListEl.innerHTML = '<div class="info">No passes in window.</div>';
    } else {
      passesListEl.innerHTML = data.map(p => {
        const start = new Date(p.start).toLocaleString();
        const end = new Date(p.end).toLocaleString();
        return `<div>Start: ${start}<br/>End: ${end}<br/>Max Elev: ${p.max_elevation_deg.toFixed(1)}°</div>`;
      }).join('');
    }
  } catch (e) {
    passesListEl.innerHTML = '<div class="info">Error predicting passes.</div>';
  }
});

// Initial load
refreshHealth();
refreshStations();
refreshSatellites();

// Tabs
navGlobeBtn.addEventListener('click', () => {
  navGlobeBtn.classList.add('active');
  navStationsBtn.classList.remove('active');
  tabGlobe.classList.remove('hidden');
  tabStations.classList.add('hidden');
});
navStationsBtn.addEventListener('click', () => {
  navStationsBtn.classList.add('active');
  navGlobeBtn.classList.remove('active');
  tabStations.classList.remove('hidden');
  tabGlobe.classList.add('hidden');
});

refreshSatsBtn.addEventListener('click', refreshSatellites);
satFilterEl && satFilterEl.addEventListener('input', refreshSatellites);

function startAutoRefresh() {
  const secs = Math.max(5, parseInt(refreshIntervalEl.value || '15', 10));
  if (autoRefreshHandle) clearInterval(autoRefreshHandle);
  autoRefreshHandle = setInterval(refreshSatellites, secs * 1000);
  satInfoEl.innerHTML = `<div class="info">Auto-refresh every ${secs}s.</div>`;
}

function stopAutoRefresh() {
  if (autoRefreshHandle) {
    clearInterval(autoRefreshHandle);
    autoRefreshHandle = null;
    satInfoEl.innerHTML = `<div class="info">Auto-refresh paused.</div>`;
  }
}

autoRefreshEl && autoRefreshEl.addEventListener('change', () => {
  if (autoRefreshEl.checked) startAutoRefresh(); else stopAutoRefresh();
});
refreshIntervalEl && refreshIntervalEl.addEventListener('change', () => {
  if (autoRefreshEl.checked) startAutoRefresh();
});

window.addEventListener('beforeunload', () => {
  if (autoRefreshHandle) clearInterval(autoRefreshHandle);
});

// Apply custom earth image
applyEarthBtn && applyEarthBtn.addEventListener('click', () => {
  const url = (earthUrlEl?.value || '').trim();
  if (url) setGrayscaleEarth(url);
});

// Handle station deletion via delegation
stationsListEl.addEventListener('click', async (ev) => {
  const btn = ev.target.closest('.delete-station');
  if (!btn) return;
  const id = parseInt(btn.dataset.id, 10);
  if (!Number.isFinite(id)) return;
  const ok = confirm(`Delete station #${id}?`);
  if (!ok) return;
  try {
    await api.delete(`/stations/${id}`);
    await refreshStations();
  } catch (e) {
    alert('Failed to delete station.');
  }
});