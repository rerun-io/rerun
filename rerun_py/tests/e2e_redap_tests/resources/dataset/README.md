## Purpose

These RRDs are an example dataset for testing gRPC calls between rerun and the OSS server.

## Structure

- 20 .rrd files (file1.rrd through file20.rrd)
- Odd-numbered files: 25 rows of data
- Even-numbered files: 50 rows of data
- Each file contains:
  - Three timelines (timestamp, duration, sequence) with intentionally unordered data
  - Three objects (/obj1, /obj2, /obj3) with Points3D components
  - Two static text documents (/text1, /text2)

## Regenerating

To regenerate these files, run:

```bash
python generate_dataset.py
```
