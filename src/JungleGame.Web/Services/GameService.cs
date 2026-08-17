using JungleGame.Core.AI;
using JungleGame.Core.Engine;
using JungleGame.Core.Model;
using JungleGame.Web.Models;
using Microsoft.JSInterop;

namespace JungleGame.Web.Services;

/// <summary>Result of a board click, so the UI layer can play matching feedback.</summary>
public enum ClickOutcome
{
    Ignored,
    Selected,
    Deselected,
    Moved,
}

/// <summary>
/// Synchronous game-flow core adapted from the desktop <c>MainViewModel</c>: owns the
/// immutable <see cref="GameState"/> and the <see cref="MinimaxEngine"/>, settings,
/// selection/highlight state, and status text. The async AI turn loop lives in the page
/// component (it needs the renderer dispatcher); this class exposes everything it needs,
/// mirroring the desktop <c>Task.Run</c> + cancellation + staleness-guard pattern.
///
/// Differences from the desktop, by design:
/// - The engine is copied verbatim, so difficulty is applied via
///   <see cref="MinimaxEngine.SetTimeLimit"/> on the single session-long AI instance.
/// - <see cref="GameState.History"/> stores only Zobrist hashes, so this service tracks
///   moves itself (<see cref="TrackedMoves"/>) for the history panel and replay-based undo.
/// - Undo is a web-only addition (the desktop has none): it replays the tracked move list
///   against the pure, deterministic <see cref="GameController.ApplyMove"/>.
/// </summary>
public sealed class GameService
{
    private readonly Dictionary<Position, int> _pieceIds = new();
    private int _nextPieceId;
    private readonly MinimaxEngine _ai;

    // Desktop defaults (StartDialog): Human moves first, Medium difficulty.
    private GameMode _gameMode = GameMode.HumanVsAi;
    private Difficulty _difficulty = Difficulty.Medium;
    private FirstMoveChoice _firstMove = FirstMoveChoice.HumanFirst;

    public GameService()
    {
        State = GameState.CreateInitial();
        _ai = new MinimaxEngine(TimeSpan.FromMilliseconds((int)_difficulty));
        StartNewGame();
        _ = LoadTablebaseAsync();
    }

    /// <summary>
    /// Fetches the gzip'd 2+3-piece endgame tables (a static wwwroot asset) and
    /// hands the inflated bytes to the probe. The search degrades gracefully to
    /// pure search until the load completes; a missing or corrupt asset stays
    /// silent (the probe reports Corrupt and search continues).
    /// </summary>
    private static async Task LoadTablebaseAsync()
    {
        try
        {
            using var http = new HttpClient();
            byte[] gz = await http.GetByteArrayAsync("tablebases/jungle3.tb.gz");
            TablebaseProbe.InitializeFromGzip(gz);
        }
        catch (HttpRequestException)
        {
            // No asset (e.g. dev server without the file): search-only play.
        }
        catch (TaskCanceledException)
        {
        }
    }

    // --- Engine / AI access -------------------------------------------------------------

    public GameState State { get; private set; }

    public MinimaxEngine Ai => _ai;

    /// <summary>The Rust backend when enabled; null when the C# engine is forced.</summary>
    public RustEngineBackend? Rust { get; private set; }

    /// <summary>True when the Rust backend is enabled AND its worker is healthy.</summary>
    public bool UseRust => Rust != null && Rust.IsAvailable;

    /// <summary>Human-readable name of the engine actually driving the AI.</summary>
    public string EngineName => UseRust ? "Rust engine" : "C# engine";

    /// <summary>Human-readable runtime string for the active engine.</summary>
    public string RuntimeLabel => UseRust ? "WASM · Web Worker" : ".NET WASM · AOT";

    /// <summary>Switches the AI to the Rust Web-Worker backend (the shipped default).</summary>
    public void EnableRustEngine(IJSRuntime js)
    {
        Rust ??= new RustEngineBackend(js, TimeSpan.FromMilliseconds((int)_difficulty));
    }

    /// <summary>Forces the C# engine (?engine=csharp), e.g. for A/B or fallback testing.</summary>
    public void DisableRustEngine() => Rust = null;

    /// <summary>Set by the page before each AI search; cancelled when a game resets.</summary>
    public CancellationTokenSource? AiCts { get; set; }

