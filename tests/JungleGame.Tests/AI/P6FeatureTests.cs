using JungleGame.Core.AI;
using JungleGame.Core.Engine;
using JungleGame.Core.Model;

namespace JungleGame.Tests.AI;

/// <summary>
/// Pins the P6 bundle pieces: the den-setup square table, SEE-sign ordering,
/// capture-history scoring, and the legacyP6 off-switch.
/// </summary>
public class P6FeatureTests
{
    [Fact]
    public void IsAdjacentToOppDen_CoversOnlyDenNeighbors()
    {
        // Blue's opponent den is (3,8) = sq 59; Red's is (3,0) = sq 3.
        Assert.True(SearchBoard.IsAdjacentToOppDen[Sq(3, 7), 0]);  // (3,7) for Blue
        Assert.True(SearchBoard.IsAdjacentToOppDen[Sq(2, 8), 0]);  // (2,8) for Blue
        Assert.True(SearchBoard.IsAdjacentToOppDen[Sq(4, 8), 0]);  // (4,8) for Blue
        Assert.False(SearchBoard.IsAdjacentToOppDen[Sq(3, 6), 0]); // (3,6): two steps away

        Assert.True(SearchBoard.IsAdjacentToOppDen[Sq(3, 1), 1]);  // (3,1) for Red
        Assert.True(SearchBoard.IsAdjacentToOppDen[Sq(2, 0), 1]);  // (2,0) for Red
        Assert.False(SearchBoard.IsAdjacentToOppDen[Sq(3, 2), 1]); // (3,2): two steps away
    }

    [Fact]
    public void SeeSign_PromotesWinningCaptureAheadOfLosingSameVictim()
    {
        var (order, _, _) = NewOrder(legacyP6: false);
        // Two Wolf x Dog captures with equal MVV-LVA: (6,2)→(6,3) is clean,
        // (3,2)→(3,3) is refuted by the Tiger recapture (SEE -300).
        var entries = TestSuiteParser.Parse("""
            P Blue 6,2:Wolf 6,3:dog 3,2:Wolf 3,3:dog 3,4:tiger 0,6:leopard
            N D1
            """);
        var board = SearchBoard.FromGameState(entries[0].State);
        var moves = GenerateAll(board);

        order.OrderMoves(board, moves, moves.Length, default, ply: 1, prevPacked: -1);

        var captures = moves.TakeWhile(m => m.IsCapture).ToArray();
        Assert.True(captures.Length >= 2, $"expected 2 captures, got {captures.Length}");
        int winning = Array.FindIndex(captures, m => m.From == Sq(6, 2) && m.To == Sq(6, 3));
        int losing = Array.FindIndex(captures, m => m.From == Sq(3, 2) && m.To == Sq(3, 3));
        Assert.True(winning >= 0 && losing >= 0);
        Assert.True(winning < losing, $"winning@{winning} must precede losing@{losing}");
    }

    [Fact]
    public void LegacyP6_KeepsPureMvvLvaOrder()
    {
        var (orderP6, _, _) = NewOrder(legacyP6: false);
        var (orderLegacy, _, _) = NewOrder(legacyP6: true);
        var entries = TestSuiteParser.Parse("""
            P Blue 6,2:Wolf 6,3:dog 3,2:Wolf 3,3:dog 3,4:tiger 0,6:leopard
            N D1
            """);
        var board = SearchBoard.FromGameState(entries[0].State);

        var movesP6 = GenerateAll(board);
        var movesLegacy = GenerateAll(board);

        orderP6.OrderMoves(board, movesP6, movesP6.Length, default, ply: 1, prevPacked: -1);
        orderLegacy.OrderMoves(board, movesLegacy, movesLegacy.Length, default, ply: 1, prevPacked: -1);

        // Equal MVV-LVA in legacy ordering: generation order wins, and the
        // losing capture (lower dictionary-order square) was generated first.
        int winningLegacy = Array.FindIndex(movesLegacy, m => m.From == Sq(6, 2) && m.To == Sq(6, 3));
        int losingLegacy = Array.FindIndex(movesLegacy, m => m.From == Sq(3, 2) && m.To == Sq(3, 3));
        Assert.Equal(0, losingLegacy);
        Assert.Equal(1, winningLegacy);

        // P6 SEE sign flips the pair.
        int winningP6 = Array.FindIndex(movesP6, m => m.From == Sq(6, 2) && m.To == Sq(6, 3));
        int losingP6 = Array.FindIndex(movesP6, m => m.From == Sq(3, 2) && m.To == Sq(3, 3));
        Assert.Equal(0, winningP6);
        Assert.Equal(1, losingP6);
    }

    [Fact]
    public void CaptureHistory_PromotesFrequentlyRefutingCaptures()
    {
        var (order, _, _) = NewOrder(legacyP6: false);
        var entries = TestSuiteParser.Parse("""
            P Blue 3,2:Wolf 3,3:dog 6,3:Cat 6,4:rat 6,8:tiger
            N D1
            """);
        var board = SearchBoard.FromGameState(entries[0].State);
        var moves = GenerateAll(board);

        // Record many cutoffs for Cat x Rat (victim rank 1): 10 × depth² = 250
        for (int i = 0; i < 10; i++)
            order.OnBetaCutoff(new SearchMove((byte)Sq(6, 3), (byte)Sq(6, 4), board.Occupant(Sq(6, 4)), false), depth: 5, ply: 1, prevPacked: -1);

        order.OrderMoves(board, moves, moves.Length, default, ply: 1, prevPacked: -1);

        int catRat = Array.FindIndex(moves, m => m.From == Sq(6, 3) && m.To == Sq(6, 4));
        int wolfDog = Array.FindIndex(moves, m => m.From == Sq(3, 2) && m.To == Sq(3, 3));
        Assert.True(catRat >= 0 && wolfDog >= 0);
        // 98 + SEE sign + 250 history > 296 + SEE sign
        Assert.True(catRat < wolfDog, $"cat@{catRat} should outrank wolf@{wolfDog} via capture history");
    }

    private static (MoveOrdering Order, int[,] Killers, int[,] History) NewOrder(bool legacyP6)
    {
        var killers = new int[64, 2];
        var history = new int[Zobrist.PositionCount, Zobrist.PositionCount];
        return (new MoveOrdering(killers, history, new int[1 << 12], new int[1 << 12], legacyP5: false, legacyP6), killers, history);
    }

    private static int Sq(int col, int row) => row * 7 + col;

    private static SearchMove[] GenerateAll(SearchBoard board)
    {
        var moves = new SearchMove[SearchBoard.MaxMovesPerPly];
        int count = board.GenerateMoves(board.Turn, moves);
        Array.Resize(ref moves, count);
        return moves;
    }
}
