// Browser conformance host: loads the exact release component and the fixture,
// drives a full v2 match with the independent JS referee and shows both boards.
import * as referee from './referee.mjs';

const $ = (id) => document.getElementById(id);
const els = {
  meta: $('meta'),
  banner: $('banner'),
  budget: $('budget'),
  seat: $('seat'),
  run: $('run'),
  flipLeft: $('flip-left'),
  flipRight: $('flip-right'),
  mode: $('mode'),
  leftCanvas: $('left'),
  rightCanvas: $('right'),
  leftTitle: $('left-title'),
  rightTitle: $('right-title'),
  leftSeat: $('left-seat'),
  rightSeat: $('right-seat'),
  leftFrames: $('left-frames'),
  rightFrames: $('right-frames'),
  leftRender: $('left-render'),
  rightRender: $('right-render'),
  result: $('result'),
  resultSub: $('result-sub'),
  log: $('log'),
};

const state = {
  running: false,
  manifest: null,
  modules: null,
  release: null,
  fixture: null,
  players: null,
  current: null,
  leftFlip: false,
  rightFlip: false,
  moves: [],
  lastReport: null,
};

function log(line) {
  els.log.textContent += `${line}\n`;
  els.log.scrollTop = els.log.scrollHeight;
}

function setMode(text) {
  els.mode.textContent = text;
}

async function loadManifest() {
  const response = await fetch('./manifest.json');
  if (!response.ok) throw new Error('manifest.json missing — run npm run build');
  return response.json();
}

async function loadComponent(entry) {
  const module = await import(`${entry.dir}/${entry.name}.js`);
  const coreFiles = entry.coreFiles ?? [];
  const cores = new Map();
  await Promise.all(
    coreFiles.map(async (file) => {
      const response = await fetch(`${entry.dir}/${file}`);
      if (!response.ok) throw new Error(`core wasm ${file} missing`);
      cores.set(file, await WebAssembly.compile(await response.arrayBuffer()));
    }),
  );
  const getCoreModule = (path) => {
    const key = String(path).split('/').pop();
    const mod = cores.get(key);
    if (!mod) throw new Error(`no compiled core module for ${key}`);
    return mod;
  };
  return {
    module,
    instantiate(display) {
      return module.instantiate(getCoreModule, { 'jungle:arena/display': display });
    },
  };
}

function makeDisplay(canvas, frameEl, renderEl) {
  const ctx = canvas.getContext('2d');
  const display = {
    frames: 0,
    ms: 0,
    last: 0,
    imports: {
      present(width, height, rgba) {
        display.frames += 1;
        display.last = rgba.length;
        if (canvas.width !== width) canvas.width = width;
        if (canvas.height !== height) canvas.height = height;
        const clamped = new Uint8ClampedArray(rgba.buffer, rgba.byteOffset, rgba.byteLength);
        ctx.putImageData(new ImageData(clamped, width, height), 0, 0);
      },
    },
  };
  display.frameEl = frameEl;
  display.renderEl = renderEl;
  return display;
}

function makePlayer(label, component, seat, canvas, frameEl, renderEl) {
  const display = makeDisplay(canvas, frameEl, renderEl);
  const instance = component.instantiate(display.imports);
  instance.game.init({ seat, difficulty: 'hard', seed: 20261120 });
  return { label, seat, instance, display };
}

function renderOne(player, snapshot, flipped) {
  player.display.frames = 0;
  const start = performance.now();
  player.instance.game.render(snapshot, flipped, 1000);
  player.display.ms = performance.now() - start;
  player.display.frameEl.textContent = `revision ${snapshot.rev} · ${player.display.frames} present · ${player.display.last} bytes`;
  player.display.renderEl.textContent = `${player.display.ms.toFixed(1)} ms`;
  if (player.display.frames !== 1) throw new Error(`${player.label}: render called present ${player.display.frames} times`);
}

function renderCurrent() {
  if (!state.players || !state.current) return;
  renderOne(state.players.left, referee.toWire(state.current), state.leftFlip);
  renderOne(state.players.right, referee.toWire(state.current), state.rightFlip);
}

function setSeats(releaseSeat) {
  const fixtureSeat = releaseSeat === 'red' ? 'blue' : 'red';
  els.leftTitle.textContent = 'Release';
  els.rightTitle.textContent = 'Fixture';
  els.leftSeat.innerHTML = `<span class="${releaseSeat}">${releaseSeat}</span> seat`;
  els.rightSeat.innerHTML = `<span class="${fixtureSeat}">${fixtureSeat}</span> seat`;
  state.leftFlip = releaseSeat === 'blue';
  state.rightFlip = fixtureSeat === 'blue';
}

