using JungleGame.Core.Engine;
using JungleGame.Core.Model;

namespace JungleGame.Core.AI;

/// <summary>
/// Move scoring and ordering for the search. Scores are pure functions of the
/// (unchanging) node board and the move, so they are precomputed into a scratch
/// array before the insertion sort instead of being recomputed inside the sort
/// loop (the old O(n²) scoring calls per node).
/// </summary>
internal sealed class MoveOrdering
{
    private const int MaxPly = 64;
    private const int TtMoveBonus = 1_000_000;
    private const int KillerBonus1 = 900;
    private const int KillerBonus2 = 800;
    private const int CounterBonus1 = 700; // P5: between TT and killers
    private const int CounterBonus2 = 600;
    private const int SeeSignBonus = 50;   // P6: split winning from losing exchanges
    private const int HistoryCap = 500;

    private readonly int[,] _killers;
    private readonly int[,] _history;
    private readonly int[] _counterMove0;
    private readonly int[] _counterMove1;
    private readonly bool _legacyP5;
    private readonly bool _legacyP6; // disables the P6 bundle: SEE sign, capture history
    private readonly int[,,] _captureHistory = new int[9, Zobrist.PositionCount, Zobrist.PositionCount]; // by victim rank
    private readonly int[] _scoreScratch = new int[SearchBoard.MaxMovesPerPly];

    /// <summary>SEE scores parallel to the sorted qsearch moves (read by the pruning step).</summary>
    internal readonly int[] SeeScores = new int[SearchBoard.MaxMovesPerPly];

    internal MoveOrdering(int[,] killers, int[,] history, int[] counterMove0, int[] counterMove1, bool legacyP5, bool legacyP6)
    {
        _killers = killers;
        _history = history;
        _counterMove0 = counterMove0;
        _counterMove1 = counterMove1;
        _legacyP5 = legacyP5;
        _legacyP6 = legacyP6;
    }

    /// <summary>
    /// Records a beta-cutoff heuristic update: killers, history, counter-moves,
    /// and (P6) capture history. Never called with scores from aborted searches.
    /// </summary>
    internal void OnBetaCutoff(in SearchMove move, int depth, int ply, int prevPacked)
    {
        if (ply >= MaxPly)
            return;

        if (move.IsCapture)
        {
            if (!_legacyP6)
                _captureHistory[SearchBoard.RankOf[move.CapturedId], move.From, move.To] += depth * depth;
            return;
        }

        int packed = move.From | (move.To << 6);
        _killers[ply, 1] = _killers[ply, 0];
        _killers[ply, 0] = packed;
        _history[move.From, move.To] += depth * depth;
        if (!_legacyP5 && prevPacked >= 0)
        {
            _counterMove1[prevPacked] = _counterMove0[prevPacked];
            _counterMove0[prevPacked] = packed;
        }
    }

    /// <summary>Halve all history scores so recent refutations dominate stale ones.</summary>
    internal void AgeHistoryTable()
    {
        for (int f = 0; f < Zobrist.PositionCount; f++)
        {
            for (int t = 0; t < Zobrist.PositionCount; t++)
            {
                _history[f, t] /= 2;
                for (int r = 1; r <= 8; r++)
                    _captureHistory[r, f, t] /= 2;
            }
        }
    }

    internal void OrderMoves(SearchBoard board, SearchMove[] moves, int count, Move ttMove, int ply, int prevPacked)
    {
        for (int i = 0; i < count; i++)
            _scoreScratch[i] = MoveScore(board, moves[i], ttMove, ply, prevPacked);

        // Insertion sort by precomputed score (strict less-than keeps equal
        // scores in generation order — the old in-loop rescore was stable too)
        for (int i = 1; i < count; i++)
        {
            var move = moves[i];
            int score = _scoreScratch[i];
            int j = i - 1;
            while (j >= 0 && _scoreScratch[j] < score)
            {
                _scoreScratch[j + 1] = _scoreScratch[j];
                moves[j + 1] = moves[j];
                j--;
            }
            _scoreScratch[j + 1] = score;
            moves[j + 1] = move;
        }
    }

