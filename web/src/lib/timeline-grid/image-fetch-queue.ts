import * as R from 'remeda';
type ObjectURL = string;

type Task = {
  url: string;
  priority: number;
  abort: AbortController;
  promise: Promise<ObjectURL>;
  resolveWait: () => void;
};

class ImageFetchQueue {
  maxConcurrent: number;
  queue: Task[];
  cache: Map<string, ObjectURL>;
  maxCached: number;
  inFlight: Map<string, Task>;

  constructor(maxConcurrent = 6, maxCached = 250) {
    this.maxConcurrent = maxConcurrent;
    this.queue = [];
    this.cache = new Map();
    this.inFlight = new Map();
    this.maxCached = maxCached;
  }

  // priority = distance to viewport
  async fetch(url: string, priority: number): Promise<ObjectURL> {
    {
      const cached = this.cache.get(url);
      if (cached) {
        // simple LRU: reinsert at the back of the ordered map
        this.cache.delete(url);
        this.cache.set(url, cached);
        return cached;
      }
    }
    const pendingQueued = this.queue.find((task) => task.url === url);
    if (pendingQueued) {
      return pendingQueued.promise;
    }
    const pending = this.inFlight.get(url);
    if (pending) {
      return pending.promise;
    }

    const abort = new AbortController();

    const {
      resolve: resolveWait,
      reject: rejectWait,
      promise: waitPromise,
    } = Promise.withResolvers<void>();

    abort.signal.addEventListener(
      'abort',
      () => {
        rejectWait();
      },
      { once: true },
    );

    const promise = (async () => {
      try {
        await waitPromise;
        const response = await fetch(url, { signal: abort.signal });
        const blob = await response.blob();
        const objectUrl = URL.createObjectURL(blob);
        if (this.cache.has(url)) {
          this.cache.delete(url);
        } else if (this.cache.size >= this.maxCached) {
          const oldest = this.cache.entries().next().value;
          if (oldest) {
            URL.revokeObjectURL(oldest[1]);
            this.cache.delete(oldest[0]);
          }
        }
        this.cache.set(url, objectUrl);
        return objectUrl;
      } finally {
        this.inFlight.delete(url);
        this.tryDequeue();
      }
    })();
    const task = { url, priority, abort, promise, resolveWait };

    const insertAt = R.sortedIndexBy(this.queue, task, R.prop('priority'));
    this.queue.splice(insertAt, 0, task);
    this.tryDequeue();
    return promise;
  }

  private tryDequeue() {
    while (this.inFlight.size < this.maxConcurrent && this.queue.length > 0) {
      const task = this.queue.shift();
      if (!task || task.abort.signal.aborted) {
        continue;
      }
      this.inFlight.set(task.url, task);
      task.resolveWait();
    }
  }

  updatePriority(url: string, newPriority: number) {
    const index = this.queue.findIndex((task) => task.url === url);
    if (index < 0) {
      return;
    }
    const task = this.queue.splice(index, 1)[0];
    task.priority = newPriority;
    const insertAt = R.sortedIndexBy(this.queue, task, R.prop('priority'));
    this.queue.splice(insertAt, 0, task);
  }

  abort(url: string) {
    const running = this.inFlight.get(url);
    if (running) {
      running.abort.abort();
    } else {
      const index = this.queue.findIndex((task) => task.url === url);
      if (index >= 0) {
        const task = this.queue.splice(index, 1)[0];
        task.abort.abort();
      }
    }
  }

  clearAll() {
    for (const task of this.queue) {
      task.abort.abort();
    }
    this.queue = [];
    for (const task of this.inFlight.values()) {
      task.abort.abort();
    }
    this.inFlight.clear();
  }
}

export const imageQueue = new ImageFetchQueue(4);
