using System.Diagnostics;
using JungleGame.Core.AI;

namespace JungleGame.Tests.AI;

/// <summary>
/// Pins the cadenced time checker: sticky abort, cancellation, node budget,
/// and the mid-search time-limit change contract.
/// </summary>
public class TimeManagerTests
{
    [Fact]
    public void Check_UnderGenerousBudget_StaysFalse()
    {
        var time = new TimeManager(TimeSpan.FromSeconds(60));
        var sw = Stopwatch.StartNew();
        for (int i = 0; i < 20_000; i++)
            Assert.False(time.Check(sw, CancellationToken.None));
    }

    [Fact]
    public void Check_ExpiredBudget_AbortsAndStaysSticky()
    {
        var time = new TimeManager(TimeSpan.FromTicks(1));
        var sw = Stopwatch.StartNew();

        bool sawAbort = false;
        for (int i = 0; i < 100_000 && !sawAbort; i++)
            sawAbort = time.Check(sw, CancellationToken.None);

        Assert.True(sawAbort, "The budget must eventually trip within the cadence");
        // Sticky: a fresh stopwatch does not un-abort the search
        Assert.True(time.Check(Stopwatch.StartNew(), CancellationToken.None));
    }

    [Fact]
    public void Check_CancelledToken_AbortsImmediately()
    {
        var time = new TimeManager(TimeSpan.FromMinutes(1));
        var sw = Stopwatch.StartNew();
        using var cts = new CancellationTokenSource();
        cts.Cancel();

        Assert.True(time.Check(sw, cts.Token));
    }

    [Fact]
    public void Reset_ClearsTheAbortFlag()
    {
        var time = new TimeManager(TimeSpan.FromTicks(1));
        AbortByBudget(time);
        Assert.True(time.Aborted);

        time.Reset();
        Assert.False(time.Aborted);
    }

    [Fact]
    public void NodeBudget_AbortsAtTheRequestedCount()
    {
        var time = new TimeManager(TimeSpan.FromMinutes(1));
        time.SetNodeBudget(100);
        var sw = Stopwatch.StartNew();

        bool aborted = false;
        // The budget is observed on the 4096-node cadence; the first cadence
        // boundary past 100 nodes is the trip point.
        for (int i = 0; i < 10_000; i++)
        {
            if (time.Check(sw, CancellationToken.None))
            {
                aborted = true;
                break;
            }
        }
        Assert.True(aborted);
    }

    private static void AbortByBudget(TimeManager time)
    {
        var sw = Stopwatch.StartNew();
        for (int i = 0; i < 100_000; i++)
        {
            if (time.Check(sw, CancellationToken.None))
                return;
        }
    }

    [Fact]
    public void SetTimeLimit_ChangesTheObservedBudget()
    {
        var time = new TimeManager(TimeSpan.FromMinutes(1));
        time.SetTimeLimit(TimeSpan.FromTicks(1));
        Assert.True(time.BudgetTicks == TimeSpan.FromTicks(1).Ticks);

        var sw = Stopwatch.StartNew();
        bool sawAbort = false;
        for (int i = 0; i < 100_000 && !sawAbort; i++)
            sawAbort = time.Check(sw, CancellationToken.None);
        Assert.True(sawAbort);
    }
}