    internal void SortCaptures(SearchBoard board, SearchMove[] moves, int count)
    {
        for (int i = 0; i < count; i++)
            _scoreScratch[i] = CaptureScore(board, moves[i]);

        for (int i = 1; i < count; i++)
        {
            var move = moves[i];
            int score = _scoreScratch[i];
            int j = i - 1;
            while (j >= 0 && _scoreScratch[j] < score)
            {
                _scoreScratch[j + 1] = _scoreScratch[j];
                moves[j + 1] = moves[j];
                j--;
            }
            _scoreScratch[j + 1] = score;
            moves[j + 1] = move;
        }
    }

    /// <summary>
    /// P5 quiescence ordering: insertion sort by SEE value. Den entries get the
    /// sentinel int.MaxValue (first, never pruned); the parallel <see cref="SeeScores"/>
    /// array stays aligned with the sorted moves so the pruning step can read the score.
    /// </summary>
    internal void OrderCapturesBySee(SearchBoard board, SearchMove[] moves, int count)
    {
        for (int i = 0; i < count; i++)
        {
            var move = moves[i];
            SeeScores[i] = move.IsCapture
                ? SeeEvaluator.See(board, board.Occupant(move.From), move.CapturedId, move.To)
                : int.MaxValue;
        }

        for (int i = 1; i < count; i++)
        {
            var move = moves[i];
            int score = SeeScores[i];
            int j = i - 1;
            while (j >= 0 && SeeScores[j] < score)
            {
                SeeScores[j + 1] = SeeScores[j];
                moves[j + 1] = moves[j];
                j--;
            }
            SeeScores[j + 1] = score;
            moves[j + 1] = move;
        }
    }

    private int MoveScore(SearchBoard board, in SearchMove move, Move ttMove, int ply, int prevPacked)
    {
        if (ttMove.From.Row * 7 + ttMove.From.Col == move.From &&
            ttMove.To.Row * 7 + ttMove.To.Col == move.To)
            return TtMoveBonus;

        if (move.IsCapture)
        {
            // MVV-LVA ordering; P6 adds the SEE sign (winning exchanges before
            // losing ones) and capture history (how often this capture refuted).
            int victimRank = SearchBoard.RankOf[move.CapturedId];
            int attackerRank = SearchBoard.RankOf[board.Occupant(move.From)];
            int captureScore = victimRank * 100 - attackerRank;
            if (!_legacyP6)
            {
                captureScore += SeeEvaluator.See(board, move) >= 0 ? SeeSignBonus : -SeeSignBonus;
                captureScore += Math.Min(_captureHistory[victimRank, move.From, move.To], HistoryCap);
            }
            return captureScore;
        }

        int score = 0;
        int packed = move.From | (move.To << 6);

        // Killer move heuristic (full from|to move)
        if (ply < MaxPly)
        {
            if (packed == _killers[ply, 0])
                score = KillerBonus1;
            else if (packed == _killers[ply, 1])
                score = KillerBonus2;
        }

        // Counter-move heuristic (P5): quiet replies that refuted this exact previous
        // move get a bonus between the killers and the history floor.
        if (!_legacyP5 && prevPacked >= 0)
        {
            if (packed == _counterMove0[prevPacked])
                score += CounterBonus1;
            else if (packed == _counterMove1[prevPacked])
                score += CounterBonus2;
        }

        // History heuristic bonus
        score += Math.Min(_history[move.From, move.To], HistoryCap);
        return score;
    }

    private int CaptureScore(SearchBoard board, in SearchMove move)
    {
        int victimRank = move.IsCapture ? SearchBoard.RankOf[move.CapturedId] : 9; // den entries rank first
        int attackerRank = SearchBoard.RankOf[board.Occupant(move.From)];
        int score = victimRank * 10 - attackerRank;
        if (move.IsCapture && !_legacyP6)
        {
            score += SeeEvaluator.See(board, move) >= 0 ? SeeSignBonus : -SeeSignBonus;
            score += Math.Min(_captureHistory[victimRank, move.From, move.To], HistoryCap);
        }
        return score;
    }
}
