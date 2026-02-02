#!/usr/bin/env python3
"""
Visualizes the path finding algorithm RRT* in a simple environment.

The algorithm finds a path between two points by randomly expanding a tree from the start point.
After it has added a random edge to the tree it looks at nearby nodes to check if it's faster to
reach them through this new edge instead, and if so it changes the parent of these nodes.
This ensures that the algorithm will converge to the optimal path given enough time.

A more detailed explanation can be found in the original paper
Karaman, S. Frazzoli, S. 2011. "Sampling-based algorithms for optimal motion planning".
or in the following medium article: https://theclassytim.medium.com/robotic-path-planning-rrt-and-rrt-212319121378
"""

from __future__ import annotations

import argparse
from typing import TYPE_CHECKING, Annotated, Literal

import numpy as np
import numpy.typing as npt
import rerun as rr
import rerun.blueprint as rrb

if TYPE_CHECKING:
    from collections.abc import Generator

DESCRIPTION = """
Visualizes the path finding algorithm RRT* in a simple environment.

The algorithm finds a [path](recording://map/path) between two points by randomly expanding a [tree](recording://map/tree/edges) from the [start point](recording://map/start).
After it has added a [random edge](recording://map/new/new_edge) to the tree it looks at [nearby nodes](recording://map/new/close_nodes) to check if it's faster to reach them through this [new edge](recording://map/new/new_edge) instead, and if so it changes the parent of these nodes.
This ensures that the algorithm will converge to the optimal path given enough time.

A more detailed explanation can be found in the original paper
Karaman, S. Frazzoli, S. 2011. "Sampling-based algorithms for optimal motion planning".
or in [this medium article](https://theclassytim.medium.com/robotic-path-planning-rrt-and-rrt-212319121378).

The full source code for this example is available [on GitHub](https://github.com/rerun-io/rerun/blob/latest/examples/python/rrt_star).
""".strip()

Point2D = Annotated[npt.NDArray[np.float64], Literal[2]]


def distance(point0: Point2D, point1: Point2D) -> float:
    return float(np.linalg.norm(point0 - point1, 2))


def segments_intersect(start0: Point2D, end0: Point2D, start1: Point2D, end1: Point2D) -> bool:
    """Checks if the segments (start0, end0) and (start1, end1) intersect."""
    dir0 = end0 - start0
    dir1 = end1 - start1
    mat = np.stack([dir0, dir1], axis=1)
    if abs(np.linalg.det(mat)) <= 0.00001:  # They are close to perpendicular
        return False
    s, t = np.linalg.solve(mat, start1 - start0)
    return (0 <= float(s) <= 1) and (0 <= -float(t) <= 1)


def steer(start: Point2D, end: Point2D, radius: float) -> Point2D:
    """Finds the point in a disc around `start` that is closest to `end`."""
    dist = distance(start, end)
    if dist < radius:
        return end
    else:
        diff = end - start
        direction = diff / np.linalg.norm(diff, 2)
        return direction * radius + start


class Node:
    parent: Node | None
    pos: Point2D
    cost: float
    children: list[Node]

    def __init__(self, parent: Node | None, position: Point2D, cost: float) -> None:
        self.parent = parent
        self.pos = position
        self.cost = cost
        self.children = []

    def change_cost(self, delta_cost: float) -> None:
        """Modifies the cost of this node and all child nodes."""
        self.cost += delta_cost
        for child_node in self.children:
            child_node.change_cost(delta_cost)


class RRTTree:
    root: Node

    def __init__(self, root_pos: Point2D) -> None:
        self.root = Node(None, root_pos, 0)

    def __iter__(self) -> Generator[Node, None, None]:
        nxt = [self.root]
        while len(nxt) >= 1:
            cur = nxt.pop()
            yield cur
            for child in cur.children:
                nxt.append(child)

    def segments(self) -> list[tuple[Point2D, Point2D]]:
        """Returns all the edges of the tree."""
        strips = []
        for node in self:
            if node.parent is not None:
                start = node.pos
                end = node.parent.pos
                strips.append((start, end))
        return strips

    def nearest(self, point: Point2D) -> Node:
        """Finds the point in the tree that is closest to `point`."""
        min_dist = distance(point, self.root.pos)
        closest_node = self.root
        for node in self:
            dist = distance(point, node.pos)
            if dist < min_dist:
                closest_node = node
                min_dist = dist

        return closest_node

    def add_node(self, parent: Node, node: Node) -> None:
        parent.children.append(node)
        node.parent = parent

    def in_neighborhood(self, point: Point2D, radius: float) -> list[Node]:
        return [node for node in self if distance(node.pos, point) < radius]


class Map:
    obstacles: list[tuple[Point2D, Point2D]]

    def set_default_map(self) -> None:
        segments = [
            ((0, 0), (0, 1)),
            ((0, 1), (2, 1)),
            ((2, 1), (2, 0)),
            ((2, 0), (0, 0)),
            ((1.0, 0.0), (1.0, 0.65)),
            ((1.5, 1.0), (1.5, 0.2)),
            ((0.4, 0.2), (0.4, 0.8)),
        ]
        for start, end in segments:
            self.obstacles.append((np.array(start), np.array(end)))

    def log_obstacles(self, path: str) -> None:
        rr.log(path, rr.LineStrips2D(self.obstacles))

    def __init__(self) -> None:
        self.obstacles = []  # List of lines as tuples of  (start, end)
        self.set_default_map()

    def intersects_obstacle(self, start: Point2D, end: Point2D) -> bool:
        return not all(
            not segments_intersect(start, end, obs_start, obs_end) for (obs_start, obs_end) in self.obstacles
        )


