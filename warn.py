#!/usr/bin/env python3
import argparse
import rerun as rr

def rerun_main():
    parser = argparse.ArgumentParser()
    rr.script_add_args(parser)

    args = parser.parse_args()
    
    rr.script_setup(
        args,
        "rerun_warning_min_repro",
    )

    keypoint_connections = [
        (0, 1),
        (0, 3),
    ]
    kpt_names = [
        "NULL",
        "ONE",
        "TWO",
        "THREE",
    ]

    rr.log(
        "/",
        rr.AnnotationContext([
            rr.ClassDescription(
                info=rr.AnnotationInfo(id=0, label="TestKeypoints", color=[100,255,100, 250]),
                keypoint_annotations=[
                    rr.AnnotationInfo(id=id, label=name) for id, name in enumerate(kpt_names)
                ],
                keypoint_connections = keypoint_connections,
            ),
        ]),
        static=True,
    )

    # log a sample pose in 2d on camera 1 view
    rr.log("/test",
           rr.archetypes.Points2D(
                    [[0.0, 10.0], [20.0, 30.0]],
                    class_ids=0,
                    keypoint_ids=[0, 3],
                    show_labels=False,
                ))

    rr.script_teardown(args)

if __name__ == "__main__":
    rerun_main()

