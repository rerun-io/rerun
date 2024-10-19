This example will query for the first 10 rows of data in your recording of choice,
and display the results as a table in your terminal.

```bash
cargo run --release -- <path_to_rrd> [entity_path_filter]
```

You can use one of your recordings, or grab one from our hosted examples, e.g.:
```bash
curl 'https://app.rerun.io/version/latest/examples/dna.rrd' -o - > /tmp/dna.rrd
```

The results can be filtered further by specifying an entity filter expression:
```bash
cargo run --release -- my_recording.rrd /helix/structure/**\
```

