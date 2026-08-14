using Microsoft.JSInterop;

namespace JungleGame.Web.Services;

/// <summary>
/// Thin wrapper over the Web Audio synth in <c>wwwroot/js/app.js</c>. Sounds are synthesized
/// in the browser (the desktop app has no audio assets to reuse). Playback failures (e.g. the
/// JS module not ready during boot) are swallowed: audio must never break the game.
/// </summary>
public sealed class SoundPlayer
{
    private readonly IJSRuntime _js;

    public SoundPlayer(IJSRuntime js) => _js = js;

    public bool IsMuted { get; private set; }

    public void SetMuted(bool muted)
    {
        IsMuted = muted;
        _ = InvokeAsync("setMuted", muted);
    }

    public Task PlayAsync(string kind) => IsMuted ? Task.CompletedTask : InvokeAsync("play", kind);

    private async Task InvokeAsync(string function, params object?[] args)
    {
        try
        {
            await _js.InvokeVoidAsync($"jungleSound.{function}", args);
        }
        catch (Exception ex) when (ex is JSException or InvalidOperationException or TaskCanceledException)
        {
            // Audio is best-effort; never let it break gameplay.
        }
    }
}
