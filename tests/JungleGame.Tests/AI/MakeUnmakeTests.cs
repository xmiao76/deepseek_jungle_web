using JungleGame.Core.AI;
using JungleGame.Core.Engine;
using JungleGame.Core.Model;

namespace JungleGame.Tests.AI;

/// <summary>
/// Parity pins for the make/unmake board API: ApplyMove and MakeMove must produce
/// byte-identical boards (including the opponent piece-id list order, which
/// drives move-generation order and therefore the deterministic search), and
/// MakeMove+UnmakeMove must restore the pre-move board exactly.
/// </summary>
public class MakeUnmakeTests
{
    [Fact]
    public void MakeUnmake_RoundTripsEveryMove_OverFuzzedGames()
    {
        // 20 seeded random games × up to 80 plies each
        for (int g = 0; g < 20; g++)
        {
            var rng = new Random(1234 + g);
            var state = GameState.CreateInitial();
            for (int ply = 0; ply < 80 && state.Status == GameStatus.InProgress; ply++)
            {
                var legal = MoveGenerator.GenerateLegalMoves(state, state.CurrentTurn);
                if (legal.Count == 0)
                    break;

                var board = SearchBoard.FromGameState(state);
                foreach (var publicMove in legal)
                    AssertRoundTrip(board, publicMove);

                state = GameController.ApplyMove(state, legal[rng.Next(legal.Count)]);
            }
        }
    }

    [Fact]
    public void MakeUnmake_RoundTripsCapturesAndDenEntries()
    {
        // Constructed positions covering: capture, trap capture, rat-x-elephant,
        // rat in water, and den entry.
        foreach (var text in CaptureRichPositions)
        {
            var entries = TestSuiteParser.Parse("P " + text + "\nN D1");
            var state = entries[0].State;
            var board = SearchBoard.FromGameState(state);
            foreach (var move in MoveGenerator.GenerateLegalMoves(state, state.CurrentTurn))
                AssertRoundTrip(board, move);
        }
    }

    private static readonly string[] CaptureRichPositions =
    {
        // Adjacent capturable pairs (Wolf x Dog, Lion x Tiger next to trap)
        "Blue 3,2:Wolf 3,3:dog 1,1:cat 4,1:tiger 6,8:rat",
        // Rat x Elephant from land
        "Blue 3,3:Rat 3,4:elephant 0,0:cat 6,8:rat",
        // Rat in water (immune) + jump blocking
        "Blue 2,4:Rat 3,6:Lion 0,0:cat 6,8:rat",
        // Trap-square capture with a piece standing on the enemy trap
        "Blue 3,7:Wolf 2,8:tiger 0,0:cat 6,8:rat",
        // Den entry available (mate threat): Lion on (3,7) steps into the den
        "Blue 3,6:Lion 3,7:rat 0,0:cat",
    };

    private static void AssertRoundTrip(SearchBoard original, Move publicMove)
    {
        var sm = ToSearchMove(original, publicMove);

        // Path 1: ApplyMove (copy-make reference behavior)
        var applied = original.Clone();
        applied.ApplyMove(sm);

        // Path 2: MakeMove on a fresh copy
        var made = original.Clone();
        var undo = made.MakeMove(sm);

        AssertBoardsEqual(applied, made);

        // Round trip: UnmakeMove restores the original exactly
        made.UnmakeMove(sm, undo);
        AssertBoardsEqual(original, made);
    }

    private static SearchMove ToSearchMove(SearchBoard board, Move move)
    {
        byte from = (byte)(move.From.Row * 7 + move.From.Col);
        byte to = (byte)(move.To.Row * 7 + move.To.Col);
        byte capturedId = board.Occupant(to);
        bool entersDen = move.Captured == null
            && Board.Initial.IsDen(move.To, (Player)(board.Turn ^ 1));
        return new SearchMove(from, to, capturedId, entersDen);
    }

    private static void AssertBoardsEqual(SearchBoard a, SearchBoard b)
    {
        Assert.Equal(a.Hash, b.Hash);
        Assert.Equal(a.Turn, b.Turn);
        Assert.Equal(a.WinnerSide, b.WinnerSide);
        Assert.Equal(a.PieceCount(0), b.PieceCount(0));
        Assert.Equal(a.PieceCount(1), b.PieceCount(1));
        for (int sq = 0; sq < 63; sq++)
            Assert.Equal(a.Occupant(sq), b.Occupant(sq));
        for (int side = 0; side < 2; side++)
        {
            Assert.Equal(a.PieceIds(side).ToArray(), b.PieceIds(side).ToArray()); // id-list ORDER matters
            foreach (byte id in a.PieceIds(side))
                Assert.Equal(a.SquareOf(id), b.SquareOf(id));
        }
    }
}