    /// <summary>
    /// Incremented on every StartNewGame. The page's AI loop pins this epoch so a stale
    /// search (started against a replaced game) is never applied. Unlike the state object
    /// itself, the epoch survives the moves of a game — the immutable GameState is replaced
    /// on every applied move, so it cannot serve as the pin.
    /// </summary>
    public int GameEpoch { get; private set; }

    /// <summary>Delay between moves in AI-vs-AI so play is visible (shortened in e2e).</summary>
    public int AiVsAiPaceMs { get; set; } = 300;

    /// <summary>
    /// Incremented whenever the whole piece layer must be rebuilt without slide animations
    /// (new game, undo, flip); a plain move keeps the epoch and animates instead.
    /// </summary>
    public int LayoutEpoch { get; private set; }

    public IReadOnlyDictionary<Position, int> PieceIds => _pieceIds;

    /// <summary>The applied moves of the current game (the desktop _moveHistory).</summary>
    public IReadOnlyList<Move> TrackedMoves => _trackedMoves;

    private List<Move> _trackedMoves = new();

    // --- Settings ------------------------------------------------------------------------

    public IReadOnlyList<GameMode> GameModes { get; } = Enum.GetValues<GameMode>().ToList();

    public IReadOnlyList<Difficulty> Difficulties { get; } = Enum.GetValues<Difficulty>().ToList();

    public IReadOnlyList<FirstMoveChoice> FirstMoveChoices { get; } = Enum.GetValues<FirstMoveChoice>().ToList();

    public GameMode GameMode
    {
        get => _gameMode;
        set { if (_gameMode != value) { _gameMode = value; StartNewGame(); } }
    }

    public Difficulty Difficulty
    {
        get => _difficulty;
        set { if (_difficulty != value) { _difficulty = value; _ai.SetTimeLimit(TimeSpan.FromMilliseconds((int)value)); Rust?.SetTimeLimit(TimeSpan.FromMilliseconds((int)value)); UpdateStatus(); } }
    }

    public FirstMoveChoice FirstMove
    {
        get => _firstMove;
        set { if (_firstMove != value) { _firstMove = value; StartNewGame(); } }
    }

    public bool IsFlipped { get; private set; }

    public bool IsBusy { get; set; }

    // --- Display state -------------------------------------------------------------------

    public Position? Selected { get; private set; }

    public HashSet<Position> Targets { get; } = new();

    public Position? LastMoveFrom { get; private set; }

    public Position? LastMoveTo { get; private set; }

    /// <summary>True while the last applied move was a capture (drives the capture ring).</summary>
    public bool LastMoveWasCapture { get; private set; }

    /// <summary>Increments on every capture so the ring animation can restart per capture.</summary>
    public int CaptureEpoch { get; private set; }

    public string StatusText { get; private set; } = string.Empty;

    public string TurnText { get; private set; } = string.Empty;

    public string SearchInfoText { get; private set; } = string.Empty;

    public string ResultMessage { get; private set; } = string.Empty;

    /// <summary>Machine-readable result for automated tests ("" until the game ends).</summary>
    public string ResultToken { get; private set; } = string.Empty;

    public event Action<Move, Piece?>? MoveExecuted;

    public event Action<GameStatus>? GameEnded;

    // --- Game lifecycle ------------------------------------------------------------------

    public void StartNewGame()
    {
        CancelAiSearch();
        // On a genuine game reset, abandon any in-flight Rust search by
        // restarting the worker (a rare event, so the tablebase reload cost is
        // acceptable here — unlike the per-move CancelAiSearch path).
        Rust?.Reset();
        GameEpoch++;
        State = GameState.CreateInitial();
        _trackedMoves = new List<Move>();
        SearchInfoText = string.Empty;
        ResultMessage = string.Empty;
        ResultToken = string.Empty;
        ClearSelection();
        ClearTargets();
        LastMoveFrom = LastMoveTo = null;
        LastMoveWasCapture = false;
        RebuildPieceIds();
        LayoutEpoch++;
        UpdateStatus();
    }

    public void ToggleFlip()
    {
        IsFlipped = !IsFlipped;
        // Flipping only remaps display coordinates; rebuild the piece layer without slides.
        LayoutEpoch++;
    }

