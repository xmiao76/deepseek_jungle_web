using System.Diagnostics;
using JungleGame.Core.Engine;
using JungleGame.Core.Model;

namespace JungleGame.Core.AI;

public class MinimaxEngine
{
    public const int MateScore = TranspositionTable.MateScore;
    private const int MateThreshold = TranspositionTable.MateScore - TranspositionTable.MateRange;
    private const int DefaultMaxDepth = 20;
    private const int MaxPly = 64;
    private const int MaxMovesPerPly = SearchBoard.MaxMovesPerPly;
    private const int AspirationDelta = 25;
    private const int CounterTableSize = 1 << 12; // packed from|(to<<6) fits 12 bits
    private const int FutilityMargin = 300;
    private const int DeltaMargin = 200;

    private readonly TimeManager _time;
    private readonly int _maxDepth;
    private readonly bool _legacyEval;
    private readonly bool _legacySearch;
    private readonly bool _legacyP5; // disables the P5 candidate set (SEE qsearch, counter-move, P5 eval terms)
    internal readonly bool _useTablebase; // reserved (wired in the tablebase phase)
    internal readonly int _contempt;      // reserved (wired in the time-management phase)
    private readonly TranspositionTable _tt;
    private readonly int[,] _killerMoves;
    private readonly int[,] _historyTable;
    private readonly int[] _counterMove0 = new int[CounterTableSize];
    private readonly int[] _counterMove1 = new int[CounterTableSize];
    private readonly int[] _prevMoveStack = new int[MaxPly]; // packed previous move per ply, -1 = none
    private readonly object _searchGate = new(); // serializes searches: one at a time per engine
    private readonly MoveOrdering _order;
    private readonly RepetitionTable _repetitions = new();
    private readonly SearchBoard _board = new(); // the single search board (make/unmake)

    // Per-search scratch state (single-threaded; one search at a time)
    private readonly SearchMove[][] _plyMoves;   // PVSearch buffers, one per ply
    private readonly SearchMove[][] _qMoves;     // quiescence buffers, one per ply
    private readonly SearchMove[] _rootMoves = new SearchMove[MaxMovesPerPly];

    /// <param name="legacyEval">Disables the P3 evaluation terms (A/B strength tests).</param>
    /// <param name="legacySearch">Disables the post-P3 search features (A/B strength tests).</param>
    /// <param name="legacyP5">Disables the P5 candidate set — SEE capture ordering and
    /// losing-capture pruning in quiescence, the counter-move heuristic, and the P5
    /// evaluation terms (A/B strength tests).</param>
    /// <param name="useTablebase">Reserved for the tablebase phase; no behavior yet.</param>
    /// <param name="contempt">Reserved for the time-management phase; no behavior yet.</param>
    /// <param name="maxNodes">Optional fixed-node budget per search (0/unlimited by default;
    /// used by deterministic tests).</param>
    public MinimaxEngine(
        TimeSpan? timeLimit = null,
        int? maxDepth = null,
        bool legacyEval = false,
        bool legacySearch = false,
        bool legacyP5 = false,
        bool useTablebase = true,
        int contempt = 30,
        long? maxNodes = null)
    {
        _time = new TimeManager(timeLimit ?? TimeSpan.FromSeconds(4));
        // PVSearch plus quiescence plies must fit inside the per-ply buffer tables
        _maxDepth = Math.Min(maxDepth ?? DefaultMaxDepth, MaxPly - 32);
        _legacyEval = legacyEval;
        _legacySearch = legacySearch;
        _legacyP5 = legacyP5;
        _useTablebase = useTablebase;
        _contempt = contempt;
        if (maxNodes.HasValue)
            _time.SetNodeBudget(maxNodes.Value);
        _tt = new TranspositionTable(1 << 20);
        _killerMoves = new int[MaxPly, 2];
        _historyTable = new int[Zobrist.PositionCount, Zobrist.PositionCount];
        _plyMoves = new SearchMove[MaxPly][];
        _qMoves = new SearchMove[MaxPly][];
        for (int i = 0; i < MaxPly; i++)
        {
            _plyMoves[i] = new SearchMove[MaxMovesPerPly];
            _qMoves[i] = new SearchMove[MaxMovesPerPly];
        }
        _order = new MoveOrdering(_killerMoves, _historyTable, _counterMove0, _counterMove1, legacyP5);
    }

    public long NodesSearched { get; private set; }
    public int LastCompletedDepth { get; private set; }

