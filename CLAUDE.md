# OVERVIEW

This is the rerun project. It's a huge open-source project, but you are only working on a part of it.
We are trying to significantly improve our task/automation tooling. This is all within the context of an
exiting base-branch `jleibs/python-uv-workflows`, which is another in-flight project moving dependency
management of the test system out of pixi and into uv.

We are currently focused on the `recompose` sub-project. Start by reading the context from: @recompose/PLAN.md

Within `rerun`, you may be interested in `pixi.toml`, `pyproject.toml`, `scripts/`, as these are the things we
eventually hope to simplify with recompose.

DO NOT MODIFY ANY CODE OUTSIDE OF `recompose` or a new `recompose_tasks` folder. We're building a new parallel
task system here and I want to keep everything else clean.

## HOW TO WORK

This project is significantly larger than you can possibly accomplish in a single session, so you are
going to need to self-manage your context strategically, leaving ample bread-crumbs to follow in the future.

Create a new high-level file in `@recompose/WORK.md` KEEP THIS FILE TIDY. It should not be a rolling log, but
should serve as a starting point reference explaining the context of what is currently inflight and a bit of what
should come next. Each time you start a new session, consult the WORK.md file. If it's clear from the NOW section
what you should be doing, keep working on that. If you are DONE with the NOW section, then clean it up, and
pull in the next item from the backlog. Make sure to add NEW items to the backlog as you go.

You may want to keep detailed project plans as you go as well. Rather than pollute the global WORK.md,
Keep the detailed per-project planning and notes in `recompose/proj/<PRI>_<NAME>_<STATE>.md` instead.

-   Use <PRI> to order tasks in order
-   Use <NAME> to give a short but meaningful identifier to the sub-project
-   Use <STATE> : TODO, IN_PROGRESS, DONE

Any time you start a new sub-project, plan it our thoroughly in this file. Make sure you understand all the
sub-tasks and have have a clear completion criteria for when that project is wrapped up and you can move onto
the next project. This means you can consult this context IF another project was relevant to your current task
but it keeps the information organized and on-demand.

DO NOT BE OVERLY AMBITIOUS. Steady, incremental progress is how you will get something working.

Whenever you are making big decisions, confirm them with the user before proceeding. Things like candidate
project plans, fundamental architecture decisions. etc.

AFTER each sub-project, take a pass at reviewing the big-picture again. Does the existing architecture, plan, scaffolding,
etc. still make sense? Do we nee to change the big-picture? Do we need to change other tentative sub-plans that haven't
been started?

## GOOD PRACTICES

-   Please, don't use mocks in your unit-tests -- every time you do this you fail to test the things you care about.
-   Leverage uv for managing dependencies. Create a NEW uv environment inside the `recompose` project along with a
    corresponding `pyproject.toml`. Don't pollute the global `uv` / `pyproject.toml`.
-   As you make progress, commit so that you don't lose your work.
    -   Keep your commit messages short and to the point. You dont need to write an essay for each message. I can read the code.
-   If you are ever uncertain, ASK CLARIFYING QUESTIONS. Make sure you capture the answers in the work log or update PLAN.md
    to make sure things are clear in the future.