def path_to_root(node: Node) -> list[Point2D]:
    path = [node.pos]
    cur_node = node
    while cur_node.parent is not None:
        cur_node = cur_node.parent
        path.append(cur_node.pos)
    return path


def rrt(
    mp: Map,
    start: Point2D,
    end: Point2D,
    max_step_size: float,
    neighborhood_size: float,
    num_iter: int | None,
) -> list[Point2D] | None:
    tree = RRTTree(start)

    path = None
    step = 0  # How many iterations of the algorithm we have done.
    end_node = None
    step_found = None

    while (num_iter is not None and step < num_iter) or (step_found is None or step < step_found * 3):
        random_point = np.multiply(np.random.rand(2), [2, 1])
        closest_node = tree.nearest(random_point)
        new_point = steer(closest_node.pos, random_point, max_step_size)
        intersects_obs = mp.intersects_obstacle(closest_node.pos, new_point)

        step += 1
        rr.set_time("step", sequence=step)
        rr.log("map/new/close_nodes", rr.Clear(recursive=False))
        rr.log(
            "map/tree/edges",
            rr.LineStrips2D(tree.segments(), radii=0.0005, colors=[0, 0, 255, 128]),
        )
        rr.log(
            "map/tree/vertices",
            rr.Points2D([node.pos for node in tree], radii=0.002),
            # So that we can see the cost at a node by hovering over it.
            rr.AnyValues(cost=[float(node.cost) for node in tree]),
        )
        rr.log("map/new/random_point", rr.Points2D([random_point], radii=0.008))
        rr.log("map/new/closest_node", rr.Points2D([closest_node.pos], radii=0.008))
        rr.log("map/new/new_point", rr.Points2D([new_point], radii=0.008))

        color = np.array([0, 255, 0, 255]).astype(np.uint8)
        if intersects_obs:
            color = np.array([255, 0, 0, 255]).astype(np.uint8)
        rr.log(
            "map/new/new_edge",
            rr.LineStrips2D([(closest_node.pos, new_point)], colors=[color], radii=0.001),
        )

        if not intersects_obs:
            # Searches for the point in a neighborhood that would result in the minimal cost (distance from start).
            close_nodes = tree.in_neighborhood(new_point, neighborhood_size)
            rr.log("map/new/close_nodes", rr.Points2D([node.pos for node in close_nodes]))

            min_node = min(
                filter(
                    lambda node: not mp.intersects_obstacle(node.pos, new_point),
                    [*close_nodes, closest_node],
                ),
                key=lambda node: node.cost + distance(node.pos, new_point),
            )

            cost = distance(min_node.pos, new_point)
            added_node = Node(min_node, new_point, cost + min_node.cost)
            tree.add_node(min_node, added_node)

            # Modifies nearby nodes that would be reached faster by going through `added_node`.
            for node in close_nodes:
                cost = added_node.cost + distance(added_node.pos, node.pos)
                if not mp.intersects_obstacle(new_point, node.pos) and cost < node.cost:
                    parent = node.parent
                    if parent is not None:
                        parent.children.remove(node)

                        node.parent = added_node
                        node.change_cost(cost - node.cost)
                        added_node.children.append(node)

            if (
                distance(new_point, end) < max_step_size
                and not mp.intersects_obstacle(new_point, end)
                and end_node is None
            ):
                end_node = Node(added_node, end, added_node.cost + distance(new_point, end))
                tree.add_node(added_node, end_node)
                step_found = step

            if end_node:
                # Reconstruct shortest path in tree
                path = path_to_root(end_node)
                segments = [(path[i], path[i + 1]) for i in range(len(path) - 1)]
                rr.log(
                    "map/path",
                    rr.LineStrips2D(segments, radii=0.002, colors=[0, 255, 255, 255]),
                )

    return path


def main() -> None:
    parser = argparse.ArgumentParser(description="Visualization of the path finding algorithm RRT*.")
    rr.script_add_args(parser)
    parser.add_argument("--max-step-size", type=float, default=0.1)
    parser.add_argument("--iterations", type=int, help="How many iterations it should do")
    args = parser.parse_args()

    blueprint = rrb.Horizontal(
        rrb.Spatial2DView(name="Map", origin="/map", background=[32, 0, 16]),
        rrb.TextDocumentView(name="Description", origin="/description"),
        column_shares=[3, 1],
    )
    rr.script_setup(args, "rerun_example_rrt_star", default_blueprint=blueprint)

    max_step_size = args.max_step_size
    neighborhood_size = max_step_size * 1.5

    start_point = np.array([0.2, 0.5])
    end_point = np.array([1.8, 0.5])

    rr.set_time("step", sequence=0)
    rr.log("description", rr.TextDocument(DESCRIPTION, media_type=rr.MediaType.MARKDOWN), static=True)
    rr.log(
        "map/start",
        rr.Points2D([start_point], radii=0.02, colors=[[255, 255, 255, 255]]),
    )
    rr.log(
        "map/destination",
        rr.Points2D([end_point], radii=0.02, colors=[[255, 255, 0, 255]]),
    )

    mp = Map()
    mp.log_obstacles("map/obstacles")

    __path = rrt(mp, start_point, end_point, max_step_size, neighborhood_size, args.iterations)

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
