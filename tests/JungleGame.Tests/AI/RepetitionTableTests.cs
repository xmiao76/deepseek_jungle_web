using System.Collections.Immutable;
using JungleGame.Core.AI;
using JungleGame.Core.Model;

namespace JungleGame.Tests.AI;

/// <summary>
/// Pins the repetition path table against a linear-scan reference: push/pop
/// counts, three-fold semantics, capacity, and slot reuse after deletion.
/// </summary>
public class RepetitionTableTests
{
    [Fact]
    public void PushPop_MatchesLinearScanReference()
    {
        var table = new RepetitionTable();
        var reference = new List<ulong>();

        // Seeded sequence with duplicates and low-bit collisions; the reference
        // list mirrors the engine's push/pop gating exactly (push only when
        // CanPush, pop only entries that were pushed).
        ulong seed = 0x9E3779B97F4A7C15UL;
        for (int i = 0; i < 1000; i++)
        {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            ulong h = i % 5 == 0 ? (seed & ~0x1FFUL) | (ulong)(uint)(i % 512) : seed;

            Assert.Equal(ReferenceIsRepetition(reference, h), table.IsRepetition(h));
            if (table.CanPush)
            {
                table.Push(h);
                reference.Add(h);
            }

            // Pop after every third push (keeping counts in lockstep)
            if (i % 3 == 2 && reference.Count > 0)
            {
                ulong popped = reference[^1];
                reference.RemoveAt(reference.Count - 1);
                table.Pop(popped);
            }

            Assert.Equal(ReferenceIsRepetition(reference, h), table.IsRepetition(h));
        }

        // Drain fully; every slot must read as empty afterwards
        while (reference.Count > 0)
        {
            ulong popped = reference[^1];
            reference.RemoveAt(reference.Count - 1);
            table.Pop(popped);
        }
        for (ulong i = 0; i < 512; i++)
            Assert.False(table.IsRepetition(i));
    }

    [Fact]
    public void ThreeFold_RequiresTwoPriorOccurrences()
    {
        var table = new RepetitionTable();
        ulong h = 0xABCDEF0123456789UL;

        Assert.False(table.IsRepetition(h));   // absent
        table.Push(h);
        Assert.False(table.IsRepetition(h));   // one prior occurrence
        table.Push(h);
        Assert.True(table.IsRepetition(h));    // two prior occurrences → third fold
        table.Pop(h);
        Assert.False(table.IsRepetition(h));   // back to one
        table.Pop(h);
        Assert.False(table.IsRepetition(h));   // absent again
    }

    [Fact]
    public void Capacity_BlocksPushesBeyond256()
    {
        var table = new RepetitionTable();
        for (int i = 0; i < 256; i++)
        {
            Assert.True(table.CanPush);
            table.Push((ulong)i * 0x100000001UL + 1);
        }
        Assert.False(table.CanPush);
    }

    [Fact]
    public void SeedPath_CountsDuplicateHistoryHashes()
    {
        ulong h = 0x1122334455667788UL;
        var state = StateWithHistory(new List<ulong> { h, 0x9999UL, h });

        var table = new RepetitionTable();
        table.SeedPath(state);

        Assert.True(table.IsRepetition(h));        // h appears twice in history
        Assert.False(table.IsRepetition(0x9999UL)); // once only
    }

    [Fact]
    public void SlotReuse_AfterDeletion_KeepsCountsCorrect()
    {
        var table = new RepetitionTable();

        // Colliding low 9 bits (both hash to slot 1) — a linear-probe chain
        ulong h0 = 0xAAAA_0000_0000_0001UL;
        ulong h1 = 0xBBBB_0000_0000_0201UL;
        table.Push(h0);
        table.Push(h1);
        Assert.False(table.IsRepetition(h0));
        Assert.False(table.IsRepetition(h1));

        table.Pop(h0); // backward-shift deletion must preserve h1's chain position
        Assert.False(table.IsRepetition(h1));
        table.Push(h1);
        Assert.True(table.IsRepetition(h1)); // count 2 → three-fold
        table.Pop(h1);
        table.Pop(h1);
        Assert.False(table.IsRepetition(h0));
        Assert.False(table.IsRepetition(h1));
    }

    private static GameState StateWithHistory(List<ulong> history) =>
        new GameState(
            Board.Initial,
            GameState.CreateInitial().Pieces,
            Player.Blue,
            GameStatus.InProgress,
            ImmutableList<Piece>.Empty,
            ImmutableList<Piece>.Empty,
            ImmutableList.CreateRange(history));

    private static bool ReferenceIsRepetition(List<ulong> path, ulong hash)
    {
        int occurrences = 0;
        foreach (ulong h in path)
        {
            if (h == hash && ++occurrences >= 2)
                return true;
        }
        return false;
    }
}
