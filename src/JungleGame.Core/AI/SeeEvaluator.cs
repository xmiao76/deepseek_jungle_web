namespace JungleGame.Core.AI;

/// <summary>
/// Static exchange evaluation for captures, in centipawns (rank × 100 scale), from the
/// side-to-move's perspective. A value &gt; 0 wins material; &lt;= 0 is an even or losing
/// exchange. All Jungle capture rules are delegated to <see cref="SearchBoard.CanCapture"/>
/// and <see cref="SearchBoard.EffectiveRankOf"/>: trap rank-zeroing (both ways),
/// rat-captures-elephant-from-land, elephant-never-captures-rat, rat-vs-rat unconditional,
/// rat-in-water immunity. Den entries never reach SEE (they are not captures).
///
/// The board is never mutated: the "piece on the exchanged square" is threaded through
/// the recursion explicitly, while every other piece stays on its real square.
/// </summary>
internal static class SeeEvaluator
{
    private const int MaxSeePly = 6;

    /// <summary>Convenience overload: SEE for a generated capture move.</summary>
    internal static int See(SearchBoard board, in SearchMove move) =>
        See(board, board.Occupant(move.From), move.CapturedId, move.To);

    /// <summary>
    /// Value of <paramref name="attackerId"/> (the side to move) capturing
    /// <paramref name="victimId"/> on <paramref name="square"/>.
    /// </summary>
    public static int See(SearchBoard board, byte attackerId, byte victimId, int square) =>
        SearchBoard.RankOf[victimId] * 100
        - Exchange(board, square, board.Turn ^ 1, attackerId, 1);

    /// <summary>
    /// Best gain the <paramref name="side"/> pieces can extract from capturing the piece
    /// that just landed on <paramref name="square"/> (<paramref name="victimId"/>),
    /// assuming <paramref name="side"/> is to move in the exchange. Stand-pat lets a side
    /// decline a losing recapture, and the least valuable attacker is chosen naturally by
    /// the max over all attackers. Limited to <see cref="MaxSeePly"/> plies.
    /// </summary>
    private static int Exchange(SearchBoard board, int square, int side, byte victimId, int depth)
    {
        if (depth >= MaxSeePly)
            return 0;

        int best = 0; // stand pat
        foreach (byte id in board.PieceIds(side))
        {
            int from = board.SquareOf(id);
            if (!Attacks(board, id, from, square))
                continue;

            if (!SearchBoard.CanCapture(id, from, victimId, square))
                continue;

            int score = SearchBoard.RankOf[victimId] * 100
                - Exchange(board, square, side ^ 1, id, depth + 1);
            if (score > best)
                best = score;
        }
        return best;
    }

    private static bool Attacks(SearchBoard board, byte pieceId, int from, int square)
    {
        foreach (byte neighbor in SearchBoard.Neighbors[from])
        {
            if (neighbor == square)
                return true;
        }
        return SearchBoard.CanJumpTo(board, pieceId, from, square);
    }
}
