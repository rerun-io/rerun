



Support for permissive queries / descriptor wildcards / fallback semantics


There are many instances where one wants to issue a query with a partially-filled descriptor, and expects to get whatever is the most relevant data as a result.
In particular, this is a pre-requisite for backwards and forwards compatibility (e.g. a system built with tagging in mind should still work will legacy untagged data, and vice versa).

One set of semantics that comes fairly naturally to mind is "most-specific at timestamp wins" (determining whether that's actually a good idea in practice remains to be seen, and is part of this thread of work).
```py
rr.set_time_sequence("frame", 42)
rr.log("points", rr.Points3D([[0, 0, 0], [1, 1, 1]], colors=[255, 0, 0]))

rr.set_time_sequence("frame", 42)
rr.log("points", [rr.components.ColorBatch([0, 0, 255])])

rr.set_time_sequence("frame", 43)
rr.log("points", [rr.components.ColorBatch([0, 255, 0])])
```

There are three columns in the database now:
* `points@Points3D:Position3D#positions`
* `points@Points3D:Color#colors`
* `points@Color`

> [!NOTE]
> Queries take a descriptor as parameter, not a mask.

> [!NOTE]
> This first work is about fallbacks, not wildcards.

What colors should the visualizer get, assuming it is querying for `points@Color`?
* For frame #42 (`points@Points3D:Color#colors`)? no results.
* For frame #43 (`points@Color`)? `points@Color`: exact match at this exact timestamp.
* For frame #100 (`points@Color`)? `points@Color`: exact match at the closest timestamp.

What colors should the visualizer get, assuming it is querying for `points@Points3D:Color`?
* For frame #42 (`points@Points3D:Color#colors`)? no results.
* For frame #43 (`points@Color`)? `points@Color`: fallback match (`points@Points3D:Color` -> `points@Color`) at this exact timestamp.
* For frame #100 (`points@Color`)? `points@Color`: fallback match (`points@Points3D:Color` -> `points@Color`) at the closest timestamp.

What colors should the visualizer get, assuming it is querying for `points@Color#colors`?
* For frame #42 (`points@Points3D:Color#colors`)? no results.
* For frame #43 (`points@Color`)? `points@Color`: fallback match (`points@Color#colors` -> `points@Color`) at this exact timestamp.
* For frame #100 (`points@Color`)? `points@Color`: fallback match (`points@Color#colors` -> `points@Color`) at the closest timestamp.

What colors should the visualizer get, assuming it is querying for `points@Points3D:Color#colors`?
* For frame #42 (`points@Points3D:Color#colors`)? `points@Points3D:Color#colors`: exact match at this exact timestamp.
* For frame #43 (`points@Color`)? `points@Color`: fallback match (`points@Points3D:Color#colors` -> `points@Points3D:Color` -> `points@Color`) at this exact timestamp.
* For frame #100 (`points@Color`)? `points@Color`: fallback match (`points@Points3D:Color#colors` -> `points@Points3D:Color` -> `points@Color`) at this exact timestamp.

What this looks like for range and dataframe queries remains to be seen. Let's see how things go in practice for latest-at queries, to start with.

```rust
fn filter(mask: ComponentDescriptor, value: ComponentDescriptor)

// TODO: better
impl ComponentDescriptor {
  fn filter(&self, mask: &ComponentDescriptor) -> bool {}

  fn masked(&self, mask: &ComponentDescriptor) -> Option<Self> {}
}

[x] filter("Points3D:Color#colors", "Points3D:Color#colors")
[ ] filter("Points3D:Color#colors", "Points3D:Color")
[ ] filter("Points3D:Color#colors", "Color#colors")
[ ] filter("Points3D:Color#colors", "Color")

[x] filter("Points3D:Color#*", "Points3D:Color#colors")
[x] filter("Points3D:Color#*", "Points3D:Color")
[ ] filter("Points3D:Color#*", "Color#colors")
[ ] filter("Points3D:Color#*", "Color")

[x] filter("*:Color#colors", "Points3D:Color#colors")
[ ] filter("*:Color#colors", "Points3D:Color")
[x] filter("*:Color#colors", "Color#colors")
[ ] filter("*:Color#colors", "Color")

[x] filter("*:Color#*", "Points3D:Color#colors")
[x] filter("*:Color#*", "Points3D:Color")
[x] filter("*:Color#*", "Color#colors")
[x] filter("*:Color#*", "Color")

// Mirrored

[x] filter("Points3D:Color#colors", "Points3D:Color#colors")
[x] filter("Points3D:Color#*", "Points3D:Color#colors")
[x] filter("*:Color#colors", "Points3D:Color#colors")
[x] filter("*:Color#*", "Points3D:Color#colors")

[ ] filter("Points3D:Color#colors", "Points3D:Color")
[x] filter("Points3D:Color#*", "Points3D:Color")
[ ] filter("*:Color#colors", "Points3D:Color")
[x] filter("*:Color#*", "Points3D:Color")

[ ] filter("Points3D:Color#colors", "Color#colors")
[ ] filter("Points3D:Color#*", "Color#colors")
[x] filter("*:Color#colors", "Color#colors")
[x] filter("*:Color#*", "Color#colors")

[x] filter("Points3D:Color#colors", "Color")
[x] filter("Points3D:Color#*", "Color")
[x] filter("*:Color#colors", "Color")
[x] filter("*:Color#*", "Color")
```

Which is nice and all, but when you think about it, we probably want the opposite on the query path, i.e. the mirror. Not only that, but we want some kind of priority-based order.
E.g. we want `latest_at(Points3D:Color#colors)` to match, in priority order:
* `Points3D:Color#colors`
* `Points3D:Color`
* `Color#colors`
* `Color`
Effectively, this gives us backwards compatibility (a system built with tagging in mind still works when untagged data is present).

Now what about `latest_at(*:Color#*)`? It would seem to make sense that this would operate the same. Effectively, it just acts as a mask, in this case. Priority-based ordering still applies though, because it's a query.
E.g. we want `latest_at(*:Color#*)` to match, in priority order:
* `Points3D:Color#colors`
* `Points3D:Color`
* `Color#colors`
* `Color`

So... does this mean that specifying anything other than a component name in a query is useless then?? That wouldn't make sense, in which case we'd expect this:
`latest_at(Color)` to match, in priority order:
* `Color`
I.e this is not a mask anymore, but an actual descriptor.

> [!NOTE]
> Descriptors can be masked independently, but in the context of a query, an extra concept of priority-based ordering comes into play.

In that case, what about:
```rust
impl ComponentDescriptor {
  fn fallback(&self) -> Option<Self> {
    if self.archetype_field_name.is_some() {
      return Self {
        archetype_name: self.archetype_name,
        archetype_field_name: None,
        component_name: self.component_name,
      }
    }

    if self.archetype_name.is_some() {
      return Self {
        archetype_name: None,
        archetype_field_name: self.archetype_field_name,
        component_name: self.component_name,
      }
    }

    None
  }
}
```

That begs the question, though: given `Points3D:Color#colors`, which one these two in the most likely fallback to follow next: `Points3D:Colors` or `Colors#colors`?
I actually think the second one (field-based) makes more sense. Not that this gonna matter much in practice.
Scratch that, I think people will expect staying within the same archetype more.

Now, what about range queries?


---

Here's how to communicate about `fallback` vs. `wildcard`:
* `fallback` is when you're looking for something very specific, and don't mind getting something less specific in return.
  * E.g. `latest_at("Points3D:Color#colors")` returning a `Color`.
  * Closed on both ends.
  * Is what makes sense in 99% of cases: you know exactly what you want, but you're ready to give the system some margin of operation.
    * E.g. any visualizer.
  * Very efficient, both semantically and computationally.
* `wildcard` is when you're looking for something very broad, and don't mind getting something much more specific in return.
  * E.g. `latest_at("*:Color#*")` returning a `Points3D:Color#colors`.
  * Open on both ends (!).
  * Is very very rarely what you want: most frequent use case is generic systems.
    * E.g. operating on `ShowLabels`, regardless of the surrounding context (i.e. archetype).
    * You could also imagine e.g. component-level fallbacks, which are the fallbacks of descriptor-level fallbacks.
  * Very costly, both semantically and computationally.
    * Double openness makes everything hard.
    * Computational cost is no joke (remember the massive perf boost just by fixing hashing -- this is much bigger than that)

Corollary:
* A function taking `ComponentName` as a parameter is automatically suspicious (and likely problematic).
* 99.9% of things need to be descriptor driven (that number is 0% today :)).






FALLBACK SEMANTICS
`latest_at("points@Points3D:Color#colors")` 
-> * `Points3D:Color#colors`
-> * `Points3D:Color`
-> * `Color#colors`
-> * `Color`


LATEST_AT -> "most-specific at closest index"


WILDCARD SEMANTICS
`latest_at("*:Color#*")`

`latest_at("*:Color#*")`
