// Settings → "Time and timezone".
//
// A list of 400+ zones is too hard to use, so the choice is split into
// small native controls: Automatic or Choose, then a Region, then a City
// near you (each city shows its current offset). A live line shows how
// times will look.

import { h, clear, button, zoneOffsetLabel, systemTimeZone } from './core.js';

const REGIONS = [
  { key: 'Asia', label: 'Asia', prefixes: ['Asia/'] },
  { key: 'Europe', label: 'Europe', prefixes: ['Europe/'] },
  { key: 'America', label: 'The Americas', prefixes: ['America/'] },
  { key: 'Africa', label: 'Africa', prefixes: ['Africa/'] },
  { key: 'Pacific', label: 'Australia and the Pacific', prefixes: ['Australia/', 'Pacific/'] },
  { key: 'Other', label: 'Atlantic, Indian Ocean, and Antarctica', prefixes: ['Atlantic/', 'Indian/', 'Antarctica/', 'Arctic/'] },
  { key: 'UTC', label: 'UTC (no local time)', prefixes: [] },
];

function allZones() {
  let zones = [];
  try { zones = Intl.supportedValuesOf('timeZone'); } catch { zones = []; }
  return zones.length ? zones : [systemTimeZone()];
}

function regionOf(zone) {
  if (zone === 'UTC' || zone === 'Etc/UTC') return 'UTC';
  return REGIONS.find((r) => r.prefixes.some((p) => zone.startsWith(p)))?.key || null;
}

/** "America/Argentina/Buenos_Aires" → "Buenos Aires, Argentina". */
function cityLabel(zone) {
  if (zone === 'UTC') return 'UTC';
  const parts = zone.split('/').slice(1).map((p) => p.replaceAll('_', ' '));
  return parts.length > 1 ? `${parts[parts.length - 1]}, ${parts.slice(0, -1).join(', ')}` : parts[0] || zone;
}

function sample(zone) {
  const opts = {
    year: 'numeric', month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit',
    timeZoneName: 'shortOffset',
  };
  try {
    return new Intl.DateTimeFormat(undefined, { ...opts, timeZone: zone }).format(new Date());
  } catch {
    return '';
  }
}

/**
 * The settings section. `save(body, message)` stores settings and resolves
 * true on success.
 */
export function timezoneSection(cfg, save) {
  const system = systemTimeZone();
  const zones = allZones();
  const current = cfg.timezone || null;
  let mode = current ? 'choose' : 'auto';

  const regionSelect = h('select', { id: 'tz-region' });
  const citySelect = h('select', { id: 'tz-city' });
  const chooser = h('div', { class: 'tz-chooser' },
    h('div', { class: 'field' }, h('label', { for: 'tz-region' }, 'Region'), regionSelect),
    h('div', { class: 'field' }, h('label', { for: 'tz-city' }, 'City near you'), citySelect));
  const preview = h('p', { class: 'tz-preview', role: 'status' });

  for (const r of REGIONS) regionSelect.append(h('option', { value: r.key }, r.label));

  function fillCities(region, selected) {
    clear(citySelect);
    const list = region === 'UTC'
      ? ['UTC']
      : zones.filter((z) => regionOf(z) === region)
        .sort((a, b) => cityLabel(a).localeCompare(cityLabel(b)));
    for (const z of list) {
      const offset = zoneOffsetLabel(z);
      citySelect.append(h('option', { value: z, selected: z === selected }, `${cityLabel(z)} (${offset})`));
    }
  }

  const chosenZone = () => (mode === 'auto' ? system : citySelect.value || system);

  function refresh() {
    chooser.hidden = mode !== 'choose';
    const zone = chosenZone();
    clear(preview).append('Times will show like this: ', h('strong', null, sample(zone)),
      mode === 'auto' ? ' (the time on this computer now).' : ` (the time in ${cityLabel(zone)} now).`);
  }

  const startZone = current || system;
  regionSelect.value = regionOf(startZone) || 'UTC';
  fillCities(regionSelect.value, startZone);
  regionSelect.addEventListener('change', () => { fillCities(regionSelect.value, null); refresh(); });
  citySelect.addEventListener('change', refresh);

  const radio = (value, label, text) => h('label', { class: 'choice' },
    h('input', {
      type: 'radio', name: 'tz-mode', value, checked: value === mode,
      onchange: () => { mode = value; refresh(); },
    }),
    h('span', null, h('strong', null, label), h('span', null, text)));

  const saveBtn = button('Save timezone', { type: 'submit', kind: 'primary' });
  const form = h('form', {
    onsubmit: async (e) => {
      e.preventDefault();
      const zone = mode === 'auto' ? '' : citySelect.value;
      await save({ timezone: zone }, 'Timezone saved. Times now show in this timezone.');
    },
  },
  h('fieldset', { class: 'choices' },
    h('legend', null, 'Show times in'),
    radio('auto', 'Automatic: use this computer’s timezone', `${cityLabel(system)} (${system}, ${zoneOffsetLabel(system)})`),
    radio('choose', 'Choose a timezone', 'Useful if this computer’s clock is set to a different place.')),
  chooser,
  preview,
  h('div', { class: 'actions' }, saveBtn));

  refresh();
  return h('section', { class: 'settings-section', 'aria-labelledby': 'set-time' },
    h('h2', { id: 'set-time' }, 'Time and timezone'),
    h('p', { class: 'help' }, 'Cairn saves every time in one standard form (UTC) and shows it in your timezone, so people in different places always see the right time. Times are marked with their difference from GMT, for example “GMT+8”.'),
    form);
}
