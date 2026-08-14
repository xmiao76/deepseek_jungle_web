using JungleGame.Core.AI;
using JungleGame.Core.Model;
using Xunit;

namespace JungleGame.Tests.AI;

/// <summary>
/// Pins for the P5 candidate set (SEE quiescence ordering, counter-move heuristic,
/// T1-T3 evaluation terms). Each eval pin compares the P5 evaluation against the
/// legacy (legacyP5: true) evaluation on a position where only the term under test
/// fires, so the delta is exactly the term's weight.
/// </summary>
public class P5FeatureTests
{
    private static GameState CreateState(Dictionary<Position, Piece> pieces)
        => new(
            Board.Initial,
            System.Collections.Immutable.ImmutableDictionary.CreateRange(pieces),
            Player.Blue,
            GameStatus.InProgress,
            System.Collections.Immutable.ImmutableList<Piece>.Empty,
            System.Collections.Immutable.ImmutableList<Piece>.Empty);

    private static (int P5, int Legacy) EvaluatePair(GameState state)
    {
        var board = SearchBoard.FromGameState(state);
        int mobility0 = board.CountLegalMoves(0);
        int mobility1 = board.CountLegalMoves(1);
        int p5 = EvaluationFunction.Evaluate(board, 0, mobility0, mobility1, legacyEval: false, legacyP5: false);
        int legacy = EvaluationFunction.Evaluate(board, 0, mobility0, mobility1, legacyEval: false, legacyP5: true);
        return (p5, legacy);
    }

    [Fact]
    public void RatNearEnemyDen_EarnsInfluenceBonus_Of15PerStep()
    {
        // Blue Rat at (3,6) is 2 steps from Red's den (3,8): T1 adds (3-2)*15 = +15.
        // No other piece sits on a front trap or has a jump lane, so the delta is
        // exactly T1's contribution.
        var state = CreateState(new Dictionary<Position, Piece>
        {
            [new Position(3, 6)] = new Piece(Animal.Rat, Player.Blue, new Position(3, 6)),
            [new Position(0, 8)] = new Piece(Animal.Cat, Player.Red, new Position(0, 8))
        });

        var (p5, legacy) = EvaluatePair(state);

        Assert.Equal(legacy + 15, p5);
    }

    [Fact]
    public void PieceOnOwnFrontTrap_EarnsGuardBonus_Of20()
    {
        // Blue Dog on Blue's own front trap (3,1) = sq 10: T2 adds +20.
        var state = CreateState(new Dictionary<Position, Piece>
        {
            [new Position(3, 1)] = new Piece(Animal.Dog, Player.Blue, new Position(3, 1)),
            [new Position(0, 8)] = new Piece(Animal.Cat, Player.Red, new Position(0, 8))
        });

        var (p5, legacy) = EvaluatePair(state);

        Assert.Equal(legacy + 20, p5);
    }

    [Fact]
    public void LionWithClearJumpLane_EarnsLaneBonus_Of12()
    {
        // Blue Lion at (1,2) has a rat-free row-changing jump across the river to
        // (1,6) — the enemy half: T3 adds +12. (The pre-P5 JumpPathBonus also fires
        // here, but identically in both evaluations, so it cancels in the delta.)
        var state = CreateState(new Dictionary<Position, Piece>
        {
            [new Position(1, 2)] = new Piece(Animal.Lion, Player.Blue, new Position(1, 2)),
            [new Position(0, 8)] = new Piece(Animal.Cat, Player.Red, new Position(0, 8))
        });

        var (p5, legacy) = EvaluatePair(state);

        Assert.Equal(legacy + 12, p5);
    }

    [Fact]
    public void ClearJumpLane_IsBlockedByRat_NoBonus()
    {
        // Same Lion, but a Red Rat occupies a crossed water square (1,4): the jump
        // is blocked, T3 must not fire.
        var state = CreateState(new Dictionary<Position, Piece>
        {
            [new Position(1, 2)] = new Piece(Animal.Lion, Player.Blue, new Position(1, 2)),
            [new Position(1, 4)] = new Piece(Animal.Rat, Player.Red, new Position(1, 4)),
            [new Position(0, 8)] = new Piece(Animal.Cat, Player.Red, new Position(0, 8))
        });

        var (p5, legacy) = EvaluatePair(state);

        Assert.Equal(legacy, p5);
    }

