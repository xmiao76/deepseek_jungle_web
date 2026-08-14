using JungleGame.Core.AI;
using JungleGame.Core.Model;
using JungleGame.Web.Models;
using JungleGame.Web.Services;
using Xunit;

namespace JungleGame.Web.Tests;

public class GameServiceTests
{
    [Fact]
    public void Defaults_MatchDesktopStartDialog_HumanFirstMediumHumanVsAi()
    {
        var service = new GameService();

        Assert.Equal(GameMode.HumanVsAi, service.GameMode);
        Assert.Equal(FirstMoveChoice.HumanFirst, service.FirstMove);
        Assert.Equal(Difficulty.Medium, service.Difficulty);
        Assert.Equal(GameStatus.InProgress, service.State.Status);
        Assert.Equal(16, service.State.Pieces.Count);
        Assert.Equal(Player.Blue, service.State.CurrentTurn); // Blue always opens
        Assert.True(service.IsHumanTurn()); // human-first -> the human plays Blue
        Assert.False(service.ShouldAiMove());
    }

    [Fact]
    public void AiFirst_HumanPlaysRed_AiMovesFirst()
    {
        var service = new GameService { FirstMove = FirstMoveChoice.AiFirst };

        Assert.False(service.IsHumanTurn());
        Assert.True(service.ShouldAiMove());
        Assert.Equal(Player.Blue, service.State.CurrentTurn); // the AI opens with Blue
    }

    [Fact]
    public void AiVsAi_NeitherSideIsHuman_BothShouldAiMove()
    {
        var service = new GameService { GameMode = GameMode.AiVsAi };

        Assert.False(service.HasHumanPlayer());
        Assert.False(service.IsHumanTurn());
        Assert.True(service.ShouldAiMove());
    }

    [Fact]
    public void ModeOrFirstMoveChange_StartsNewGame()
    {
        var service = new GameService();
        var initialState = service.State;

        service.GameMode = GameMode.AiVsAi;

        Assert.NotSame(initialState, service.State);
        Assert.Empty(service.TrackedMoves);
    }

    [Fact]
    public void GameEpoch_IsStableAcrossMoves_AndBumpsOnNewGame()
    {
        var service = new GameService();
        var epoch = service.GameEpoch;

        service.OnSquareClicked(new Position(1, 1));
        service.OnSquareClicked(new Position(1, 2));
        Assert.Equal(epoch, service.GameEpoch); // moves do not change the epoch

        service.StartNewGame();
        Assert.True(service.GameEpoch > epoch); // the AI loop's staleness pin
    }

    [Fact]
    public void DifficultyChange_KeepsGameRunning_UpdatesTimeLimit()
    {
        var service = new GameService();
        var state = service.State;

        service.Difficulty = Difficulty.Hard;

        Assert.Same(state, service.State);
        Assert.Equal(Difficulty.Hard, service.Difficulty);
        // The engine accepts the new limit without throwing (SetTimeLimit is exercised).
        Assert.IsType<MinimaxEngine>(service.Ai);
    }

    [Fact]
    public void Click_SelectThenMove_AppliesHumanMove()
    {
        var service = new GameService();

        // Blue cat at (1,1) is a legal first selection.
        Assert.Equal(ClickOutcome.Selected, service.OnSquareClicked(new Position(1, 1)));
        Assert.Equal(new Position(1, 1), service.Selected);
        Assert.Contains(new Position(1, 2), service.Targets);

        // Clicking an empty non-target square deselects.
        Assert.Equal(ClickOutcome.Deselected, service.OnSquareClicked(new Position(0, 3)));

        // Select again and move the cat one step forward.
        Assert.Equal(ClickOutcome.Selected, service.OnSquareClicked(new Position(1, 1)));
        Assert.Equal(ClickOutcome.Moved, service.OnSquareClicked(new Position(1, 2)));

        Assert.Equal(new Position(1, 2), service.State.GetPieceAt(new Position(1, 2))?.Position);
        Assert.Single(service.TrackedMoves);
        Assert.True(service.ShouldAiMove()); // now it is the AI's turn
        Assert.False(service.IsHumanTurn());
    }

