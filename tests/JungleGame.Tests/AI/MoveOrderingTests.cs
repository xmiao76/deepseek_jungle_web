using JungleGame.Core.AI;
using JungleGame.Core.Engine;
using JungleGame.Core.Model;

namespace JungleGame.Tests.AI;

/// <summary>
/// Pins the extracted move-ordering helpers: TT move first, MVV-LVA captures,
/// killer/counter/history bonuses for quiets, den entries ranked first in
/// qsearch, and the SEE score array staying aligned with the sorted moves.
/// </summary>
public class MoveOrderingTests
{
    private const int MaxPly = 64;

    private static (MoveOrdering Order, int[,] Killers, int[,] History) NewOrder()
    {
        var killers = new int[MaxPly, 2];
        var history = new int[Zobrist.PositionCount, Zobrist.PositionCount];
        return (new MoveOrdering(killers, history, new int[1 << 12], new int[1 << 12], legacyP5: false, legacyP6: false), killers, history);
    }

    [Fact]
    public void OrderMoves_PutsTheTtMoveFirst()
    {
        var (order, _, _) = NewOrder();
        var board = SearchBoard.FromGameState(GameState.CreateInitial());
        var moves = GenerateAll(board);

        var ttMove = new Move(new Position(0, 2), new Position(0, 3)); // Elephant a3→a4
        order.OrderMoves(board, moves, moves.Length, ttMove, ply: 1, prevPacked: -1);

        Assert.Equal(Sq(0, 2), moves[0].From);
        Assert.Equal(Sq(0, 3), moves[0].To);
    }

    [Fact]
    public void OrderMoves_CapturesRankByVictimMinusAttacker()
    {
        var (order, _, _) = NewOrder();
        var entries = TestSuiteParser.Parse("""
            P Blue 3,2:Wolf 3,3:dog 6,3:Cat 6,4:rat 6,8:tiger
            N D1
            """);
        var board = SearchBoard.FromGameState(entries[0].State);
        var moves = GenerateAll(board);

        order.OrderMoves(board, moves, moves.Length, default, ply: 1, prevPacked: -1);

        // Captures come first: Wolf x Dog (victim 3 ×100 − attacker 4 = 296)
        // must precede Cat x Rat (1×100 − 2 = 98)
        var captures = moves.TakeWhile(m => m.IsCapture).ToArray();
        Assert.True(captures.Length >= 2, $"expected 2 captures, got {captures.Length}");
        int wolfIdx = Array.FindIndex(captures, m => m.From == Sq(3, 2) && m.To == Sq(3, 3));
        int catIdx = Array.FindIndex(captures, m => m.From == Sq(6, 3) && m.To == Sq(6, 4));
        Assert.True(wolfIdx >= 0 && catIdx >= 0 && wolfIdx < catIdx);
    }

    [Fact]
    public void SortCaptures_DenEntriesRankFirst()
    {
        var (order, _, _) = NewOrder();
        var entries = TestSuiteParser.Parse("""
            P Blue 3,7:Lion 2,7:Cat 2,6:rat 6,8:wolf
            N D1
            """);
        var board = SearchBoard.FromGameState(entries[0].State);
        var moves = new SearchMove[SearchBoard.MaxMovesPerPly];
        int count = board.GenerateCaptures(board.Turn, moves);

        order.SortCaptures(board, moves, count);

        // Den entry (3,7 → 3,8) outranks the Cat x Rat capture
        Assert.True(count >= 2, $"expected den entry + capture, got {count} moves");
        Assert.False(moves[0].IsCapture);
        Assert.Equal(Sq(3, 7), moves[0].From);
        Assert.Equal(Sq(3, 8), moves[0].To);
    }

    [Fact]
    public void OrderCapturesBySee_AlignsSeeScoresWithSortedMoves()
    {
        var (order, _, _) = NewOrder();
        // Winning capture (Lion x Tiger) and losing capture (Wolf x protected Dog)
        var entries = TestSuiteParser.Parse("""
            P Blue 3,2:Wolf 3,3:dog 3,4:lion 1,3:Lion 1,4:tiger 6,8:rat
            N D1
            """);
        var board = SearchBoard.FromGameState(entries[0].State);
        var moves = new SearchMove[SearchBoard.MaxMovesPerPly];
        int count = board.GenerateCaptures(board.Turn, moves);

        order.OrderCapturesBySee(board, moves, count);

        for (int i = 0; i < count; i++)
        {
            var move = moves[i];
            int expected = move.IsCapture
                ? SeeEvaluator.See(board, board.Occupant(move.From), move.CapturedId, move.To)
                : int.MaxValue;
            Assert.Equal(expected, order.SeeScores[i]);
        }
    }

    [Fact]
    public void OrderMoves_KillerMovesOutrankHistory()
    {
        var (order, killers, history) = NewOrder();
        var board = SearchBoard.FromGameState(GameState.CreateInitial());
        var moves = GenerateAll(board);

        // Quiet moves from the start: Elephant a3→a4 as killer 1; Wolf c3→b3 history
        killers[1, 0] = Sq(0, 2) | (Sq(0, 3) << 6);
        history[Sq(2, 2), Sq(1, 2)] = 400;

        order.OrderMoves(board, moves, moves.Length, default, ply: 1, prevPacked: -1);

        int killerIdx = Array.FindIndex(moves, m => m.From == Sq(0, 2) && m.To == Sq(0, 3));
        int historyIdx = Array.FindIndex(moves, m => m.From == Sq(2, 2) && m.To == Sq(1, 2));
        Assert.True(killerIdx >= 0 && historyIdx >= 0);
        Assert.True(killerIdx < historyIdx, $"killer@{killerIdx} should precede history@{historyIdx}");
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
