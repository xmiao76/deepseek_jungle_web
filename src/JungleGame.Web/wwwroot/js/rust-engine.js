// Main-thread facade for the Rust engine worker. Exposes a single async
// `search` function that Blazor calls via IJSRuntime. Returns a Promise of
// { nodes, depth, from, to, noMove }.
(function () {
  let worker = null;
  let nextId = 1;
  const pending = new Map();

  function getWorker() {
    if (!worker) {
      worker = new Worker("js/rust-engine-worker.js");
      worker.onmessage = (e) => {
        const { id } = e.data;
        const resolve = pending.get(id);
        if (resolve) {
          pending.delete(id);
          resolve(e.data);
        }
      };
      worker.onerror = (e) => {
        // Reject all pending requests on worker failure.
        for (const [id, resolve] of pending) {
          pending.delete(id);
          resolve({ id, error: e.message || "worker error" });
        }
      };
    }
    return worker;
  }

  window.rustEngine = {
    search(hex, turn, history, budgetMs, maxDepth) {
      return new Promise((resolve) => {
        const id = nextId++;
        pending.set(id, resolve);
        getWorker().postMessage({ id, hex, turn, history, budgetMs, maxDepth });
      });
    },
    // Terminate the worker (used on game reset to abandon a stale search).
    reset() {
      if (worker) {
        worker.terminate();
        worker = null;
        for (const [id, resolve] of pending) {
          pending.delete(id);
          resolve({ id, error: "reset" });
        }
      }
    },
  };
})();