    [Fact]
    public void Click_OnOpponentPieceOrOutsideTargets_IsIgnored()
    {
        var service = new GameService();

        // Red piece: not the human's side in the default game.
        Assert.Equal(ClickOutcome.Ignored, service.OnSquareClicked(new Position(1, 7)));
    }

    [Fact]
    public void ApplyAiMove_AdvancesState_AndTracksMove()
    {
        var service = new GameService { FirstMove = FirstMoveChoice.AiFirst };
        var move = service.Ai.FindBestMove(service.State, CancellationToken.None);

        service.ApplyAiMove(move);

        Assert.NotNull(move);
        Assert.Single(service.TrackedMoves);
        Assert.Equal(Player.Red, service.State.CurrentTurn);
        Assert.True(service.IsHumanTurn()); // AI opened with Blue; now the human (Red) moves
    }

    [Fact]
    public void ApplyAiMove_NullMove_EndsGameForSideToMove()
    {
        var service = new GameService { FirstMove = FirstMoveChoice.AiFirst };
        GameStatus? ended = null;
        service.GameEnded += status => ended = status;

        service.ApplyAiMove(null);

        Assert.NotNull(ended); // defensive mapping: side to move loses, game never hangs
    }

    [Fact]
    public void Undo_AfterHumanMoveAndAiReply_RestoresHumanTurn()
    {
        var service = new GameService();
        Assert.False(service.Undo()); // nothing to undo yet

        service.OnSquareClicked(new Position(1, 1));
        service.OnSquareClicked(new Position(1, 2));
        Assert.Single(service.TrackedMoves);

        var reply = service.Ai.FindBestMove(service.State, CancellationToken.None);
        service.ApplyAiMove(reply);
        Assert.Equal(2, service.TrackedMoves.Count);
        Assert.True(service.IsHumanTurn()); // the AI's Red reply returns the turn to Blue (human)

        Assert.True(service.Undo());

        Assert.Empty(service.TrackedMoves);
        Assert.True(service.IsHumanTurn());
        Assert.Equal(16, service.State.Pieces.Count);
        Assert.Null(service.State.GetPieceAt(new Position(1, 2))); // cat back home
    }

    [Fact]
    public void Undo_InWatchMode_TakesBackSingleMove()
    {
        var service = new GameService { GameMode = GameMode.AiVsAi };
        var move = service.Ai.FindBestMove(service.State, CancellationToken.None);
        service.ApplyAiMove(move);
        Assert.Single(service.TrackedMoves);

        Assert.True(service.Undo());

        Assert.Empty(service.TrackedMoves);
        Assert.Equal(Player.Blue, service.State.CurrentTurn);
    }

    [Fact]
    public void Flip_IsVisualOnly_AndBumpsLayoutEpoch()
    {
        var service = new GameService();
        var state = service.State;
        var epoch = service.LayoutEpoch;

        service.ToggleFlip();

        Assert.True(service.IsFlipped);
        Assert.Same(state, service.State);
        Assert.True(service.LayoutEpoch > epoch);
    }

    [Fact]
    public void BuildMoveHistory_NumbersMovesAndMarksSides()
    {
        var service = new GameService();
        service.OnSquareClicked(new Position(1, 1));
        service.OnSquareClicked(new Position(1, 2));
        var reply = service.Ai.FindBestMove(service.State, CancellationToken.None);
        service.ApplyAiMove(reply);

        var lines = service.BuildMoveHistory();

        Assert.Equal(2, lines.Count);
        Assert.Contains("1.", lines[0]);
        Assert.Contains("🔵", lines[0]);
        Assert.Contains("B2→B3", lines[0]);
        Assert.Contains("2.", lines[1]);
        Assert.Contains("🔴", lines[1]);
    }

    [Fact]
    public void UpdateSearchInfo_FormatsDepthNodesTimeNps()
    {
        var service = new GameService();

        service.UpdateSearchInfo(depth: 7, nodes: 30_000, elapsedMs: 1_000);

        Assert.Equal("depth 7 · 30,000 nodes · 1000 ms · 30,000 nps", service.SearchInfoText);
    }

    [Fact]
    public void CapturedTrays_ReflectGameState()
    {
        var service = new GameService();

        // Trays come straight from the immutable GameState lists (captures update them).
        Assert.Empty(service.State.CapturedBlue);
        Assert.Empty(service.State.CapturedRed);
    }
}
