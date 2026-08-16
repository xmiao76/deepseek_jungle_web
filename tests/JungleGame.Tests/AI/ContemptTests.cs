using JungleGame.Core.AI;
using JungleGame.Core.Model;

namespace JungleGame.Tests.AI;

/// <summary>
/// Pins the draw-score contempt policy: the side with a non-negative static
/// evaluation scores a draw as -contempt (avoids it), the worse side scores it
/// as +contempt (seeks it), and contempt 0 is exactly neutral.
/// </summary>
public class ContemptTests
{
    [Fact]
    public void DrawScore_StrongerSide_AvoidsTheDraw()
    {
        var engine = new MinimaxEngine(TimeSpan.FromSeconds(1), contempt: 30);
        var entries = TestSuiteParser.Parse("""
            P Blue 0,0:Lion 6,0:Wolf 0,8:rat
            N D1
            """);
        var board = SearchBoard.FromGameState(entries[0].State);

        // Blue (Lion + Wolf vs a Rat) evaluates clearly positive → avoids the draw
        Assert.Equal(-30, engine.DrawScore(board));
    }

    [Fact]
    public void DrawScore_WeakerSide_SeeksTheDraw()
    {
        var engine = new MinimaxEngine(TimeSpan.FromSeconds(1), contempt: 30);
        var entries = TestSuiteParser.Parse("""
            P Blue 0,0:Lion 6,0:Wolf 0,8:rat
            N D1
            """);
        var board = SearchBoard.FromGameState(entries[0].State);
        board.MakeNullMove(); // Red to move: clearly worse → seeks the draw

        Assert.Equal(30, engine.DrawScore(board));
    }

    [Fact]
    public void DrawScore_ZeroContempt_IsNeutral()
    {
        var engine = new MinimaxEngine(TimeSpan.FromSeconds(1), contempt: 0);
        var entries = TestSuiteParser.Parse("""
            P Blue 0,0:Lion 6,0:Wolf 0,8:rat
            N D1
            """);
        var board = SearchBoard.FromGameState(entries[0].State);

        Assert.Equal(0, engine.DrawScore(board));
        board.MakeNullMove();
        Assert.Equal(0, engine.DrawScore(board));
    }
}
