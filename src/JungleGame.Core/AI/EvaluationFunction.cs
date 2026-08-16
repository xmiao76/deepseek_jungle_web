using JungleGame.Core.Engine;
using JungleGame.Core.Model;

namespace JungleGame.Core.AI;

/// <summary>
/// Evaluation entry point: a thin wrapper over the linear features × weights
/// form (<see cref="EvalFeatureExtractor"/> + <see cref="EvalParameters"/>).
/// The historical hand-written terms live on as the default weight vector; the
/// feature extraction reproduces the old code's scores exactly (pinned by the
/// StaticEvalReference differential test). The tuning harness adopts a fitted
/// vector via <see cref="SetParameters"/>. The legacyEval flag suppresses the
/// P3 features (doomed piece bonus, den escort); legacyP5 suppresses the P5
/// features (rat den influence, front-trap guard, clear jump lane) — both used
/// only by the A/B strength harness.
/// </summary>
public static class EvaluationFunction
{
    /// <summary>The active weight vector (Default until a tuned vector is adopted).</summary>
    private static EvalParameters _current = EvalParameters.Default;

    /// <summary>
    /// Adopts a tuned weight vector (tuning harness; affects every engine in the
    /// process — the harness runs single-configuration per process).
    /// </summary>
    internal static void SetParameters(EvalParameters parameters) => _current = parameters;

    internal static EvalParameters GetParameters() => _current;

    /// <summary>
    /// Fast-path evaluation on the internal search board. Mobility counts are supplied
    /// by the caller (the side-to-move count comes from move generation it already
    /// performed; the opponent count from one cheap SearchBoard.CountLegalMoves call).
    /// </summary>
    internal static int Evaluate(SearchBoard board, int side, int myMobility, int oppMobility, bool legacyEval = false, bool legacyP5 = false)
        => EvaluateStatic(board, side, legacyEval, legacyP5) + (myMobility - oppMobility) * _current.MobilityWeight;

    /// <summary>
    /// Everything <see cref="Evaluate"/> computes except the mobility term, which
    /// costs two move generations per call. Used for futility pruning at depth 1
    /// and for lazy mobility in the quiescence stand-pat.
    /// </summary>
    internal static int EvaluateStatic(SearchBoard board, int side, bool legacyEval = false, bool legacyP5 = false)
        => _current.Dot(EvalFeatureExtractor.ExtractStatic(board, side, legacyEval, legacyP5));

    /// <summary>
    /// Public evaluation of a game state (delegates to the fast internal path).
    /// </summary>
    public static int Evaluate(GameState state, Player player)
    {
        // Terminal scores match MinimaxEngine's mate convention; the search itself
        // handles terminal nodes before reaching this method.
        if (state.Status == GameStatus.BlueWins)
            return player == Player.Blue ? MinimaxEngine.MateScore : -MinimaxEngine.MateScore;
        if (state.Status == GameStatus.RedWins)
            return player == Player.Red ? MinimaxEngine.MateScore : -MinimaxEngine.MateScore;
        if (state.Status == GameStatus.Draw)
            return 0;

        var board = SearchBoard.FromGameState(state);
        int side = (int)player;
        return Evaluate(board, side, board.CountLegalMoves(side), board.CountLegalMoves(side ^ 1));
    }
}
