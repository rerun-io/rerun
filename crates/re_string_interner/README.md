# Yet another string interning library

The main thing that makes this library different is that
`InternedString` stores the hash of the string, which makes
using it in lookups is really fast, especially when using `nohash_hasher::IntMap`.

The hash is assumed to be perfect, which means this library accepts the risk of hash collisions!

The interned strings are never freed, so don't intern too many things.
