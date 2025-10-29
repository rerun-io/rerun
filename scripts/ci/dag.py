from __future__ import annotations

import time
from collections.abc import Callable, Hashable
from concurrent.futures import ThreadPoolExecutor
from math import floor
from multiprocessing import Event, cpu_count
from queue import Empty, Queue
from typing import TYPE_CHECKING, Generic, TypeVar

if TYPE_CHECKING:
    from multiprocessing.synchronize import Event as EventClass


class RateLimiter:
    """
    Burst rate limiter.

    Starts at `max_tokens`, and refills one token every `refill_interval_sec / max_tokens`.

    This implementation attempts to mimic <https://github.com/rust-lang/crates.io/blob/e66c852d3db3f0dfafa1f9a01e7806f0b2ad1465/src/rate_limiter.rs>
    """

    def __init__(self, max_tokens: int, refill_interval_sec: float) -> None:
        self.start_tokens = max_tokens
        self.tokens_per_second = 1.0 / refill_interval_sec
        self.start_time = time.time()
        self.used_tokens = 0

    def get(self) -> bool:
        seconds_since_start = time.time() - self.start_time
        num_refilled_tokens = floor(self.tokens_per_second * seconds_since_start)
        total_tokens = self.start_tokens + num_refilled_tokens

        if self.used_tokens < total_tokens:
            self.used_tokens += 1
            return True
        else:
            return False


_T = TypeVar("_T", bound=Hashable)


def _sanitize_dependency_graph(dependency_graph: dict[_T, list[_T]]) -> None:
    """
    Sanitize the dependency graph.

    This checks the following thing:
    - make sure all the listed dependencies exist in the graph
    - make sure the graph is acyclic
    """

    # Check for missing dependencies

    all_dependencies = set.union(*[set(deps) for deps in dependency_graph.values()])
    missing_dependencies = all_dependencies - dependency_graph.keys()
    print(missing_dependencies)
    assert len(missing_dependencies) == 0, f"these dependencies are missing: {missing_dependencies}"

    # Check for cycles using DFS
    visited = set()
    rec_stack = set()
    path = []

    def find_cycle(node: _T) -> bool:
        visited.add(node)
        rec_stack.add(node)
        path.append(node)

        for neighbor in dependency_graph.get(node, []):
            if neighbor not in visited:
                if find_cycle(neighbor):
                    return True
            elif neighbor in rec_stack:
                # Found cycle - extract and display it
                cycle_start = path.index(neighbor)
                cycle = [*path[cycle_start:], neighbor]
                raise ValueError(f"cycle detected: {' -> '.join(map(str, cycle))}")

        path.pop()
        rec_stack.remove(node)
        return False

    for node in dependency_graph:
        if node not in visited:
            find_cycle(node)


class DAG(Generic[_T]):
    def __init__(self, dependency_graph: dict[_T, list[_T]]) -> None:
        """
        Construct a directed acyclic graph from an adjacency list.

        The `dependency_graph` _must not_ contain any cycles.
        """

        _sanitize_dependency_graph(dependency_graph)

        self._graph = dependency_graph

    def walk_parallel(
        self,
        f: Callable[[_T], None],
        *,
        rate_limiter: RateLimiter,
        num_workers: int = max(1, cpu_count() - 1),
    ) -> None:
        """
        Process the graph in parallel.

        Each node in the graph is processed only once all of its dependencies have been processed.

        Concurrency is limited by the following bucket rate limiting algorithm:
        * Processing may not begin until a token can be acquired from the bucket.
        * There are at most `max_tokens - num_in_progress` in the bucket at any time.
        * Tokens are refreshed every `refill_interval_sec`.
        """

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
        # with the task queue and done queue, so that both
        # the push and pull loops can eventually make progress.

        state = _State(self)

        with ThreadPoolExecutor(max_workers=num_workers) as p:
            task_queue: Queue[_T] = Queue()
            done_queue: Queue[_T] = Queue()
            shutdown: EventClass = Event()

            def worker(_index: int) -> None:
                # Attempt to grab a task from the queue,
                # execute it, then put it in the done queue.
                while not shutdown.is_set():
                    try:
                        node = task_queue.get_nowait()
                        state._start(node)
                        f(node)
                        done_queue.put(node)
                    except Empty:
                        time.sleep(0)  # yield to prevent busy-looping
                        continue
                    except Exception:
                        shutdown.set()
                        raise

            # start all workers
            futures = [p.submit(worker, n) for n in range(num_workers)]

            while not shutdown.is_set():
                if state._is_done():
                    shutdown.set()
                    state._sanity_check()
                    break

                while len(state._queue) > 0:  # push loop
                    if len(state._queue) == 0 or not rate_limiter.get():
                        break

                    task_queue.put(state._queue.pop())

                try:
                    while True:  # pull loop
                        state._finish(done_queue.get_nowait())
                except Empty:
                    time.sleep(0)  # yield here to prevent busy-looping

            for future in futures:
                future.result()  # propagate exceptions


class _NodeState(Generic[_T]):
    def __init__(self, node: _T) -> None:
        self.node = node

        self.started: bool = False
        """Whether or not a worker ever picked up this node for processing."""

        self.pending_dependencies: int = 0
        """The number of this node's dependencies which have not yet been processed."""

        self.dependents: list[_NodeState[_T]] = []
        """The list of dependents which are waiting for this node to be processed."""


class _State(Generic[_T]):
    def __init__(self, dag: DAG[_T]) -> None:
        self._node_states: dict[_T, _NodeState[_T]] = {}
        self._queue: list[_T] = []
        self._num_finished: int = 0

        for node, deps in dag._graph.items():
            new_node_state = self._get_or_insert(node)
            new_node_state.pending_dependencies += len(deps)
            for dep in deps:
                self._get_or_insert(dep).dependents.append(new_node_state)

        self._queue.extend(state.node for state in self._node_states.values() if state.pending_dependencies == 0)

        assert len(self._node_states) == 0 or 0 < len(self._queue), "No sources in DAG - we have a cyclic dependency!"

    def _get_or_insert(self, node: _T) -> _NodeState[_T]:
        if node not in self._node_states:
            self._node_states[node] = _NodeState(node)
        return self._node_states[node]

    def _start(self, node: _T) -> None:
        self._node_states[node].started = True

    def _finish(self, node: _T) -> None:
        # mark the `node` as finished, which decrements the pending dependency counter on its dependents
        # once a node reaches `0` on its counter, it is marked ready and put in the queue for processing
        for dependent in self._node_states[node].dependents:
            assert dependent.pending_dependencies > 0, f"unexpected state for {dependent.node}"
            dependent.pending_dependencies -= 1
            if dependent.pending_dependencies == 0:
                self._queue.append(dependent.node)
        self._num_finished += 1

    def _is_done(self) -> bool:
        # the number of nodes in the graph should never change
        return self._num_finished == len(self._node_states)

    def _sanity_check(self) -> None:
        for node, state in self._node_states.items():
            assert state.pending_dependencies == 0, f"pending_dependencies for {node} was not at 0"
            assert state.started, f"{node} was never processed"


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
    #   Processed B at T+0.5
    #   Processed D at T+1
    # `A` and `C` may swap places.
    dag = DAG({
        "A": [],
        "B": ["A"],
        "C": [],
        "D": ["A", "B", "C"],
    })

    # `walk_parallel` can be called multiple times
    dag.walk_parallel(
        process,
        rate_limiter=RateLimiter(max_tokens=2, refill_interval_sec=1),
    )


if __name__ == "__main__":
    main()