    /// <summary>
    /// Web-only addition (the desktop has no undo). Takes back the last move; in modes with
    /// a human it also undoes the AI's reply so it is the human's turn again. Replays the
    /// tracked move list against the deterministic, pure GameController.ApplyMove.
    /// </summary>
    public bool Undo()
    {
        if (IsBusy || TrackedMoves.Count == 0)
        {
            return false;
        }

        ReplayToMoveCount(TrackedMoves.Count - 1);
        if (HasHumanPlayer() && !IsHumanTurn() && TrackedMoves.Count > 0)
        {
            ReplayToMoveCount(TrackedMoves.Count - 1);
        }

        ClearSelection();
        ClearTargets();
        RebuildPieceIds();
        LayoutEpoch++;
        RestoreLastMoveHighlight();
        SearchInfoText = string.Empty;
        UpdateStatus();
        return true;
    }

    public void CancelAiSearch()
    {
        AiCts?.Cancel();
        AiCts?.Dispose();
        AiCts = null;
        // NOTE: the Rust worker is NOT reset here. This is called before every
        // AI move, and terminating the worker would force the ~31 MB tablebase
        // (gzip decompress + SHA-256) to reload on every single move — a ~1.4 s
        // per-move latency tax. Stale-search abandonment is already handled by
        // the page's GameEpoch guard, so the worker stays alive across moves.
    }

    // --- Interaction ---------------------------------------------------------------------

    public ClickOutcome OnSquareClicked(Position position)
    {
        if (State.Status != GameStatus.InProgress || IsBusy || !IsHumanTurn())
        {
            return ClickOutcome.Ignored;
        }

        var piece = State.GetPieceAt(position);

        if (Selected is { } selected)
        {
            if (Targets.Contains(position))
            {
                ApplyHumanMove(new Move(selected, position, piece));
                return ClickOutcome.Moved;
            }

            if (piece is { } p && p.Owner == State.CurrentTurn)
            {
                SelectSquare(position);
                return ClickOutcome.Selected;
            }

            ClearSelection();
            ClearTargets();
            return ClickOutcome.Deselected;
        }

        if (piece is { } p2 && p2.Owner == State.CurrentTurn)
        {
            SelectSquare(position);
            return ClickOutcome.Selected;
        }

        return ClickOutcome.Ignored;
    }

    // --- Turn helpers (used by the page's AI loop) ---------------------------------------

    /// <summary>The human plays the side of the first move: Blue when human-first,
    /// Red when AI-first (the desktop StartDialog behavior; Blue always opens).</summary>
    private Player HumanSide => _firstMove == FirstMoveChoice.HumanFirst ? Player.Blue : Player.Red;

    public bool IsHumanTurn()
    {
        if (State.Status != GameStatus.InProgress || _gameMode == GameMode.AiVsAi)
        {
            return false;
        }

        return State.CurrentTurn == HumanSide;
    }

    public bool ShouldAiMove()
    {
        if (State.Status != GameStatus.InProgress)
        {
            return false;
        }

        return _gameMode == GameMode.AiVsAi || State.CurrentTurn != HumanSide;
    }

    public bool HasHumanPlayer() => _gameMode != GameMode.AiVsAi;

    /// <summary>Formats the search stats of the last completed AI search.
    /// The engine exposes depth and nodes only; time is measured by the page loop and
    /// nps is derived here (the engine is copied verbatim — no score is available).</summary>
    public void UpdateSearchInfo(int depth, long nodes, long elapsedMs)
    {
        var nps = elapsedMs > 0 ? nodes * 1000 / elapsedMs : 0;
        SearchInfoText = $"depth {depth} · {nodes:N0} nodes · {elapsedMs} ms · {nps:N0} nps";
    }

    /// <summary>Move history for the side panel: number, mover, from→to, capture mark.
    /// TrackedMoves starts with Blue and alternates; Blue is always the opening side.</summary>
    public IReadOnlyList<string> BuildMoveHistory()
    {
        var lines = new List<string>(TrackedMoves.Count);
        for (var i = 0; i < TrackedMoves.Count; i++)
        {
            var move = TrackedMoves[i];
            var mover = i % 2 == 0 ? Player.Blue : Player.Red;
            var marker = mover == Player.Blue ? "🔵" : "🔴";
            var capture = move.IsCapture ? " ×" : string.Empty;
            lines.Add($"{i + 1,3}. {marker} {BoardCoordinates.SquareName(move.From)}→{BoardCoordinates.SquareName(move.To)}{capture}");
        }

        return lines;
    }

