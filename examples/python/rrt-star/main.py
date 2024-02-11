#!/usr/bin/env python3
"""
This examples visualizes the pathfifnding algorithm RRT* in a simple enviroment.

Run:
```bash
pip install -r examples/python/rrt-star/requirements.txt
python examples/python/rrt-star/main.py
```
"""

from __future__ import annotations

import argparse
from typing import Annotated, Literal, Generator
import numpy as np
import numpy.typing as npt
import rerun as rr

Point2D = Annotated[npt.NDArray[np.float64], Literal[2]]

def segments_intersect(start0: Point2D, end0: Point2D, start1: Point2D, end1: Point2D) -> bool:
    """ Checks if the segments (start0, end0) and (start1, end1) intersect. """
    dir0 = end0-start0
    dir1 = end1-start1
    mat = np.stack([dir0, dir1], axis=1)
    if abs(np.linalg.det(mat)) <= 0.00001: # They are close to perpendicular
        return False
    s,t = np.linalg.solve(mat, start1-start0)
    return (0 <= s <= 1) and (0 <= -t <= 1)

def steer(start: Point2D, end: Point2D, radius: float) -> Point2D:
    """ Finds the point in a disc around `start` that is closest to `end`. """
    dist = np.linalg.norm(start-end, 2)
    if dist < radius:
        return end
    else:
        diff = end-start
        direction = diff/np.linalg.norm(diff, 2)
        return direction*radius+start

class Node:
    parent: Node | None
    pos: Point2D
    cost: float
    children: list[Node]

    def __init__(self, parent, position, cost):
        self.parent = parent
        self.pos = position
        self.cost = cost
        self.children = []

    def change_cost(self, delta_cost):
        """ Modifies the cost of this node and all child nodes. """
        self.cost += delta_cost
        for child_node in self.children:
            child_node.change_cost(delta_cost)

class RRTTree:

    root: Node

    def __init__(self, root_pos):
        self.root = Node(None, root_pos, 0)

    def __iter__(self) -> Generator[Node, None, None]:
        nxt = [self.root]
        while len(nxt) >= 1:
            cur = nxt.pop()
            yield cur
            for child in cur.children:
                nxt.append(child)

    def segments(self) -> list[(Point2D, Point2D)]:
        """ Returns all the edges of the tree. """
        strips = []
        for node in self:
            if node.parent is not None:
                start = node.pos
                end = node.parent.pos
                strips.append((start, end))
        return strips

    def nearest(self, point: Point2D) -> Node:
        """ Finds the point in the tree that is closest to `point` """
        min_dist = float('inf')
        closest_node = None
        for node in self:
            dist = np.linalg.norm(point-node.pos, 2)
            if dist < min_dist:
                closest_node = node
                min_dist = dist

        return closest_node

    def add_node(self, parent: Node, node: Node):
        parent.children.append(node)
        node.parent = parent

    def in_neighbourhood(self, point: Point2D, radius: float) -> list[Node]:
        return [ node for node in self if np.linalg.norm(node.pos-point, 2) < radius ]

class Map:
    def set_default_map(self):
        segments = [
            ( (0, 0), (0, 1) ),
            ( (0, 1), (2, 1) ),
            ( (2, 1), (2, 0) ),
            ( (2, 0), (0, 0) ),
            ( (1.0, 0.0), (1.0, 0.65) ),

            ( (1.5, 1.0), (1.5, 0.2) ),
            ( (0.4, 0.2), (0.4, 0.8) ),
        ]
        for start, end in segments:
            self.obstacles.append((np.array(start), np.array(end)))

    def log_obstacles(self, path: str):
        rr.log(path, rr.LineStrips2D(self.obstacles))

    def __init__(self):
        self.obstacles = [] # List of lines as tuples of  (start_point, end_point)
        self.set_default_map()

    def intersects_obstacle(self, start: Point2D, end: Point2D) -> bool:
        return not all( not segments_intersect(start, end, obs_start, obs_end) for (obs_start, obs_end) in self.obstacles )

def path_to_root(node: Node) -> list[Point2D]:
    path = [node.pos]
    cur_node = node
    while cur_node.parent is not None:
        cur_node = cur_node.parent
        path.append(cur_node.pos)
    return path


