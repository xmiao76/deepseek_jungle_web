namespace JungleGame.Web;

/// <summary>App-level identity shown in the header and the credits.</summary>
public static class AppInfo
{
    /// <summary>Continues the desktop UI's version numbering (JungleGame.UI 1.1.0).</summary>
    public const string Version = "1.2.0";

    // The header badges render the ACTIVE engine dynamically via GameService
    // (EngineName/RuntimeLabel — "Rust engine · WASM · Web Worker" when the Rust
    // worker is healthy, "C# engine · .NET WASM · AOT" as the fallback). These
    // constants record the shipped defaults.
    public const string EngineName = "Rust engine";

    public const string Runtime = "WASM · Web Worker";

    public const string ModelCredit = "DeepSeek V4 Pro";

    public const string AgentCredit = "Claude Code";
}
