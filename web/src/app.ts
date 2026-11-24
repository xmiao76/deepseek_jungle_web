import { IDENTITY, ARENA_UPLOAD_URL, ARENA_URL } from './identity.generated';
import { EngineClient } from './engine';
import { Sound, type SoundKind } from './audio';
import {
  ANIMAL_NAMES,
  SIDE_BLUE,
  SIDE_RED,
  animalName,
  formatMove,
  moveKey,
  ownerName,
  pieceOwner,
  pieceRank,
  reasonText,
  resultToken,
  sideName,
  turnAfter,
  type AiResult,
  type Frame,
  type Move,
  type Snapshot,
  type WireSide,
} from './protocol';

type Mode = 'human' | 'aiai';
type Difficulty = 'easy' | 'medium' | 'hard';
type First = 'human' | 'ai' | 'red' | 'blue';
type Screen = 'menu' | 'game';

interface Settings {
  mode: Mode;
  difficulty: Difficulty;
  first: First;
  muted: boolean;
}

interface LatencyEntry {
  requestId: number;
  requestedAt: number;
  repliedAt: number;
  committedAt: number;
  ms: number;
  difficulty: Difficulty;
}

const AI_LIMITS: Record<Difficulty, { ms: number; depth: number }> = {
  easy: { ms: 90, depth: 4 },
  medium: { ms: 300, depth: 10 },
  hard: { ms: 950, depth: 32 },
};

const DIFFICULTY_LABEL: Record<Difficulty, string> = {
  easy: 'Easy',
  medium: 'Medium',
  hard: 'Hard',
};

const STORAGE_KEY = 'jungle-v2-settings';

