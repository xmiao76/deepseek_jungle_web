using JungleGame.Core.AI;
using JungleGame.Core.Engine;
using JungleGame.Core.Model;
using JungleGame.Tests.Helpers;

namespace JungleGame.Tests.AI;

/// <summary>
/// Score-identity gate for the linearized evaluation: the features × weights
/// form must reproduce the historical hand-rolled loops byte-for-byte. Fuzzes
/// seeded random games plus tactical/constructed positions, both sides, all
/// four legacyEval × legacyP5 combinations.
/// </summary>
public class LinearEvalIdentityTests
{
    [Fact]
    public void LinearEval_MatchesReference_OverFuzzedGames()
    {
        // 30 seeded random games × up to 60 plies, every position checked
        for (int g = 0; g < 30; g++)
        {
            var rng = new Random(777 + g);
            var state = GameState.CreateInitial();
            for (int ply = 0; ply < 60 && state.Status == GameStatus.InProgress; ply++)
            {
                AssertIdentity(state);
                var legal = MoveGenerator.GenerateLegalMoves(state, state.CurrentTurn);
                if (legal.Count == 0)
                    break;
                state = GameController.ApplyMove(state, legal[rng.Next(legal.Count)]);
            }
            AssertIdentity(state);
        }
    }

    [Fact]
    public void LinearEval_MatchesReference_OverConstructedPositions()
    {
        foreach (var text in ConstructedPositions)
        {
            var entries = TestSuiteParser.Parse("P " + text + "\nN D1");
            AssertIdentity(entries[0].State);
        }
    }

    private static void AssertIdentity(GameState state)
    {
        var board = SearchBoard.FromGameState(state);
        foreach (bool legacyEval in new[] { false, true })
        {
            foreach (bool legacyP5 in new[] { false, true })
            {
                for (int side = 0; side < 2; side++)
                {
                    int actual = EvaluationFunction.EvaluateStatic(board, side, legacyEval, legacyP5);
                    int expected = StaticEvalReference.EvaluateStatic(board, side, legacyEval, legacyP5);
                    Assert.Equal(expected, actual);
                }
            }
        }
    }

    private static readonly string[] ConstructedPositions =
    {
        // Start position (covered by the fuzz too, kept for a fixed anchor)
        "Blue 0,0:Tiger 6,0:Lion 1,1:Cat 5,1:Dog 0,2:Elephant 2,2:Wolf 4,2:Leopard 6,2:Rat 0,6:rat 2,6:leopard 4,6:wolf 6,6:elephant 1,7:dog 5,7:cat 0,8:lion 6,8:tiger",
        // Trapped pieces on both sides + den-race endgame
        "Blue 3,1:Wolf 3,7:lion 2,8:tiger 0,2:cat 6,8:rat",
        // Rats in water, jump blockers, elephant-rat fear
        "Blue 1,4:Rat 4,5:rat 0,1:Lion 6,0:elephant 5,2:dog",
        // Front-trap guards both sides
        "Blue 3,1:Cat 3,7:cat 0,0:Lion 6,8:lion 2,2:wolf",
        // Endgame advance + den threat + escort interplay
        "Blue 3,5:Leopard 3,4:wolf 0,0:rat 0,8:tiger",
        // Duplicate-animal position (second id range)
        "Blue 3,2:Wolf 4,2:Wolf 3,6:rat 4,6:rat",
        // Clear jump lanes: Lion and Tiger on river banks with/without rat blockers
        "Blue 0,3:Lion 6,3:Tiger 1,4:rat 5,4:rat 2,8:dog",
    };
}