async function runMatch() {
  if (state.running) return null;
  state.running = true;
  els.run.disabled = true;
  setMode('running');
  els.log.textContent = '';
  els.result.textContent = 'Starting…';
  els.resultSub.textContent = 'Instantiating two isolated instances from the release file and the fixture.';
  state.moves = [];

  try {
    if (!state.manifest) state.manifest = await loadManifest();
    if (!state.modules) {
      state.modules = {
        release: await loadComponent(state.manifest.release),
        fixture: await loadComponent(state.manifest.fixture),
      };
    }
    const releaseSeat = els.seat.value;
    const fixtureSeat = releaseSeat === 'red' ? 'blue' : 'red';
    setSeats(releaseSeat);

    const releasePlayer = makePlayer('Release', state.modules.release, releaseSeat, els.leftCanvas, els.leftFrames, els.leftRender);
    const fixturePlayer = makePlayer('Fixture', state.modules.fixture, fixtureSeat, els.rightCanvas, els.rightFrames, els.rightRender);
    state.players = {
      left: releasePlayer,
      right: fixturePlayer,
      bySeat: releaseSeat === 'red' ? { red: releasePlayer, blue: fixturePlayer } : { red: fixturePlayer, blue: releasePlayer },
    };
    const info = releasePlayer.instance.game.info();
    els.meta.innerHTML = `<strong>${escapeHtml(info.title)}</strong><br>v${escapeHtml(info.gameVersion)} · ${escapeHtml(info.model)} · ${escapeHtml(info.codeAgent)}${info.modelPlatform ? ` · ${escapeHtml(info.modelPlatform)}` : ''}<br><code>${escapeHtml(state.manifest.release.filename)}</code><br><code>${escapeHtml(state.manifest.release.sha256)}</code>`;
    els.banner.innerHTML = `Rules <code>${escapeHtml(info.rulesId)}</code> · digest <code>${escapeHtml(info.rulesDigest.slice(0, 24))}…</code> · imports only <code>jungle:arena/display@1.0.0</code> · budget <strong>${Number(els.budget.value).toLocaleString('en-US')}</strong> nodes`;

    state.current = referee.initial('red');
    renderCurrent();
    let plies = 0;
    let maxMoveMs = 0;
    let maxRenderMs = 0;
    while (state.current.status === 'playing' && plies < 300) {
      const legal = referee.legalMoves(state.current);
      if (legal.length === 0) throw new Error('referee found no legal moves in a playing position');
      const active = state.players.bySeat[state.current.turn];
      const snapshot = referee.toWire(state.current);
      const start = performance.now();
      const index = active.instance.game.chooseMove(snapshot, referee.toWireLegal(legal), Number(els.budget.value));
      const ms = performance.now() - start;
      maxMoveMs = Math.max(maxMoveMs, ms);
      if (!Number.isInteger(index) || index < 0 || index >= legal.length) {
        throw new Error(`${active.label} returned out-of-range index ${index} for ${legal.length} moves`);
      }
      const chosen = legal[index];
      state.moves.push({ turn: state.current.turn, index, src: chosen.src, dst: chosen.dst, ms });
      const next = referee.applyMove(state.current, chosen.src, chosen.dst);
      if (!next) throw new Error('referee rejected the selected move');
      state.current = next;
      plies += 1;
      renderCurrent();
      maxRenderMs = Math.max(maxRenderMs, releasePlayer.display.ms, fixturePlayer.display.ms);
      if (plies % 8 === 0 || state.current.status !== 'playing') {
        els.result.textContent = `Revision ${state.current.rev} · ${state.current.turn} to move`;
        els.resultSub.textContent = `${state.moves.length} plies · last choose-move ${ms.toFixed(1)} ms · max ${maxMoveMs.toFixed(1)} ms`;
        await new Promise((resolve) => setTimeout(resolve, 0));
      }
    }
    if (state.current.status === 'playing') throw new Error('match did not finish within 300 plies');

    const final = state.current;
    const winnerText = final.status === 'drawn' ? 'Draw' : `${final.winner === 'red' ? 'Red' : 'Blue'} wins`;
    els.result.textContent = `${winnerText} · ${final.reason}`;
    els.resultSub.textContent = `${state.moves.length} plies · max choose-move ${maxMoveMs.toFixed(1)} ms · max render ${maxRenderMs.toFixed(1)} ms · both boards rendered every revision.`;
    setMode('done');
    log(`result ${final.status} ${final.winner ?? ''} ${final.reason} at revision ${final.rev}`);
    log(`max choose-move ${maxMoveMs.toFixed(1)} ms, max render ${maxRenderMs.toFixed(1)} ms`);
    return { plies: state.moves.length, status: final.status, winner: final.winner, reason: final.reason, maxMoveMs, maxRenderMs, moves: state.moves };
  } catch (error) {
    setMode('error');
    els.result.textContent = 'Conformance error';
    els.resultSub.textContent = String(error);
    log(`ERROR ${error}`);
    return null;
  } finally {
    state.running = false;
    els.run.disabled = false;
  }
}

function escapeHtml(value) {
  return String(value).replace(/[&<>"']/g, (ch) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[ch]));
}

els.run.addEventListener('click', () => void runMatch());
els.seat.addEventListener('change', () => setSeats(els.seat.value));
els.flipLeft.addEventListener('click', () => {
  state.leftFlip = !state.leftFlip;
  renderCurrent();
});
els.flipRight.addEventListener('click', () => {
  state.rightFlip = !state.rightFlip;
  renderCurrent();
});

window.__conformance = {
  run: runMatch,
  state: () => ({
    running: state.running,
    current: state.current,
    moves: state.moves,
    manifest: state.manifest,
  }),
};

setSeats(els.seat.value);
try {
  state.manifest = await loadManifest();
  const mods = await Promise.all([loadComponent(state.manifest.release), loadComponent(state.manifest.fixture)]);
  state.modules = { release: mods[0], fixture: mods[1] };
  els.meta.innerHTML = `<strong>${escapeHtml(state.manifest.release.info?.title ?? 'Release')}</strong><br><code>${escapeHtml(state.manifest.release.filename)}</code><br><code>${escapeHtml(state.manifest.release.sha256)}</code>`;
  setMode('ready');
  log('release and fixture components loaded');
} catch (error) {
  setMode('load error');
  els.result.textContent = 'Could not load components';
  els.resultSub.textContent = String(error);
  log(`ERROR ${error}`);
}