function esc(value: string): string {
  return value.replace(/[&<>"']/g, (ch) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[ch]!));
}

function fmt(n: number): string {
  return n.toLocaleString('en-US');
}

function $<T extends HTMLElement = HTMLElement>(selector: string): T | null {
  return document.querySelector<T>(selector);
}

export class JungleApp {
  private engine: EngineClient;
  private sound: Sound;
  private root: HTMLElement;
  private screen: Screen = 'menu';
  private settings: Settings;
  private previewSnapshot: Snapshot | null = null;
  private snapshot: Snapshot | null = null;
  private first: WireSide = SIDE_RED;
  private humanSide: WireSide | null = null;
  private moves: Move[] = [];
  private flipped = false;
  private selected: number | null = null;
  private hover: number | null = null;
  private cursor: number | null = null;
  private thinking = false;
  private paused = false;
  private generation = 0;
  private requestSeq = 0;
  private latency: LatencyEntry[] = [];
  private lastAi: AiResult | null = null;
  private autoTimer: number | null = null;
  private renderToken = 0;
  private boardCanvas: HTMLCanvasElement | null = null;
  private boardObserver: ResizeObserver | null = null;
  private menuCanvas: HTMLCanvasElement | null = null;
  private menuObserver: ResizeObserver | null = null;
  private modalRoot: HTMLElement | null = null;
  private toastTimer: number | null = null;
  private resultDismissed = false;

  constructor(root: HTMLElement, engine: EngineClient, opening: Snapshot) {
    this.root = root;
    this.engine = engine;
    this.previewSnapshot = opening;
    this.settings = this.loadSettings();
    this.sound = new Sound(this.settings.muted);
    this.bindGlobal();
    this.showMenu(opening);
    this.exposeTestApi();
  }

  // ---- Settings -----------------------------------------------------------

  private loadSettings(): Settings {
    const fallback: Settings = { mode: 'human', difficulty: 'medium', first: 'human', muted: false };
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      if (!raw) return fallback;
      const parsed = JSON.parse(raw) as Partial<Settings>;
      return {
        mode: parsed.mode === 'aiai' ? 'aiai' : 'human',
        difficulty: parsed.difficulty === 'easy' || parsed.difficulty === 'hard' ? parsed.difficulty : 'medium',
        first: parsed.first === 'ai' || parsed.first === 'red' || parsed.first === 'blue' ? parsed.first : 'human',
        muted: parsed.muted === true,
      };
    } catch {
      return fallback;
    }
  }

  private saveSettings(): void {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(this.settings));
    } catch {
      // storage can be unavailable (private mode); settings still work in-session
    }
  }

  private setSettings(patch: Partial<Settings>): void {
    const next = { ...this.settings, ...patch };
    if (next.mode === 'human' && next.first !== 'human' && next.first !== 'ai') next.first = 'human';
    if (next.mode === 'aiai' && next.first !== 'red' && next.first !== 'blue') next.first = 'red';
    this.settings = next;
    this.sound.setMuted(next.muted);
    this.saveSettings();
    if (this.screen === 'menu') {
      this.renderMenu();
    } else {
      this.updateGameUI();
    }
  }

  // ---- Menu ---------------------------------------------------------------

  private async showMenu(opening?: Snapshot): Promise<void> {
    this.generation++;
    this.stopAuto();
    this.thinking = false;
    this.detachBoard();
    this.screen = 'menu';
    this.previewSnapshot = opening ?? this.previewSnapshot;
    this.renderMenu();
    if (!this.previewSnapshot) {
      try {
        this.previewSnapshot = await this.engine.newGame(SIDE_RED, 0);
      } catch {
        this.previewSnapshot = null;
      }
    }
    this.renderMenuPreview();
  }

  private renderMenu(): void {
    this.root.innerHTML = this.menuHtml();
    this.menuCanvas = $('#menu-canvas');
    if (this.menuObserver) this.menuObserver.disconnect();
    if (this.menuCanvas && typeof ResizeObserver !== 'undefined') {
      this.menuObserver = new ResizeObserver(() => this.renderMenuPreview());
      this.menuObserver.observe(this.menuCanvas);
    }
    this.renderMenuPreview();
  }

  private menuHtml(): string {
    const platform = IDENTITY.modelPlatform.trim();
    const firstOptions: Array<[First, string]> =
      this.settings.mode === 'human'
        ? [['human', 'Human first'], ['ai', 'AI first']]
        : [['red', 'Red first'], ['blue', 'Blue first']];
    const seg = (setting: 'mode' | 'difficulty' | 'first', value: string, label: string) => {
      const active = this.settings[setting] === value ? ' is-active' : '';
      return `<button type="button" class="seg${active}" data-setting="${setting}" data-value="${value}" aria-pressed="${active ? 'true' : 'false'}">${esc(label)}</button>`;
    };
    return `
      <div class="app-shell menu-shell" data-screen="menu">
        <header class="topbar">
          <a class="wordmark" href="#" aria-label="Jungle home"><span class="wordmark-mark">J</span><span>JUNGLE</span></a>
          <nav class="topnav">
            <button class="link-button" data-action="how-to">How to play</button>
            ${window.__JUNGLE_EMBEDDED__ ? '' : '<a class="link-button" href="./conformance/">Conformance host</a>'}
            <a class="link-button" href="${esc(IDENTITY.githubUrl)}" target="_blank" rel="noreferrer">GitHub</a>
          </nav>
        </header>

        <main class="menu-grid">
          <section class="hero">
            <p class="eyebrow">Dou Shou Qi · 斗兽棋</p>
            <h1>Jungle<span class="title-dot">.</span></h1>
            <p class="tagline">A quiet board, a treacherous river, and eight animals fighting for the den.</p>
            <div class="identity-line">
              <span>${esc(IDENTITY.model)}</span><span class="dot-sep">·</span><span>${esc(IDENTITY.codeAgent)}</span>${platform ? `<span class="dot-sep">·</span><span>${esc(platform)}</span>` : ''}
            </div>
            <div class="hero-board-wrap">
              <canvas id="menu-canvas" aria-label="Jungle opening position"></canvas>
            </div>
            <div class="hero-footer">
              <span>Engine ${esc(IDENTITY.engineVersion)}</span><span class="dot-sep">·</span><span>${esc(IDENTITY.runtime)}</span>
            </div>
          </section>

          <section class="setup-card card">
            <div class="card-kicker">New match</div>
            <h2>Enter the jungle</h2>

            <div class="field">
              <div class="field-label">Mode</div>
              <div class="segmented" role="group" aria-label="Mode">
                ${seg('mode', 'human', 'Human vs AI')}
                ${seg('mode', 'aiai', 'AI vs AI')}
              </div>
            </div>

            <div class="field">
              <div class="field-label">Difficulty</div>
              <div class="segmented" role="group" aria-label="Difficulty">
                ${seg('difficulty', 'easy', 'Easy')}
                ${seg('difficulty', 'medium', 'Medium')}
                ${seg('difficulty', 'hard', 'Hard')}
              </div>
              <p class="field-help">${this.settings.difficulty === 'hard' ? 'One second of search per move.' : this.settings.difficulty === 'medium' ? 'A brisk, balanced opponent.' : 'A gentler opponent for learning the board.'}</p>
            </div>

            <div class="field">
              <div class="field-label">First move</div>
              <div class="segmented" role="group" aria-label="First move">
                ${firstOptions.map(([value, label]) => seg('first', value, label)).join('')}
              </div>
            </div>

            <button class="btn btn-primary btn-large" data-action="start">Start the match</button>
            <div class="setup-actions">
              <button class="link-button" data-action="how-to">How to play</button>
              <button class="link-button" data-action="download">Download Arena .wasm</button>
            </div>
          </section>
        </main>

        <footer class="menu-footer">
          <div>
            <span class="footer-label">Rules</span>
            <span class="mono">${esc(IDENTITY.rulesId)}</span>
            <span class="footer-digest mono" title="${esc(IDENTITY.rulesDigest)}">${esc(IDENTITY.rulesDigest.slice(0, 12))}…</span>
          </div>
          <div>
            <a href="${esc(IDENTITY.githubUrl)}" target="_blank" rel="noreferrer">${esc(IDENTITY.githubUrl.replace('https://', ''))}</a>
            <span class="dot-sep">·</span>
            <span>v${esc(IDENTITY.gameVersion)}</span>
          </div>
        </footer>
      </div>`;
  }

  private async renderMenuPreview(): Promise<void> {
    const canvas = this.menuCanvas;
    const snapshot = this.previewSnapshot;
    if (!canvas || !snapshot || this.screen !== 'menu') return;
    const width = canvas.clientWidth || 340;
    const scale = Math.max(1, Math.min(2, Math.round((width * Math.min(2, devicePixelRatio || 1)) / 448)));
    try {
      const frame = await this.engine.render({
        scale,
        flipped: false,
        progress: 1000,
        selected: null,
        hover: null,
        cursor: null,
      });
      if (this.screen !== 'menu') return;
      this.drawFrame(canvas, frame);
    } catch {
      // preview is decorative; the menu still works
    }
  }

  // ---- Game ---------------------------------------------------------------

  private async startGame(): Promise<void> {
    this.generation++;
    const gen = this.generation;
    this.stopAuto();
    this.sound.unlock();
    this.sound.play('click');
    this.first =
      this.settings.mode === 'human'
        ? SIDE_RED
        : this.settings.first === 'blue'
          ? SIDE_BLUE
          : SIDE_RED;
    this.humanSide =
      this.settings.mode === 'human'
        ? this.settings.first === 'human'
          ? SIDE_RED
          : SIDE_BLUE
        : null;
    this.flipped = this.humanSide === SIDE_BLUE;
    this.moves = [];
    this.thinking = false;
    this.paused = false;
    this.selected = null;
    this.hover = null;
    this.cursor = null;
    this.lastAi = null;
    this.latency = [];
    this.resultDismissed = false;
    this.screen = 'game';
    this.detachBoard();
    this.root.innerHTML = this.gameHtml();
    this.bindBoard();
    this.updateGameUI();
    try {
      const snapshot = await this.engine.reset(this.first, (Date.now() & 0xffffffff) >>> 0);
      if (gen !== this.generation) return;
      this.snapshot = snapshot;
      this.updateGameUI();
      await this.renderBoard(1000);
      if (this.isAiTurn()) this.scheduleAi(220);
    } catch (error) {
      this.showError(`The engine could not start. ${String(error)}`);
    }
  }

  private gameHtml(): string {
    const humanLabel = this.humanSide === null ? 'AI vs AI' : `You play ${sideName(this.humanSide)}`;
    return `
      <div class="app-shell game-shell" data-screen="game" id="game-view">
        <header class="game-topbar">
          <button class="icon-button" data-action="menu" aria-label="Back to menu">←</button>
          <div class="game-heading">
            <div class="game-title">Jungle</div>
            <div class="game-sub">${esc(humanLabel)} · ${DIFFICULTY_LABEL[this.settings.difficulty]}</div>
          </div>
          <div class="game-top-actions">
            <span class="chip" id="engine-mode">${this.engine.mode === 'worker' ? 'worker engine' : 'main-thread engine'}</span>
            <button class="icon-button" data-action="mute" id="mute-button" aria-label="Toggle sound">${this.settings.muted ? '🔇' : '🔊'}</button>
          </div>
        </header>

        <main class="game-grid">
          <section class="board-stage">
            <div class="turn-row">
              <div class="turn-pill" id="turn-pill"><span class="side-dot red"></span><span id="turn-text">Red to move</span></div>
              <div class="status-text" id="status-text" aria-live="polite">Loading position…</div>
            </div>
            <div class="board-frame" id="board-frame">
              <canvas id="board-canvas" tabindex="0" aria-label="Jungle board. Select a piece, then a highlighted square."></canvas>
              <div class="result-overlay" id="result-overlay" hidden></div>
            </div>
            <div class="board-hint">
              <span><span class="hint-dot jade"></span> move</span>
              <span><span class="hint-ring"></span> capture</span>
              <span>Click a piece, then a square. Flip and undo stay available while the AI thinks.</span>
            </div>
          </section>

          <aside class="rail">
            <section class="card status-card">
              <div class="card-kicker">Match</div>
              <div class="status-line" id="rail-status">Preparing…</div>
              <div class="status-sub" id="rail-sub"></div>
              <div class="search-info mono" id="search-info">depth — · nodes — · — ms</div>
            </section>

            <section class="card controls-card">
              <div class="button-grid">
                <button class="btn" data-action="new-game">New game</button>
                <button class="btn" data-action="undo" id="undo-button">Undo</button>
                <button class="btn" data-action="flip">Flip board</button>
                <button class="btn" data-action="mute">${this.settings.muted ? 'Unmute' : 'Mute'}</button>
              </div>
              ${this.settings.mode === 'aiai' ? `<button class="btn btn-primary" data-action="pause" id="pause-button">Pause</button>` : ''}
              <button class="link-button rail-link" data-action="download">Download Arena .wasm</button>
              <button class="link-button rail-link" data-action="menu">Leave match</button>
            </section>

            <section class="card captured-card">
              <div class="card-kicker">Captured</div>
              <div class="captured-row"><span class="side-label red">Red lost</span><div id="captured-red" class="captured-list"></div></div>
              <div class="captured-row"><span class="side-label blue">Blue lost</span><div id="captured-blue" class="captured-list"></div></div>
            </section>

            <section class="card history-card">
              <div class="card-kicker">Moves</div>
              <div class="move-history" id="move-history"></div>
            </section>
          </aside>
        </main>
      </div>`;
  }

  private bindBoard(): void {
    const canvas = $('#board-canvas') as HTMLCanvasElement | null;
    if (!canvas) return;
    this.boardCanvas = canvas;
    canvas.addEventListener('pointerdown', (event) => this.onBoardPointer(event, true));
    canvas.addEventListener('pointermove', (event) => this.onBoardPointer(event, false));
    canvas.addEventListener('pointerleave', () => {
      if (this.hover !== null) {
        this.hover = null;
        void this.renderBoard();
      }
    });
    canvas.addEventListener('focus', () => {
      if (this.cursor === null) {
        this.cursor = this.isHumanTurn() ? this.findHumanPiece() : 40;
        void this.renderBoard();
      }
    });
    canvas.addEventListener('blur', () => {
      if (this.cursor !== null) {
        this.cursor = null;
        void this.renderBoard();
      }
    });
    canvas.addEventListener('keydown', (event) => this.onBoardKey(event));
    if (typeof ResizeObserver !== 'undefined') {
      const frame = $('#board-frame');
      this.boardObserver = new ResizeObserver(() => void this.renderBoard());
      if (frame) this.boardObserver.observe(frame);
    }
  }

  private detachBoard(): void {
    this.boardObserver?.disconnect();
    this.boardObserver = null;
    this.boardCanvas = null;
    if (this.menuObserver) {
      this.menuObserver.disconnect();
      this.menuObserver = null;
    }
  }

  private findHumanPiece(): number {
    if (!this.snapshot || this.humanSide === null) return 40;
    const ownerBit = this.humanSide === SIDE_RED ? 1 : 0;
    for (let sq = 0; sq < 63; sq++) {
      const id = this.snapshot.board[sq];
      if (id !== 0 && pieceOwner(id) === ownerBit) return sq;
    }
    return 40;
  }

  private squareFromEvent(event: PointerEvent): number | null {
    const canvas = this.boardCanvas;
    if (!canvas) return null;
    const rect = canvas.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) return null;
    const col = Math.floor(((event.clientX - rect.left) / rect.width) * 7);
    const row = Math.floor(((event.clientY - rect.top) / rect.height) * 9);
    if (col < 0 || col > 6 || row < 0 || row > 8) return null;
    const display = row * 7 + col;
    return this.flipped ? 62 - display : display;
  }

  private onBoardPointer(event: PointerEvent, commit: boolean): void {
    const sq = this.squareFromEvent(event);
    if (sq === null) return;
    if (commit) {
      this.onSquare(sq);
      return;
    }
    if (!this.isHumanTurn() || this.snapshot === null) return;
    if (this.hover !== sq) {
      this.hover = sq;
      void this.renderBoard();
    }
  }

  private onSquare(sq: number): void {
    if (!this.snapshot || !this.isHumanTurn()) return;
    if (this.selected !== null) {
      const legal = this.snapshot.legal.some((m) => m[0] === this.selected && m[1] === sq);
      if (legal) {
        const from = this.selected;
        this.selected = null;
        void this.commitMove(from, sq, null);
        return;
      }
    }
    const id = this.snapshot.board[sq];
    const humanOwnerBit = this.humanSide === SIDE_RED ? 1 : 0;
    if (id !== 0 && pieceOwner(id) === humanOwnerBit) {
      this.selected = sq;
      this.cursor = sq;
      this.sound.play('select');
      void this.renderBoard();
      this.updateGameUI();
      return;
    }
    if (this.selected !== null) {
      this.selected = null;
      void this.renderBoard();
      this.updateGameUI();
    }
  }

  private onBoardKey(event: KeyboardEvent): void {
    if (this.cursor === null) {
      this.cursor = this.isHumanTurn() ? this.findHumanPiece() : 40;
    }
    let display = this.flipped ? 62 - this.cursor : this.cursor;
    let row = Math.floor(display / 7);
    let col = display % 7;
    switch (event.key) {
      case 'ArrowUp':
        row = Math.max(0, row - 1);
        break;
      case 'ArrowDown':
        row = Math.min(8, row + 1);
        break;
      case 'ArrowLeft':
        col = Math.max(0, col - 1);
        break;
      case 'ArrowRight':
        col = Math.min(6, col + 1);
        break;
      case 'Enter':
      case ' ':
        event.preventDefault();
        this.onSquare(this.cursor);
        return;
      case 'Escape':
        this.selected = null;
        this.updateGameUI();
        void this.renderBoard();
        return;
      default:
        return;
    }
    event.preventDefault();
    display = row * 7 + col;
    this.cursor = this.flipped ? 62 - display : display;
    void this.renderBoard();
  }

  private isHumanTurn(): boolean {
    return (
      this.settings.mode === 'human' &&
      this.snapshot !== null &&
      this.snapshot.status === 0 &&
      this.humanSide !== null &&
      this.snapshot.turn === this.humanSide
    );
  }

  private isAiTurn(): boolean {
    if (!this.snapshot || this.snapshot.status !== 0) return false;
    if (this.settings.mode === 'aiai') return true;
    return this.humanSide !== null && this.snapshot.turn !== this.humanSide;
  }

  // ---- AI -----------------------------------------------------------------

  private scheduleAi(delay: number): void {
    this.stopAuto();
    if (this.paused || this.thinking || !this.isAiTurn()) return;
    this.autoTimer = window.setTimeout(() => {
      this.autoTimer = null;
      void this.requestAi();
    }, delay);
  }

  private stopAuto(): void {
    if (this.autoTimer !== null) {
      clearTimeout(this.autoTimer);
      this.autoTimer = null;
    }
  }

  private async requestAi(): Promise<void> {
    if (!this.snapshot || this.thinking || this.paused || !this.isAiTurn()) return;
    const gen = this.generation;
    const requestId = ++this.requestSeq;
    const requestedAt = performance.now();
    const limits = AI_LIMITS[this.settings.difficulty];
    this.thinking = true;
    this.updateGameUI();
    let entry: LatencyEntry | null = null;
    try {
      const result = await this.engine.search(0, limits.ms, limits.depth);
      if (gen !== this.generation) return;
      const repliedAt = performance.now();
      this.lastAi = result;
      entry = { requestId, requestedAt, repliedAt, committedAt: repliedAt, ms: repliedAt - requestedAt, difficulty: this.settings.difficulty };
      const legal = this.snapshot.legal;
      const move = legal.find((m) => m[0] === result.from && m[1] === result.to) ?? legal[0];
      if (!move) {
        this.thinking = false;
        this.updateGameUI();
        return;
      }
      await this.commitMove(move[0], move[1], entry);
    } catch (error) {
      if (gen !== this.generation) return;
      this.thinking = false;
      try {
        this.snapshot = await this.engine.fallback(this.moves, this.first);
        this.updateGameUI();
        this.renderBoard();
        if (this.isAiTurn()) this.scheduleAi(120);
      } catch {
        this.showError(`The engine stopped responding. ${String(error)}`);
      }
    }
  }

  private async commitMove(from: number, to: number, latency: LatencyEntry | null): Promise<void> {
    if (!this.snapshot) return;
    const gen = this.generation;
    let result;
    try {
      result = await this.engine.apply(from, to);
    } catch (error) {
      this.thinking = false;
      this.updateGameUI();
      this.showError(`Could not apply the move. ${String(error)}`);
      return;
    }
    if (gen !== this.generation) return;
    if (result.code !== 0) {
      this.thinking = false;
      this.updateGameUI();
      this.showToast('That move is not legal.');
      return;
    }
    this.moves.push([from, to]);
    this.snapshot = result.snapshot;
    this.selected = null;
    this.hover = null;
    this.thinking = false;
    this.resultDismissed = false;
    if (latency) {
      latency.committedAt = performance.now();
      latency.ms = latency.committedAt - latency.requestedAt;
      this.latency.push(latency);
    }
    this.sound.play(this.snapshot.lastCapture !== 0 ? 'capture' : 'move');
    this.updateGameUI();
    await this.animateMove();
    if (gen !== this.generation) return;
    this.updateGameUI();
    if (this.snapshot.status !== 0) {
      this.onTerminal();
      return;
    }
    if (this.isAiTurn()) this.scheduleAi(this.settings.mode === 'aiai' ? 140 : 0);
  }

  private async animateMove(): Promise<void> {
    if (this.screen !== 'game' || !this.boardCanvas) return;
    const steps = 5;
    for (let i = 1; i <= steps; i++) {
      if (this.screen !== 'game') return;
      await this.renderBoard(Math.round((i / steps) * 1000), true);
    }
    await this.renderBoard(1000);
  }

  private onTerminal(): void {
    if (!this.snapshot) return;
    if (this.snapshot.status === 2) this.sound.play('draw');
    else if (this.humanSide !== null) this.sound.play(this.snapshot.winner === this.humanSide ? 'win' : 'lose');
    else this.sound.play('win');
    this.updateGameUI();
  }

  // ---- Rendering ----------------------------------------------------------

  private drawFrame(canvas: HTMLCanvasElement, frame: Frame): void {
    if (canvas.width !== frame.width) canvas.width = frame.width;
    if (canvas.height !== frame.height) canvas.height = frame.height;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    if (frame.bitmap) {
      ctx.drawImage(frame.bitmap, 0, 0);
    } else if (frame.data) {
      ctx.putImageData(new ImageData(frame.data as Uint8ClampedArray<ArrayBuffer>, frame.width, frame.height), 0, 0);
    }
  }

  private chooseScale(animation: boolean): number {
    const canvas = this.boardCanvas;
    const cssWidth = canvas?.clientWidth || 448;
    const dpr = Math.min(3, Math.max(1, devicePixelRatio || 1));
    const wanted = Math.round((cssWidth * dpr) / 448);
    const capped = Math.max(1, Math.min(3, wanted));
    return animation ? Math.min(2, capped) : capped;
  }

  private async renderBoard(progress = 1000, animation = false): Promise<void> {
    if (this.screen !== 'game' || !this.boardCanvas || !this.snapshot) return;
    const token = ++this.renderToken;
    try {
      const frame = await this.engine.render({
        scale: this.chooseScale(animation),
        flipped: this.flipped,
        progress,
        selected: this.selected,
        hover: this.hover,
        cursor: this.cursor,
      });
      if (token !== this.renderToken || this.screen !== 'game' || !this.boardCanvas) return;
      this.drawFrame(this.boardCanvas, frame);
    } catch {
      // a stale animation frame is harmless
    }
  }

  // ---- UI updates ---------------------------------------------------------

  private updateGameUI(): void {
    if (this.screen !== 'game' || !this.snapshot) return;
    const view = $('#game-view');
    if (view) {
      view.dataset.turn = sideName(this.snapshot.turn);
      view.dataset.status = String(this.snapshot.status);
      view.dataset.ply = String(this.moves.length);
      view.dataset.thinking = String(this.thinking);
      view.dataset.result = resultToken(this.snapshot);
      view.dataset.engineMode = this.engine.mode;
      view.dataset.paused = String(this.paused);
      view.dataset.flipped = String(this.flipped);
      view.dataset.humanSide = this.humanSide === null ? 'none' : sideName(this.humanSide);
    }
    const terminal = this.snapshot.status !== 0;
    const turnName = sideName(this.snapshot.turn);
    const turnText = $('#turn-text');
    if (turnText) turnText.textContent = terminal ? 'Match over' : `${turnName[0].toUpperCase()}${turnName.slice(1)} to move`;
    const pill = $('#turn-pill');
    if (pill) {
      pill.dataset.side = terminal ? 'none' : turnName;
      pill.innerHTML = terminal
        ? `<span class="side-dot muted"></span><span>Match over</span>`
        : `<span class="side-dot ${turnName}"></span><span>${turnName[0].toUpperCase()}${turnName.slice(1)} to move</span>`;
    }

    let status = '';
    let sub = '';
    if (terminal) {
      status = this.resultTitle();
      sub = reasonText(this.snapshot.reasonCode);
    } else if (this.thinking) {
      status = this.settings.mode === 'aiai' ? `${turnName[0].toUpperCase()}${turnName.slice(1)} is thinking…` : 'The AI is thinking…';
      sub = `${DIFFICULTY_LABEL[this.settings.difficulty]} · thought stays off the board`;
    } else if (this.settings.mode === 'aiai') {
      status = this.paused ? 'Paused' : 'AI vs AI';
      sub = this.paused ? 'Resume to continue the match.' : 'Sit back and watch the jungle decide.';
    } else if (this.isHumanTurn()) {
      status = 'Your move';
      sub = this.selected !== null ? 'Choose a highlighted square.' : 'Select one of your pieces.';
    } else {
      status = 'Waiting';
      sub = '';
    }
    const statusEl = $('#status-text');
    if (statusEl) statusEl.textContent = terminal ? `${status}${sub ? ` · ${sub}` : ''}` : status;
    const railStatus = $('#rail-status');
    if (railStatus) railStatus.textContent = status;
    const railSub = $('#rail-sub');
    if (railSub) railSub.textContent = sub;

    const mode = $('#engine-mode');
    if (mode) mode.textContent = this.engine.mode === 'worker' ? 'worker engine' : 'main-thread engine';

    const searchInfo = $('#search-info');
    if (searchInfo) {
      if (this.lastAi) {
        const nps = this.lastAi.elapsedMs > 0 ? Math.round(this.lastAi.nodes / (this.lastAi.elapsedMs / 1000)) : 0;
        searchInfo.textContent = `depth ${this.lastAi.depth} · ${fmt(this.lastAi.nodes)} nodes · ${Math.round(this.lastAi.elapsedMs)} ms · ${fmt(nps)} nps`;
      } else {
        searchInfo.textContent = `depth — · nodes — · — ms`;
      }
    }

    const undo = $('#undo-button') as HTMLButtonElement | null;
    if (undo) undo.disabled = this.moves.length === 0 || (this.settings.mode === 'human' && this.humanSide !== null && turnAfter(this.first, 0) !== this.humanSide && this.moves.length < 2);
    const pause = $('#pause-button');
    if (pause) {
      pause.textContent = this.paused ? 'Resume' : 'Pause';
      pause.setAttribute('data-action', this.paused ? 'resume' : 'pause');
      pause.hidden = terminal;
    }
    const mute = $('#mute-button');
    if (mute) mute.textContent = this.settings.muted ? '🔇' : '🔊';

    this.renderResult();
    this.renderCaptured();
    this.renderHistory();
  }

  private resultTitle(): string {
    if (!this.snapshot) return '';
    if (this.snapshot.status === 2) return 'Draw';
    const winner = this.snapshot.winner ?? SIDE_RED;
    if (this.settings.mode === 'human' && this.humanSide !== null) {
      return winner === this.humanSide ? 'You win' : 'AI wins';
    }
    return `${sideName(winner)[0].toUpperCase()}${sideName(winner).slice(1)} wins`;
  }

  private renderResult(): void {
    const overlay = $('#result-overlay');
    if (!overlay || !this.snapshot) return;
    if (this.snapshot.status === 0 || this.resultDismissed) {
      overlay.hidden = true;
      overlay.innerHTML = '';
      return;
    }
    const winner = this.snapshot.winner;
    overlay.hidden = false;
    overlay.dataset.result = resultToken(this.snapshot);
    overlay.innerHTML = `
      <div class="result-card ${this.snapshot.status === 2 ? 'is-draw' : winner === SIDE_RED ? 'is-red' : 'is-blue'}">
        <div class="result-kicker">Match result</div>
        <div class="result-title">${esc(this.resultTitle())}</div>
        <div class="result-reason">${esc(reasonText(this.snapshot.reasonCode))}</div>
        <div class="result-actions">
          <button class="btn btn-primary" data-action="rematch">Play again</button>
          <button class="btn" data-action="review">Review board</button>
          <button class="link-button" data-action="menu">Menu</button>
        </div>
      </div>`;
  }

  private renderCaptured(): void {
    if (!this.snapshot) return;
    const current: Record<'red' | 'blue', Set<number>> = { red: new Set(), blue: new Set() };
    for (let sq = 0; sq < 63; sq++) {
      const id = this.snapshot.board[sq];
      if (id === 0) continue;
      const owner = ownerName(id);
      current[owner].add(pieceRank(id));
    }
    const missing = (side: 'red' | 'blue'): number[] => {
      const out: number[] = [];
      for (let rank = 1; rank <= 8; rank++) if (!current[side].has(rank)) out.push(rank);
      return out.reverse();
    };
    const render = (ranks: number[]) =>
      ranks.length === 0
        ? '<span class="empty-note">none</span>'
        : ranks.map((rank) => `<span class="capture-chip rank-${rank}">${esc(ANIMAL_NAMES[rank - 1])}</span>`).join('');
    const red = $('#captured-red');
    const blue = $('#captured-blue');
    if (red) red.innerHTML = render(missing('red'));
    if (blue) blue.innerHTML = render(missing('blue'));
  }

  private renderHistory(): void {
    const el = $('#move-history');
    if (!el) return;
    if (this.moves.length === 0) {
      el.innerHTML = '<div class="empty-note">The first move awaits.</div>';
      return;
    }
    const lines: string[] = [];
    for (let i = 0; i < this.moves.length; i += 2) {
      const mover = turnAfter(this.first, i);
      const first = this.moves[i];
      const second = this.moves[i + 1];
      const dot = (side: WireSide) => `<span class="side-dot ${sideName(side)}"></span>`;
      const one = `${dot(mover)} ${formatMove(first)}`;
      const two = second ? `<span class="move-pair">${dot(turnAfter(this.first, i + 1))} ${formatMove(second)}</span>` : '';
      lines.push(`<div class="move-line"><span class="move-no">${Math.floor(i / 2) + 1}.</span><span class="move-one">${one}</span>${two}</div>`);
    }
    el.innerHTML = lines.join('');
    el.scrollTop = el.scrollHeight;
  }

  // ---- Actions ------------------------------------------------------------

  private async newGame(): Promise<void> {
    if (this.screen !== 'game') return;
    this.generation++;
    const gen = this.generation;
    this.stopAuto();
    this.thinking = false;
    this.paused = false;
    this.moves = [];
    this.selected = null;
    this.hover = null;
    this.cursor = null;
    this.lastAi = null;
    this.latency = [];
    this.resultDismissed = false;
    this.sound.play('click');
    try {
      const snapshot = await this.engine.reset(this.first, (Date.now() & 0xffffffff) >>> 0);
      if (gen !== this.generation) return;
      this.snapshot = snapshot;
      this.updateGameUI();
      await this.renderBoard(1000);
      if (this.isAiTurn()) this.scheduleAi(220);
    } catch (error) {
      this.showError(`Could not start a new game. ${String(error)}`);
    }
  }

  private async undo(): Promise<void> {
    if (this.screen !== 'game' || !this.snapshot || this.moves.length === 0) return;
    this.stopAuto();
    this.generation++;
    const gen = this.generation;
    const wasThinking = this.thinking;
    this.thinking = false;
    let keep = this.moves.length - 1;
    if (this.settings.mode === 'aiai') {
      this.paused = true;
    } else if (this.humanSide !== null) {
      while (keep > 0 && turnAfter(this.first, keep) !== this.humanSide) keep--;
      if (keep === 0 && turnAfter(this.first, 0) !== this.humanSide) {
        this.showToast('There is no human move to undo yet.');
        this.updateGameUI();
        return;
      }
    }
    const keepMoves = this.moves.slice(0, keep);
    try {
      const snapshot = wasThinking
        ? await this.engine.restart(keepMoves, this.first)
        : await this.engine.load(keepMoves, this.first);
      if (gen !== this.generation) return;
      this.moves = keepMoves;
      this.snapshot = snapshot;
      this.selected = null;
      this.hover = null;
      this.lastAi = null;
      this.resultDismissed = false;
      this.sound.play('click');
      this.updateGameUI();
      await this.renderBoard(1000);
      if (this.settings.mode === 'aiai' && !this.paused && this.isAiTurn()) this.scheduleAi(180);
    } catch (error) {
      this.showError(`Undo failed. ${String(error)}`);
    }
  }

  private flip(): void {
    if (this.screen !== 'game') return;
    this.flipped = !this.flipped;
    this.sound.play('click');
    this.updateGameUI();
    void this.renderBoard();
  }

  private toggleMute(): void {
    const muted = !this.settings.muted;
    this.setSettings({ muted });
    this.sound.play('click');
  }

  private setPaused(paused: boolean): void {
    this.paused = paused;
    if (paused) this.stopAuto();
    this.updateGameUI();
    if (!paused && this.isAiTurn() && !this.thinking) this.scheduleAi(180);
  }

  private async leaveToMenu(): Promise<void> {
    this.generation++;
    this.stopAuto();
    this.thinking = false;
    this.detachBoard();
    let preview = this.previewSnapshot;
    try {
      preview = await this.engine.newGame(SIDE_RED, 0);
    } catch {
      // engine may have been terminated while thinking; recreate below
      try {
        preview = await this.engine.reset(SIDE_RED, 0);
      } catch {
        preview = null;
      }
    }
    this.screen = 'menu';
    this.previewSnapshot = preview;
    this.renderMenu();
  }

  // ---- Modals -------------------------------------------------------------

  private openModal(inner: string): void {
    if (!this.modalRoot) {
      this.modalRoot = document.createElement('div');
      this.modalRoot.id = 'modal-root';
      document.body.appendChild(this.modalRoot);
    }
    this.modalRoot.innerHTML = `<div class="modal-backdrop" data-action="close-modal"><div class="modal-panel" role="dialog" aria-modal="true">${inner}</div></div>`;
    const close = this.modalRoot.querySelector<HTMLElement>('.modal-close');
    close?.focus();
  }

  private closeModal(): void {
    if (this.modalRoot) this.modalRoot.innerHTML = '';
  }

  private showHowTo(): void {
    this.openModal(`
      <div class="modal-head">
        <div>
          <div class="card-kicker">Rules</div>
          <h2>How to play</h2>
        </div>
        <button class="icon-button modal-close" data-action="close-modal" aria-label="Close">✕</button>
      </div>
      <div class="modal-body prose">
        <h3>Win the den</h3>
        <p>Move any piece into the opponent's <strong>den</strong>, or capture every enemy piece. A side with <strong>no legal move</strong> loses. The game is drawn when the same position (all 63 squares plus the side to move) appears for the <strong>third time</strong>, or after <strong>100 plies without a capture</strong>.</p>
        <h3>The pieces</h3>
        <p class="rank-line">Rat 1 · Cat 2 · Dog 3 · Wolf 4 · Leopard 5 · Tiger 6 · Lion 7 · Elephant 8</p>
        <p>On land, an animal captures an equal or lower rank. The <strong>Rat</strong> may take an <strong>Elephant</strong> from land, but an Elephant can never take an untrapped Rat. Only Rats swim; captures cannot cross the water boundary, and a swimming Rat can capture another Rat in adjacent water.</p>
        <h3>Rivers and jumps</h3>
        <p>A <strong>Lion</strong> jumps 2 or 3 water squares; a <strong>Tiger</strong> jumps exactly 2. A Rat on any water square along the path blocks the jump. The short jump is <span class="mono">21 → 24</span>, the long jump <span class="mono">15 → 43</span>.</p>
        <h3>Traps and dens</h3>
        <p>A piece on an <strong>opponent trap</strong> has rank zero: any attacker can capture it, but a trapped attacker cannot capture an untrapped defender. Friendly traps are safe. You may never enter your own den.</p>
        <h3>Controls</h3>
        <p>Click a piece, then a highlighted square. <strong>Green</strong> dots are moves; <strong>red</strong> rings are captures. <strong>Undo</strong> returns to your previous decision, <strong>Flip</strong> only rotates the view, <strong>Mute</strong> silences sound. Keyboard: arrows move the cursor, Enter selects, U undoes, F flips, M mutes.</p>
      </div>
    `);
  }

  private async showDownload(): Promise<void> {
    const embedded = window.__JUNGLE_EMBEDDED__;
    let bytes: Uint8Array;
    try {
      bytes = embedded?.componentBase64
        ? base64(embedded.componentBase64)
        : new Uint8Array(await (await fetch(new URL(`assets/${IDENTITY.releaseFilename}`, document.baseURI))).arrayBuffer());
    } catch {
      this.showToast('The Arena file is not available in this build.');
      return;
    }
    let digest = embedded?.sha256 ?? '';
    if (!digest && globalThis.crypto?.subtle) {
      try {
        const hash = await crypto.subtle.digest('SHA-256', bytes as unknown as BufferSource);
        digest = [...new Uint8Array(hash)].map((b) => b.toString(16).padStart(2, '0')).join('');
      } catch {
        digest = '';
      }
    }
    this.openModal(`
      <div class="modal-head">
        <div>
          <div class="card-kicker">Arena entrant</div>
          <h2>Download .wasm</h2>
        </div>
        <button class="icon-button modal-close" data-action="close-modal" aria-label="Close">✕</button>
      </div>
      <div class="modal-body">
        <p class="download-name mono">${esc(IDENTITY.releaseFilename)}</p>
        <dl class="meta-list">
          <div><dt>Size</dt><dd>${fmt(bytes.length)} bytes</dd></div>
          <div><dt>SHA-256</dt><dd class="mono hash">${esc(digest || 'unavailable')}</dd></div>
          <div><dt>Rules</dt><dd class="mono">${esc(IDENTITY.rulesId)}</dd></div>
          <div><dt>Identity</dt><dd>${esc(IDENTITY.title)}</dd></div>
        </dl>
        <button class="btn btn-primary btn-large" data-action="download-wasm">Download Arena .wasm</button>
        <h3>Upload it to the Arena</h3>
        <ol class="steps">
          <li>Open the <a href="${esc(ARENA_UPLOAD_URL)}" target="_blank" rel="noreferrer">Arena Upload page</a>.</li>
          <li>Choose the downloaded <span class="mono">${esc(IDENTITY.releaseFilename)}</span> file.</li>
          <li>Wait for every admission stage to pass, then pick <strong>Play in left seat</strong> or <strong>Play in right seat</strong>.</li>
          <li>Your title, rules identity and opening frame are shown before the match; the Arena identifies the file by SHA-256.</li>
        </ol>
        <p class="muted">The entry implements <span class="mono">desktop-jungle-v2</span> and imports only the Arena's <span class="mono">display</span> interface.</p>
      </div>
    `);
  }

  private downloadComponent(): void {
    const embedded = window.__JUNGLE_EMBEDDED__;
    const trigger = (bytes: Uint8Array) => {
      const blob = new Blob([bytes as unknown as BlobPart], { type: 'application/wasm' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = IDENTITY.releaseFilename;
      document.body.appendChild(a);
      a.click();
      a.remove();
      setTimeout(() => URL.revokeObjectURL(url), 4000);
      this.showToast('Arena file downloaded.');
    };
    if (embedded?.componentBase64) {
      trigger(base64(embedded.componentBase64));
      return;
    }
    void fetch(new URL(`assets/${IDENTITY.releaseFilename}`, document.baseURI))
      .then((r) => r.arrayBuffer())
      .then((b) => trigger(new Uint8Array(b)))
      .catch(() => this.showToast('Download failed.'));
  }

  private showToast(message: string): void {
    let toast = $('#toast');
    if (!toast) {
      toast = document.createElement('div');
      toast.id = 'toast';
      toast.className = 'toast';
      document.body.appendChild(toast);
    }
    toast.textContent = message;
    toast.classList.add('is-visible');
    if (this.toastTimer !== null) clearTimeout(this.toastTimer);
    this.toastTimer = window.setTimeout(() => toast?.classList.remove('is-visible'), 2600);
  }

  private showError(message: string): void {
    this.showToast(message);
    const status = $('#status-text');
    if (status) status.textContent = message;
  }

  // ---- Events -------------------------------------------------------------

  private bindGlobal(): void {
    document.addEventListener('pointerdown', () => this.sound.unlock(), { once: true });
    document.addEventListener('click', (event) => {
      const target = (event.target as HTMLElement).closest<HTMLElement>('[data-action], [data-setting]');
      if (!target) return;
      const setting = target.dataset.setting as 'mode' | 'difficulty' | 'first' | undefined;
      if (setting) {
        const value = target.dataset.value!;
        if (setting === 'mode') this.setSettings({ mode: value as Mode });
        else if (setting === 'difficulty') this.setSettings({ difficulty: value as Difficulty });
        else this.setSettings({ first: value as First });
        return;
      }
      const action = target.dataset.action;
      switch (action) {
        case 'start':
          void this.startGame();
          break;
        case 'new-game':
          void this.newGame();
          break;
        case 'undo':
          void this.undo();
          break;
        case 'flip':
          this.flip();
          break;
        case 'mute':
          this.toggleMute();
          break;
        case 'menu':
          this.closeModal();
          void this.leaveToMenu();
          break;
        case 'how-to':
          this.showHowTo();
          break;
        case 'download':
          void this.showDownload();
          break;
        case 'download-wasm':
          this.downloadComponent();
          break;
        case 'close-modal':
          if (event.target === target) this.closeModal();
          break;
        case 'rematch':
          this.closeModal();
          void this.startGame();
          break;
        case 'review':
          this.resultDismissed = true;
          this.updateGameUI();
          void this.renderBoard(1000);
          break;
        case 'pause':
          this.setPaused(true);
          break;
        case 'resume':
          this.setPaused(false);
          break;
      }
    });

    document.addEventListener('keydown', (event) => {
      if (event.key === 'Escape') {
        this.closeModal();
        return;
      }
      const tag = (event.target as HTMLElement | null)?.tagName;
      if (tag === 'INPUT' || tag === 'SELECT' || tag === 'TEXTAREA') return;
      if (this.screen !== 'game' || !this.isHumanTurn()) return;
      if (event.key === 'u' || event.key === 'U') void this.undo();
      if (event.key === 'f' || event.key === 'F') this.flip();
      if (event.key === 'm' || event.key === 'M') this.toggleMute();
    });
  }

  private exposeTestApi(): void {
    const api = {
      screen: () => this.screen,
      state: () => ({
        screen: this.screen,
        settings: { ...this.settings },
        turn: this.snapshot ? sideName(this.snapshot.turn) : null,
        status: this.snapshot?.status ?? null,
        result: this.snapshot ? resultToken(this.snapshot) : null,
        ply: this.moves.length,
        thinking: this.thinking,
        paused: this.paused,
        engineMode: this.engine.mode,
        selected: this.selected,
        flipped: this.flipped,
        humanSide: this.humanSide === null ? null : sideName(this.humanSide),
        legal: this.snapshot?.legal.map((m) => [m[0], m[1]]) ?? [],
        board: this.snapshot ? Array.from(this.snapshot.board) : [],
        moves: this.moves.map((m) => [m[0], m[1]]),
        latency: this.latency.slice(),
      }),
      latency: () => this.latency.slice(),
      start: () => this.startGame(),
      newGame: () => this.newGame(),
      undo: () => this.undo(),
      flip: () => this.flip(),
      setSettings: (patch: Partial<Settings>) => this.setSettings(patch),
      choose: (from: number, to: number) => this.commitMove(from, to, null),
    };
    (window as unknown as { __jungleTest?: typeof api }).__jungleTest = api;
  }
}

function base64(input: string): Uint8Array {
  const clean = input.replace(/\s+/g, '');
  const padding = clean.endsWith('==') ? 2 : clean.endsWith('=') ? 1 : 0;
  const out = new Uint8Array((clean.length / 4) * 3 - padding);
  let o = 0;
  const val = (c: number) =>
    c >= 65 && c <= 90 ? c - 65 : c >= 97 && c <= 122 ? c - 97 + 26 : c >= 48 && c <= 57 ? c - 48 + 52 : c === 43 ? 62 : c === 47 ? 63 : 0;
  for (let i = 0; i < clean.length; i += 4) {
    const a = val(clean.charCodeAt(i));
    const b = val(clean.charCodeAt(i + 1));
    const c = val(clean.charCodeAt(i + 2));
    const d = val(clean.charCodeAt(i + 3));
    if (o < out.length) out[o++] = (a << 2) | (b >> 4);
    if (o < out.length) out[o++] = ((b & 15) << 4) | (c >> 2);
    if (o < out.length) out[o++] = ((c & 3) << 6) | d;
  }
  return out;
}

export type { Settings };