    /// <summary>
    /// Changes the per-move time budget (difficulty). A running search observes the
    /// new limit at its next check; the transposition table is kept, so strength
    /// carries over between games and difficulty changes.
    /// </summary>
    public void SetTimeLimit(TimeSpan timeLimit) => _time.SetTimeLimit(timeLimit);

    /// <summary>
    /// Searches for the best move. Returns null if the game is already over or the
    /// side to move has no legal moves. Cancellation stops the search promptly and
    /// returns the best completed-depth move. Searches are serialized per engine
    /// instance (the search scratch state is not thread-safe), so overlapping calls
    /// simply queue.
    /// </summary>
    public Move? FindBestMove(GameState state, CancellationToken token = default)
    {
        lock (_searchGate)
        {
            _tt.NewGeneration(); // Entries older than the previous search are ignored
            return SearchBestMove(state, token);
        }
    }

    private Move? SearchBestMove(GameState state, CancellationToken token)
    {
        if (state.Status != GameStatus.InProgress)
            return null;

        _time.Reset();
        _repetitions.Reset();
        NodesSearched = 0;
        LastCompletedDepth = 0;
        var sw = Stopwatch.StartNew();

        // The single search board is rebuilt from the game state at the start of
        // every search (an aborted search can leave it mid-tree); make/unmake
        // restores it to the root position after every completed child search.
        var root = _board;
        root.CopyFromGameState(state);

        // Seed the repetition path with the real game's history, reserving headroom
        // for the search path itself (long shuffling games can approach the cap)
        _repetitions.SeedPath(state);

        int side = (int)state.CurrentTurn;
        int moveCount = root.GenerateMoves(side, _rootMoves);
        if (moveCount == 0)
            return null;
        if (moveCount == 1)
            return ToPublicMove(_rootMoves[0]);

        SearchMove bestMove = _rootMoves[0];
        Move bestMoveForOrdering = ToPublicMove(bestMove);
        int bestScore = int.MinValue;

        for (int depth = 1; depth <= _maxDepth; depth++)
        {
            if (_time.Check(sw, token))
                break;

            int currentBest = int.MinValue;
            SearchMove currentBestMove = bestMove;

            // Aspiration window around the previous iteration's score; a fail
            // low/high re-searches the same depth with a widened window.
            int searchAlpha = bestScore == int.MinValue ? int.MinValue + 1 : bestScore - AspirationDelta;
            int searchBeta = bestScore == int.MinValue ? int.MaxValue : bestScore + AspirationDelta;

            bool retry;
            do
            {
                retry = false;
                currentBest = int.MinValue;

                _order.OrderMoves(root, _rootMoves, moveCount, bestMoveForOrdering, 0, -1);

                for (int i = 0; i < moveCount; i++)
                {
                    if (_time.Check(sw, token))
                        break;

                    var move = _rootMoves[i];
                    var undo = root.MakeMove(move);
                    if (!_legacyP5)
                        _prevMoveStack[0] = move.From | (move.To << 6);

                    int score;
                    if (i == 0)
                        // NOTE: the root PV child is searched with the UN-negated
                        // window, matching the original engine. The "correct"
                        // negated form (-searchBeta, -searchAlpha) was tried and
                        // produced draw-seeking play (see CLAUDE.md recorded
                        // results): with proper cutoffs, the narrow aspiration
                        // window stores bound entries that steer later searches
                        // toward repetition lines in equal positions.
                        score = -PVSearch(root, depth - 1, searchAlpha, searchBeta, 1, sw, token);
                    else
                    {
                        score = -PVSearch(root, depth - 1, -(currentBest + 1), -currentBest, 1, sw, token);
                        if (score > currentBest)
                            score = -PVSearch(root, depth - 1, -searchBeta, -currentBest, 1, sw, token);
                    }

                    root.UnmakeMove(move, undo);

                    if (score > currentBest)
                    {
                        currentBest = score;
                        currentBestMove = move;
                    }
                }

                if (!_time.Check(sw, token) && (currentBest <= searchAlpha || currentBest >= searchBeta))
                {
                    // Failed low/high: widen the window to full width and try the
                    // depth again (incremental widening was tried and reverted —
                    // see the root-window note above)
                    if (currentBest <= searchAlpha)
                        searchAlpha = int.MinValue + 1;
                    if (currentBest >= searchBeta)
                        searchBeta = int.MaxValue;
                    retry = true;
                }
            }
            while (retry && !_time.Check(sw, token));

            // Only accept results from fully completed depths
            if (!_time.Check(sw, token))
            {
                bestScore = currentBest;
                bestMove = currentBestMove;
                bestMoveForOrdering = ToPublicMove(currentBestMove);
                LastCompletedDepth = depth;
                AgeHistoryTable();
            }

            // A forced win was found — no deeper search needed
            if (bestScore >= MateThreshold)
                break;
        }

        return ToPublicMove(bestMove);
    }

