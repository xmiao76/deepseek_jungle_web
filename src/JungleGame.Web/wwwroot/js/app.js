// Small JS surface exposed to Blazor. Audio synthesis (Phase 5) joins this file.
window.jungleEnv = {
    // Required by the threading badge: without cross-origin isolation the page has no
    // SharedArrayBuffer and .NET WASM threading (the background-thread AI) cannot start.
    isCrossOriginIsolated: () => self.crossOriginIsolated === true,
};
