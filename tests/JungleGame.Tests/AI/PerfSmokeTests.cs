using JungleGame.Core.AI;
using JungleGame.Core.Model;
using Xunit;

namespace JungleGame.Tests.AI;

/// <summary>
/// Performance gates, tagged [Trait("Category","Perf")] for opt-in runs via
/// `dotnet test --filter "Category=Perf"`. The collection below disables
/// parallelization so the gates are not skewed by CPU contention from other
/// test classes. Recalibrated on a Debug build after the P2 speed overhaul
/// (make-unmake): ~245k nodes/s and depth 10 in 2s from the start position;
/// the floors keep headroom for slow CI runners.
/// </summary>
[CollectionDefinition("Perf", DisableParallelization = true)]
public class PerfCollection { }

[Trait("Category", "Perf")]
[Collection("Perf")]
public class PerfSmokeTests
{
    [Fact]
    public void Search_FromStartPosition_ExceedsNodeFloor()
    {
        var engine = new MinimaxEngine(TimeSpan.FromSeconds(1));
        engine.FindBestMove(GameState.CreateInitial());

        Assert.True(
            engine.NodesSearched >= 150_000,
            $"Only {engine.NodesSearched} nodes searched in 1s (floor: 150,000)");
    }

    [Fact]
    public void Search_FromStartPosition_ReachesDepth7In2s()
    {
        var engine = new MinimaxEngine(TimeSpan.FromSeconds(2));
        engine.FindBestMove(GameState.CreateInitial());

        Assert.True(
            engine.LastCompletedDepth >= 8,
            $"Only reached depth {engine.LastCompletedDepth} in 2s (floor: 8)");
    }
}