    private int PVSearch(SearchBoard board, int depth, int alpha, int beta, int ply, Stopwatch sw, CancellationToken token)
    {
        NodesSearched++;
        if (_time.Check(sw, token))
            return 0;

        // Game ended by the previously applied move
        if (board.WinnerSide != SearchBoard.NoWinner)
            return TerminalScore(board.WinnerSide, board.Turn, ply);

        // Three-fold repetition → draw (checked before the TT probe: the hash carries
        // no repetition state, so a probed score would mask the draw)
        if (_repetitions.IsRepetition(board.Hash))
            return 0;

        // TT probe
        if (_tt.TryProbe(board.Hash, depth, alpha, beta, ply, out int ttScore, out Move ttMove))
            return ttScore;

        if (depth == 0)
            return QuiescenceSearch(board, alpha, beta, ply, sw, token);

        int side = board.Turn;

        // Null-move pruning: if the side to move can pass and still refute the
        // opponent's best try at reduced depth, this node is a beta cutoff. The
        // pass move makes the position strictly worse for the passer, so a cutoff
        // is sound — except in zugzwang, which Dou Shou Qi positions with few
        // pieces frequently are. Guards: disabled with <= 6 total pieces or <= 2
        // pieces for the side to move; a verification re-search at reduced depth
        // must confirm the cutoff. (Never applied at the root.)
        if (!_legacySearch && depth >= 3 && beta < MateThreshold
            && board.PieceCount(0) + board.PieceCount(1) > 6
            && board.PieceCount(side) > 2)
        {
            const int nullReduction = 2;
            board.MakeNullMove();
            // Null moves never create counter-move pairs: clear the slot so the child
            // sees "no previous move" instead of a stale value from a sibling node.
            if (!_legacyP5)
                _prevMoveStack[ply] = -1;
            int nullScore = -PVSearch(board, depth - 1 - nullReduction, -beta, -(beta - 1), ply + 1, sw, token);
            board.UnmakeNullMove();
            if (_time.Check(sw, token))
                return 0;

            if (nullScore >= beta)
            {
                // Verification: a reduced-depth real search of this node must
                // confirm the cutoff (catches zugzwang the guards missed)
                int verifyScore = PVSearch(board, depth - 1 - nullReduction, alpha, beta, ply, sw, token);
                if (_time.Check(sw, token))
                    return 0;
                if (verifyScore >= beta)
                    return beta;
            }
        }

        var moves = _plyMoves[ply];
        int moveCount = board.GenerateMoves(side, moves);

        // No legal moves: the side to move loses
        if (moveCount == 0)
            return -(MateScore - ply);

        // Packed previous move for the counter-move heuristic (P5); -1 at the root's
        // first ply (game history stores hashes, not moves).
        int prevPacked = !_legacyP5 && ply > 0 ? _prevMoveStack[ply - 1] : -1;
        _order.OrderMoves(board, moves, moveCount, ttMove, ply, prevPacked);

        SearchMove bestMove = moves[0];
        int bestScore = int.MinValue;
        BoundType bound = BoundType.UpperBound;
        bool anySearched = false;

        // Futility pruning at the frontier: a quiet move that cannot lift the
        // static score near alpha is skipped. Trap squares are exempt — their
        // tactical value is not captured by the static evaluation. Never applies
        // in mate-or-be-mated windows, where the only defense may be quiet moves.
        bool futilityApplicable = !_legacySearch && depth == 1
            && alpha > -MateThreshold && alpha < MateThreshold
            && board.PieceCount(0) + board.PieceCount(1) > 8;
        int staticEval = 0;
        if (futilityApplicable)
            staticEval = EvaluationFunction.EvaluateStatic(board, side, _legacyEval, _legacyP5);

        // Push the position onto the repetition path (skip if the stack is full)
        bool pathPushed = _repetitions.CanPush;
        if (pathPushed)
            _repetitions.Push(board.Hash);

        for (int i = 0; i < moveCount; i++)
        {
            if (_time.Check(sw, token))
            {
                // A partially searched node must not store anything in the TT
                break;
            }

            var move = moves[i];

            if (futilityApplicable && !move.IsCapture && !move.EntersDen
                && !SearchBoard.IsEnemyTrapSquare(move.To, side)
                && SearchBoard.EffectiveRankOf(board.Occupant(move.From), move.From) != 0
                && staticEval + FutilityMargin <= alpha)
                continue;

            anySearched = true;

            var undo = board.MakeMove(move);
            if (!_legacyP5)
                _prevMoveStack[ply] = move.From | (move.To << 6);

            int score;
            if (i == 0)
                score = -PVSearch(board, depth - 1, -beta, -alpha, ply + 1, sw, token);
            else if (i >= 3 && depth >= 3 && !move.IsCapture)
            {
                // Late move reduction: quiet moves late in the order are searched
                // shallower with a null window; re-searched if they score well.
                // The reduction scales with move index and depth; moves that enter
                // a den, and the last couple of moves (small move sets), are exempt.
                bool reduce = !_legacySearch && !move.EntersDen && i < moveCount - 2;
                int reducedDepth = reduce
                    ? Math.Max(1, depth - 1 - (1 + (i >= 6 ? 1 : 0) + (depth >= 8 ? 1 : 0)))
                    : depth - 2;

                score = -PVSearch(board, reducedDepth, -(alpha + 1), -alpha, ply + 1, sw, token);
                if (score > alpha)
                    score = -PVSearch(board, depth - 1, -(alpha + 1), -alpha, ply + 1, sw, token);
                if (score > alpha && score < beta)
                    score = -PVSearch(board, depth - 1, -beta, -alpha, ply + 1, sw, token);
            }
            else
            {
                score = -PVSearch(board, depth - 1, -(alpha + 1), -alpha, ply + 1, sw, token);
                if (score > alpha && score < beta)
                    score = -PVSearch(board, depth - 1, -beta, -alpha, ply + 1, sw, token);
            }

            board.UnmakeMove(move, undo);

            if (score > bestScore)
            {
                bestScore = score;
                bestMove = move;
            }

            if (score >= beta)
            {
                // Beta cutoff — this is a lower bound. Heuristic tables and the TT
                // must not be touched by scores from aborted searches.
                if (!move.IsCapture && ply < MaxPly && !_time.Aborted)
                {
                    int packed = move.From | (move.To << 6);
                    _killerMoves[ply, 1] = _killerMoves[ply, 0];
                    _killerMoves[ply, 0] = packed;
                    _historyTable[move.From, move.To] += depth * depth;
                    if (!_legacyP5 && prevPacked >= 0)
                    {
                        _counterMove1[prevPacked] = _counterMove0[prevPacked];
                        _counterMove0[prevPacked] = packed;
                    }
                }

                if (!_time.Aborted)
                    _tt.Store(board.Hash, depth, AdjustMateForStore(score, ply),
                        ToPublicMove(move), BoundType.LowerBound);
                if (pathPushed)
                    _repetitions.Pop(board.Hash);
                return score;
            }

            if (score > alpha)
            {
                alpha = score;
                bound = BoundType.Exact;
            }
        }

        if (pathPushed)
            _repetitions.Pop(board.Hash);

        // Every move was pruned by futility: return the static evaluation (a
        // fail-low value). Returning int.MinValue here would overflow when
        // negated by the parent and turn the line into a phantom blunder.
        if (!anySearched && !_time.Aborted)
            return staticEval;

        // An abort between the top-of-function check and the first completed
        // move leaves bestScore at int.MinValue, which would wrap on negation
        // in the parent; return a neutral value instead (the abort flag already
        // prevents any TT store and root acceptance)
        if (_time.Aborted)
            return 0;

        // A partially searched node must not pollute the table
        _tt.Store(board.Hash, depth, AdjustMateForStore(bestScore, ply),
            ToPublicMove(bestMove), bound);
        return bestScore;
    }

