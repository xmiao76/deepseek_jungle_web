using System.Diagnostics;
using JungleGame.Core.AI;
using JungleGame.Core.Model;

namespace JungleGame.Web.Services;

/// <summary>
/// Owns the game engine and AI. The full game-flow logic (modes, first move, selection,
/// undo, move tracking) is added in the game-flow phase; the threading spike needs only
/// the background search path.
/// </summary>
public sealed class GameService
{
    private readonly MinimaxEngine _ai;

    public GameService()
    {
        State = GameState.CreateInitial();
        _ai = new MinimaxEngine(TimeSpan.FromSeconds(2));
    }

    public GameState State { get; private set; }

    public MinimaxEngine Ai => _ai;

    /// <summary>
    /// Runs one AI search off the UI thread and returns a one-line report
    /// (best move / depth / nodes / time / nps) for the spike page.
    /// </summary>
    public async Task<string> RunSearchAsync(CancellationToken cancellationToken)
    {
        var state = State;
        var sw = Stopwatch.StartNew();
        var move = await Task.Run(() => _ai.FindBestMove(state, cancellationToken), cancellationToken);
        sw.Stop();

        if (cancellationToken.IsCancellationRequested)
        {
            return "cancelled";
        }

        if (move is null)
        {
            return "no legal moves";
        }

        var nps = sw.Elapsed.TotalSeconds > 0 ? _ai.NodesSearched / sw.Elapsed.TotalSeconds : 0;
        return $"best {move.Value} · depth {_ai.LastCompletedDepth} · {_ai.NodesSearched:N0} nodes · {sw.ElapsedMilliseconds} ms · {nps:N0} nps";
    }
}
