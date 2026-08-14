namespace JungleGame.Web.Models;

/// <summary>The two play configurations, matching the desktop StartDialog.</summary>
public enum GameMode
{
    HumanVsAi,
    AiVsAi,
}

/// <summary>Who makes the first move of a new game (Blue always opens).</summary>
public enum FirstMoveChoice
{
    HumanFirst,
    AiFirst,
}

/// <summary>
/// The three desktop difficulty presets (JungleGame.UI StartDialog):
/// difficulty = per-move search time in milliseconds, applied via SetTimeLimit.
/// </summary>
public enum Difficulty
{
    Easy = 300,
    Medium = 1000,
    Hard = 2000,
}

public static class GameModeDisplay
{
    public static string Label(this GameMode mode) => mode switch
    {
        GameMode.HumanVsAi => "Human vs AI",
        GameMode.AiVsAi => "AI vs AI (watch)",
        _ => "?",
    };

    /// <summary>Difficulty preset labels with the per-move search budget.</summary>
    public static string Label(this Difficulty difficulty) => difficulty switch
    {
        Difficulty.Easy => $"Easy ({(int)difficulty} ms)",
        Difficulty.Medium => $"Medium ({(int)difficulty} ms)",
        Difficulty.Hard => $"Hard ({(int)difficulty} ms)",
        _ => "?",
    };

    /// <summary>
    /// The first-move choice rendered for the current mode: "Human/AI moves first" when a
    /// human faces the AI, "Blue/Red moves first" in watch mode (no human to name).
    /// </summary>
    public static string FirstMoveLabel(this FirstMoveChoice choice, GameMode mode) =>
        (choice, mode) switch
        {
            (FirstMoveChoice.HumanFirst, GameMode.HumanVsAi) => "Human moves first",
            (FirstMoveChoice.AiFirst, GameMode.HumanVsAi) => "AI moves first",
            (FirstMoveChoice.HumanFirst, GameMode.AiVsAi) => "Blue moves first",
            (FirstMoveChoice.AiFirst, GameMode.AiVsAi) => "Red moves first",
            _ => string.Empty,
        };
}