    private int QuiescenceSearch(SearchBoard board, int alpha, int beta, int ply, Stopwatch sw, CancellationToken token)
    {
        NodesSearched++;
        if (_time.Check(sw, token))
            return 0;

        if (board.WinnerSide != SearchBoard.NoWinner)
            return TerminalScore(board.WinnerSide, board.Turn, ply);

        int side = board.Turn;

        // Lazy mobility in the stand-pat: the full evaluation costs two move
        // generations per qnode. Most qnodes are null-window cut nodes, so the
        // mobility-free value is checked first; the full evaluation only runs
        // when that value falls inside the window.
        int standPat = _legacySearch
            ? EvaluationFunction.Evaluate(board, side, board.CountLegalMoves(side), board.CountLegalMoves(side ^ 1), _legacyEval, _legacyP5)
            : EvaluationFunction.EvaluateStatic(board, side, _legacyEval, _legacyP5);

        if (standPat >= beta)
            return beta;

        if (!_legacySearch && standPat > alpha)
        {
            standPat = EvaluationFunction.Evaluate(board, side, board.CountLegalMoves(side), board.CountLegalMoves(side ^ 1), _legacyEval, _legacyP5);
            if (standPat >= beta)
                return beta;
        }

        if (standPat > alpha)
            alpha = standPat;

        if (ply > 32) // Safety limit
            return alpha;

        // Captures plus enemy-den entries (a winning "quiet" move that must be
        // visible at the horizon)
        var moves = _qMoves[ply];
        int moveCount = board.GenerateCaptures(side, moves);

        // P5: order captures by static exchange value (more accurate than MVV-LVA);
        // den entries rank above every capture and are never SEE-pruned. The legacy
        // MVV-LVA path stays for A/B gates (--legacyP5B).
        if (!_legacyP5)
            _order.OrderCapturesBySee(board, moves, moveCount);
        else
            _order.SortCaptures(board, moves, moveCount);

        for (int i = 0; i < moveCount; i++)
        {
            if (_time.Check(sw, token))
                break;

            var move = moves[i];

            // Delta pruning: skip a capture that cannot raise the score near
            // alpha. Den entries are never pruned (winning move at the horizon).
            if (!_legacySearch && move.IsCapture)
            {
                int victimGain = SearchBoard.EffectiveRankOf(move.CapturedId, move.To) * 100;
                if (standPat + victimGain + DeltaMargin <= alpha)
                    continue;
            }

            // P5: skip strictly losing exchanges — they cannot beat the stand-pat
            // floor. Captures landing on an enemy trap are exempt: the trap zeroes
            // our piece, so the opponent's recapture is guaranteed and the plain
            // SEE recursion mis-evaluates that line. Den entries are not captures.
            if (!_legacyP5 && move.IsCapture && _order.SeeScores[i] < 0
                && !SearchBoard.IsEnemyTrapSquare(move.To, side))
                continue;

            var undo = board.MakeMove(move);
            int score = -QuiescenceSearch(board, -beta, -alpha, ply + 1, sw, token);
            board.UnmakeMove(move, undo);

            if (score >= beta)
                return beta;

            if (score > alpha)
                alpha = score;
        }

        return alpha;
    }