def rrt(mp: Map, start: Point2D, end: Point2D, max_step_size: float, neighbourhood_size: float, nb_iter: int | None):
    tree = RRTTree(start)

    path = None
    step = 0 # How many iterations of the algorithm we have done.
    end_node = None
    step_found = None

    while (nb_iter is not None and step < nb_iter) or (step_found is None or step < step_found*3):

        random_point = np.multiply(np.random.rand(2), [2,1])
        closest_node = tree.nearest(random_point)
        new_point = steer(closest_node.pos, random_point, max_step_size)
        intersects_obs = mp.intersects_obstacle(closest_node.pos, new_point)

        step += 1
        rr.set_time_sequence("step", step)
        rr.log("map/close_nodes", rr.Clear(recursive=False))
        rr.log(
            "map/tree/edges", 
            rr.LineStrips2D(tree.segments(), radii=0.0005, colors=[0, 0, 255, 128])
        )
        rr.log(
            "map/tree/vertices", 
            rr.Points2D([ node.pos for node in tree ], radii=0.002),

            # So that we can see the cost at a node by hovering over it.
            rr.AnyValues(cost=[ float(node.cost) for node in tree]),
        )
        rr.log("map/random_point", rr.Points2D([random_point], radii=0.008))
        rr.log("map/closest_node", rr.Points2D([closest_node.pos], radii=0.008))
        rr.log("map/new_point", rr.Points2D([new_point], radii=0.008))

        color = np.array([0, 255, 0, 255]).astype(np.uint8)
        if intersects_obs:
            color = np.array([255, 0, 0, 255]).astype(np.uint8)
        rr.log(
            "map/new_edge", 
            rr.LineStrips2D([(closest_node.pos, new_point)], colors=[color], radii=0.001)
        )

        if not intersects_obs:

            # Searches for the point in a neighbourhood that would result in the minimal cost (distance from start).
            close_nodes = tree.in_neighbourhood(new_point, neighbourhood_size)
            rr.log("map/close_nodes", rr.Points2D([node.pos for node in close_nodes]))

            min_node = min(
                filter(
                    lambda node: not mp.intersects_obstacle(node.pos, new_point),
                    close_nodes+[closest_node]
                ),
                key=lambda node: node.cost + np.linalg.norm(node.pos - new_point, 2)
            )

            cost = np.linalg.norm(min_node.pos-new_point, 2)
            added_node = Node(min_node, new_point, cost+min_node.cost)
            tree.add_node(min_node, added_node)

            # Modifies nearby nodes that would be reached faster by going through `added_node`.
            for node in close_nodes:
                cost = added_node.cost + np.linalg.norm(added_node.pos-node.pos, 2)
                if not mp.intersects_obstacle(new_point, node.pos) and cost < node.cost:

                    parent = node.parent
                    parent.children.remove(node)

                    node.parent = added_node
                    node.change_cost(cost-node.cost)
                    added_node.children.append(node)

            if np.linalg.norm(new_point-end, 2) < max_step_size and not mp.intersects_obstacle(new_point, end) and end_node is None:
                end_node = Node(added_node, end, added_node.cost+np.linalg.norm(new_point-end, 2))
                tree.add_node(added_node, end_node)
                step_found = step

            if end_node:
                # Reconstruct shortest path in tree
                path = path_to_root(end_node)
                segments = [(path[i], path[i+1]) for i in range(len(path)-1)]
                rr.log("map/path", rr.LineStrips2D(segments, radii=0.002, colors=[0, 255, 255, 255]))

    return path

def main() -> None:
    parser = argparse.ArgumentParser(description="Example of using the Rerun visualizer")
    rr.script_add_args(parser)
    parser.add_argument("--max-step-size", default=0.1)
    parser.add_argument("--iterations", help="How many iterations it should do")
    args = parser.parse_args()
    rr.script_setup(args, "")
    max_step_size = args.max_step_size
    neighbourhood_size = max_step_size*1.5

    start_point = np.array([0.2, 0.5])
    end_point = np.array([1.8, 0.5])

    # rr.log("map/points", rr.Points2D([[0.0,0.0], [2.0, 1.0]], colors=[255,255,255,255]))

    rr.set_time_sequence("step", 0)
    rr.log("map/start", rr.Points2D([start_point], radii=0.02, colors=[[255, 255, 255, 255]]))
    rr.log("map/destination", rr.Points2D([end_point], radii=0.02, colors=[[255, 255, 0, 255]]))

    mp = Map()
    mp.log_obstacles("map/obstacles")
    __path = rrt(mp, start_point, end_point, max_step_size, neighbourhood_size, args.iterations)

    rr.script_teardown(args)

if __name__ == "__main__":
    main()
