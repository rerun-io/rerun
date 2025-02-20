import rerun as rr
import numpy as np
from uuid import uuid4


if __name__ == "__main__":
    for rec in range(5):
        rr.init("vector logger", recording_id=uuid4())

        for i in range(25_000):
            vector_1024 = np.random.rand(1024).astype(np.float32)
            rr.log("vectors", rr.AnyValues(embedding=vector_1024))

        print("logging completed, saving the file")
        rr.save(f"/tmp/my_indexable_data{rec}.rrd")