    /// <summary>Score of a finished game, from the perspective of the side to move.</summary>
    private static int TerminalScore(byte winnerSide, int sideToMove, int ply) =>
        winnerSide == sideToMove ? MateScore - ply : ply - MateScore;

    /// <summary>
    /// Convert a root-relative mate score into a node-relative score before storing.
    /// Clamped to ±MateScore so extreme values can never wrap during adjustment.
    /// </summary>
    private static int AdjustMateForStore(int score, int ply)
    {
        if (score > MateThreshold)
            return Math.Min(score + ply, MateScore);
        if (score < -MateThreshold)
            return Math.Max(score - ply, -MateScore);
        return score;
    }

    /// <summary>Halve all history scores so recent refutations dominate stale ones.</summary>
    private void AgeHistoryTable()
    {
        for (int f = 0; f < Zobrist.PositionCount; f++)
            for (int t = 0; t < Zobrist.PositionCount; t++)
                _historyTable[f, t] /= 2;
    }

    private static Move ToPublicMove(in SearchMove move)
    {
        var from = new Position(move.From % 7, move.From / 7);
        var to = new Position(move.To % 7, move.To / 7);
        Piece? captured = move.CapturedId == 0 ? null : new Piece(
            (Animal)((((move.CapturedId - 1) % SearchBoard.DistinctPieceKinds) >> 1) + 1),
            (Player)(((move.CapturedId - 1) % SearchBoard.DistinctPieceKinds) & 1),
            to);
        return new Move(from, to, captured);
    }
}