    public void UpdateStatus()
    {
        if (State.Status != GameStatus.InProgress)
        {
            StatusText = GameStrings.StatusText(State.Status);
            TurnText = "Game over";
            return;
        }

        var turn = State.CurrentTurn;
        StatusText = IsBusy ? $"AI ({turn}) is thinking…" : $"{turn} to move";
        TurnText = $"{turn} · move {TrackedMoves.Count + 1}";
    }

    // --- AI move application (called by the page's AI loop on the renderer thread) -------

    /// <summary>
    /// Applies the move the AI chose. A null move can only mean the side to move has no
    /// legal moves; mirror the desktop DoAIMove defensive mapping and end the game rather
    /// than leaving it hung.
    /// </summary>
    public void ApplyAiMove(Move? move)
    {
        if (move is null)
        {
            GameEnded?.Invoke(State.CurrentTurn == Player.Blue ? GameStatus.RedWins : GameStatus.BlueWins);
            return;
        }

        State = GameController.ApplyMove(State, move.Value);
        _trackedMoves.Add(move.Value);
        OnMoveApplied(move.Value);
    }

    // --- Internals -----------------------------------------------------------------------

    private void ApplyHumanMove(Move move)
    {
        State = GameController.ApplyMove(State, move);
        _trackedMoves.Add(move);
        OnMoveApplied(move);
        ClearSelection();
        ClearTargets();
    }

    private void OnMoveApplied(Move move)
    {
        // Incremental piece-id update so the moved piece keeps its DOM node (slide animation);
        // a captured piece's id is dropped with it.
        if (_pieceIds.TryGetValue(move.From, out var id))
        {
            _pieceIds.Remove(move.From);
            _pieceIds[move.To] = id;
        }

        LastMoveFrom = move.From;
        LastMoveTo = move.To;
        LastMoveWasCapture = move.IsCapture;
        if (move.IsCapture)
        {
            CaptureEpoch++;
        }
        UpdateStatus();
        MoveExecuted?.Invoke(move, move.Captured);

        if (State.Status != GameStatus.InProgress)
        {
            OnGameEnded(State.Status);
        }
    }

    private void OnGameEnded(GameStatus status)
    {
        UpdateStatus();
        ResultMessage = GameStrings.StatusText(status);
        ResultToken = status switch
        {
            GameStatus.BlueWins => "blue-wins",
            GameStatus.RedWins => "red-wins",
            GameStatus.Draw => "draw",
            _ => string.Empty,
        };
        GameEnded?.Invoke(status);
    }

    private void ReplayToMoveCount(int moveCount)
    {
        var state = GameState.CreateInitial();
        for (var i = 0; i < moveCount; i++)
        {
            // Replayed positions are never terminal (the truncated tail held the moves
            // that ended the game), so ApplyMove cannot throw here.
            state = GameController.ApplyMove(state, TrackedMoves[i]);
        }

        State = state;
        _trackedMoves = _trackedMoves.Take(moveCount).ToList();
    }

    private void SelectSquare(Position position)
    {
        Selected = position;
        Targets.Clear();
        foreach (var move in MoveGenerator.GenerateLegalMoves(State, State.CurrentTurn))
        {
            if (move.From == position)
            {
                Targets.Add(move.To);
            }
        }
    }

    private void ClearSelection() => Selected = null;

    private void ClearTargets() => Targets.Clear();

    private void RebuildPieceIds()
    {
        _pieceIds.Clear();
        _nextPieceId = 0;
        foreach (var position in State.Pieces.Keys)
        {
            _pieceIds[position] = _nextPieceId++;
        }
    }

    private void RestoreLastMoveHighlight()
    {
        if (TrackedMoves.Count > 0)
        {
            var last = TrackedMoves[^1];
            LastMoveFrom = last.From;
            LastMoveTo = last.To;
            LastMoveWasCapture = last.IsCapture;
        }
        else
        {
            LastMoveFrom = LastMoveTo = null;
            LastMoveWasCapture = false;
        }
    }

    /// <summary>Desktop GameStrings single source of finished-game text.</summary>
    private static class GameStrings
    {
        public static string StatusText(GameStatus status) => status switch
        {
            GameStatus.BlueWins => "Blue wins!",
            GameStatus.RedWins => "Red wins!",
            GameStatus.Draw => "Draw",
            _ => string.Empty,
        };
    }
}
