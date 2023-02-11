# Run-time memory tracking and profiling.

Includes an opt-in sampling profiler for allocation callstacks.
Each time memory is allocated there is a chance a callstack will be collected.
This information is tracked until deallocation.
You can thus get information about what callstacks lead to the most live allocations,
giving you a very useful memory profile of your running app, with minimal overhead.
