from __future__ import annotations

import time
from concurrent.futures import ThreadPoolExecutor
from multiprocessing import Event, cpu_count
from multiprocessing.synchronize import Event as EventClass
from queue import Empty, Queue
from typing import Callable, Generic, Hashable, TypeVar

_T = TypeVar("_T", bound=Hashable)


class _Node(Generic[_T]):
    def __init__(self, value: _T):
        self.value = value
        self.counter: int = 0
        """The number of this node's dependencies which have not yet been processed"""
        self.dependents: list[_Node[_T]] = []
        """The list of dependents which are waiting for this node to be processed"""


class DAG(Generic[_T]):
    def __init__(self, dependency_graph: dict[_T, list[_T]]):
        """
        Construct a directed acyclic graph from an adjacency list.

        The `dependency_graph` _must not_ contain any cycles.
        """

        self._nodes: dict[_T, _Node[_T]] = {}
        self._queue: list[_T] = []
        self._num_finished: int = 0

        for node, deps in dependency_graph.items():
            new_node = self._get_or_insert(node)
            new_node.counter += len(deps)
            for dep in deps:
                self._get_or_insert(dep).dependents.append(new_node)

        self._queue.extend(node.value for node in self._nodes.values() if node.counter == 0)

    def walk_parallel(self, f: Callable[[_T], None], max_tokens: int, refill_interval_s: float) -> None:
        """
        Process the graph in parallel.

        Each node in the graph is processed only once all of its dependencies have been processed.

        Concurrency is limited by the following bucket rate limiting algorithm:
        * Processing may not begin until a token can be acquired from the bucket.
        * There are at most `max_tokens - in_progress_tasks` in the bucket at any time.
        * Tokens are refreshed every `refill_interval_s`.
        """

        num_cpus = cpu_count()
        # we need one main thread + N worker threads, so `N = num_cpus - 1`,
        # and we need at least 1 worker thread.
        num_workers = num_cpus - 1 if num_cpus > 0 else 1
        with ThreadPoolExecutor(max_workers=num_workers) as p:
            task_queue: Queue[_T] = Queue()
            done_queue: Queue[_T] = Queue()
            shutdown: EventClass = Event()

            def worker(n: int) -> None:
                # Attempt to grab a task from the queue,
                # execute it, then put it in the done queue.
                while not shutdown.is_set():
                    try:
                        node = task_queue.get_nowait()
                        f(node)
                        done_queue.put(node)
                    except Empty:
                        time.sleep(0)  # yield to prevent busy-looping
                        continue

            for n in range(0, num_workers):  # start all workers
                p.submit(worker, n)

            tokens = max_tokens
            in_progress = 0
            last_refill = time.time()
            while not self._is_done():
                # This loop has two parts, `push` and `pull`.
                #
                # The `push` loop attempts to push tasks
                # onto the `task_queue` while there are
                # some tasks ready to go, and some tokens left.
                # It is also responsible for refreshing the bucket.
                #
                # The `pull` loop attempts to retrieve done
                # tasks and decrement the dependency counter on
                # their dependents.
                #
                # Once a node has no pending dependencies left,
                # it becomes ready and will be queued in one of
                # the iterations of the `push` loop.
                #
                # It's important to always use non-blocking `get`
                # with the task and done queues.
                # This is so that both the push and pull loops
                # can eventually make progress.
                while len(self._queue) > 0:  # push loop
                    now = time.time()
                    if now - last_refill > refill_interval_s:
                        tokens = max_tokens - in_progress
                        last_refill = now

                    if len(self._queue) == 0 or tokens == 0:
                        break

                    tokens -= 1
                    in_progress += 1
                    task_queue.put(self._queue.pop())

                try:
                    while True:  # pull loop
                        self._finish(done_queue.get_nowait())
                        in_progress -= 1
                except Empty:
                    time.sleep(0)  # yield here to prevent busy-looping

            shutdown.set()

    def _get_or_insert(self, node: _T) -> _Node[_T]:
        if node not in self._nodes:
            self._nodes[node] = _Node(node)
        return self._nodes[node]

    def _finish(self, node: _T) -> None:
        # mark the `node` as finished, which decrements the pending dependency counter on its dependents
        # once a node reaches `0` on its counter, it is marked ready and put in the queue for processing
        for dependent in self._nodes[node].dependents:
            dependent.counter -= 1
            if dependent.counter == 0:
                self._queue.append(dependent.value)
        self._num_finished += 1

    def _is_done(self) -> bool:
        # the number of nodes in the graph should never change
        return self._num_finished == len(self._nodes)


# example:
def main() -> None:
    def process(node: str) -> None:
        time.sleep(0.5)
        print(f"processed {node} at", time.time())

    # Tokens = 2
    # Refresh interval = 1s
    # The output should be:
    #   Processed A at T+0
    #   Processed C at T+0
    #   Processed B at T+1
    #   Processed D at T+1.5
    # `A` and `C` may swap places.
    DAG(
        {
            "A": [],
            "B": ["A"],
            "C": [],
            "D": ["A", "B", "C"],
        }
    ).walk_parallel(
        process,
        max_tokens=2,
        refill_interval_s=1,
    )


if __name__ == "__main__":
    main()