    [Theory]
    [InlineData(Animal.Lion, 1, 2, 1, 6, true)]    // lion row-change jump across the river
    [InlineData(Animal.Tiger, 1, 2, 1, 6, false)]  // tiger may not jump row-changing
    [InlineData(Animal.Cat, 1, 2, 1, 6, false)]    // only lion/tiger jump at all
    [InlineData(Animal.Lion, 1, 2, 1, 7, false)]   // no jump lands on (1,7)
    public void CanJumpTo_MirrorsJumpRules(Animal animal, int fc, int fr, int tc, int tr, bool expected)
    {
        var state = CreateState(new Dictionary<Position, Piece>
        {
            [new Position(fc, fr)] = new Piece(animal, Player.Blue, new Position(fc, fr)),
            [new Position(0, 8)] = new Piece(Animal.Cat, Player.Red, new Position(0, 8))
        });
        var board = SearchBoard.FromGameState(state);
        byte id = board.Occupant(fr * 7 + fc);

        Assert.Equal(expected, SearchBoard.CanJumpTo(board, id, fr * 7 + fc, tr * 7 + tc));
    }

    [Fact]
    public void CanJumpTo_RatOnPath_BlocksJump()
    {
        var state = CreateState(new Dictionary<Position, Piece>
        {
            [new Position(1, 2)] = new Piece(Animal.Lion, Player.Blue, new Position(1, 2)),
            [new Position(1, 4)] = new Piece(Animal.Rat, Player.Red, new Position(1, 4)),
            [new Position(0, 8)] = new Piece(Animal.Cat, Player.Red, new Position(0, 8))
        });
        var board = SearchBoard.FromGameState(state);
        byte id = board.Occupant(1 * 7 + 2);

        Assert.False(SearchBoard.CanJumpTo(board, id, 15, 43)); // (1,2) -> (1,6)
    }

    [Fact]
    public void See_SimpleCapture_ValuesVictimMinusRecapture()
    {
        // Blue Dog captures a Red Cat with no recapture: SEE = +200.
        var state = CreateState(new Dictionary<Position, Piece>
        {
            [new Position(3, 4)] = new Piece(Animal.Dog, Player.Blue, new Position(3, 4)),
            [new Position(3, 5)] = new Piece(Animal.Cat, Player.Red, new Position(3, 5)),
            [new Position(0, 8)] = new Piece(Animal.Rat, Player.Red, new Position(0, 8)),
            [new Position(0, 0)] = new Piece(Animal.Rat, Player.Blue, new Position(0, 0))
        });
        var board = SearchBoard.FromGameState(state);
        byte dog = board.Occupant(4 * 7 + 3);   // (3,4)
        byte cat = board.Occupant(5 * 7 + 3);   // (3,5)

        Assert.Equal(200, SeeEvaluator.See(board, dog, cat, 5 * 7 + 3));
    }

    [Fact]
    public void See_LosingExchange_IsNegative()
    {
        // Blue Wolf captures a Red Dog that a Red Elephant recaptures next move:
        // the Wolf gains 300, then the Elephant takes the Wolf back (400):
        // SEE = 300 - 400 = -100 for Blue.
        var state = CreateState(new Dictionary<Position, Piece>
        {
            [new Position(3, 4)] = new Piece(Animal.Wolf, Player.Blue, new Position(3, 4)),
            [new Position(3, 5)] = new Piece(Animal.Dog, Player.Red, new Position(3, 5)),
            [new Position(3, 6)] = new Piece(Animal.Elephant, Player.Red, new Position(3, 6)),
            [new Position(0, 0)] = new Piece(Animal.Rat, Player.Blue, new Position(0, 0)),
            [new Position(0, 8)] = new Piece(Animal.Rat, Player.Red, new Position(0, 8))
        });
        var board = SearchBoard.FromGameState(state);
        byte wolf = board.Occupant(4 * 7 + 3); // (3,4)
        byte dog = board.Occupant(5 * 7 + 3);  // (3,5)

        Assert.Equal(-100, SeeEvaluator.See(board, wolf, dog, 5 * 7 + 3));
    }

    [Fact]
    public void See_TrappedVictim_IsWorthItsRawMaterial()
    {
        // A Red Wolf on Blue's trap (3,1) has effective rank 0: a Blue Cat captures
        // it for +400 (the trap zeroing is delegated to CanCapture/EffectiveRankOf).
        var state = CreateState(new Dictionary<Position, Piece>
        {
            [new Position(3, 2)] = new Piece(Animal.Cat, Player.Blue, new Position(3, 2)),
            [new Position(3, 1)] = new Piece(Animal.Wolf, Player.Red, new Position(3, 1)),
            [new Position(0, 0)] = new Piece(Animal.Rat, Player.Blue, new Position(0, 0)),
            [new Position(0, 8)] = new Piece(Animal.Rat, Player.Red, new Position(0, 8))
        });
        var board = SearchBoard.FromGameState(state);
        byte cat = board.Occupant(2 * 7 + 3);  // (3,2)
        byte wolf = board.Occupant(1 * 7 + 3); // (3,1)

        Assert.Equal(400, SeeEvaluator.See(board, cat, wolf, 1 * 7 + 3));
    }
}
