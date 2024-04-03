<!--[metadata]
title = "ARFlow: A Framework for Simplifying AR Experimentation Workflow"
source = "https://github.com/cake-lab/ARFlow"
tags = ["3D", "Augmented Reality", "Spatial Computing"]
thumbnail = "https://static.rerun.io/arflow/a6b509af10a42b3c7ad3909d44e972a3cb1a9c41/480w.png"
thumbnail_dimensions = [480, 480]
-->


[Paper](https://doi.org/10.1145/3638550.3643617) | [BibTeX](#bibtex) | [Project Page](https://cake.wpi.edu/ARFlow/) | [Video](https://youtu.be/mml8YrCgfTk)


ARFlow is designed to lower the barrier for AR researchers to evaluate ideas in hours instead of weeks or months, following:
- **Efficient AR experiment data collection** from various data sources, including camera, depth sensors, and IMU sensors with an efficient thin mobile client.
- Flexible AR runtime data **management** with **real-time visualization** (powered by Rerun).
- **Easy integration** with existing AR research projects without breaking the experimentation logic.

Watch our demo video:

[![Demo video](https://img.youtube.com/vi/mml8YrCgfTk/maxresdefault.jpg)](https://youtu.be/mml8YrCgfTk)


## Get Started

Please check our [project repo](https://github.com/cake-lab/ARFlow/blob/main/README.md), and refer to the individual [server](https://github.com/cake-lab/ARFlow/blob/main/python/README.md) and [client](https://github.com/cake-lab/ARFlow/blob/main/unity/README.md) installation guides.

## Citation

Please add the following citation in your publication if you used our code for your research project.

```bibtex
@inproceedings{zhao2024arflow,
author = {Zhao, Yiqin and Guo, Tian},
title = {Demo: ARFlow: A Framework for Simplifying AR Experimentation Workflow},
year = {2024},
isbn = {9798400704970},
publisher = {Association for Computing Machinery},
address = {New York, NY, USA},
url = {https://doi.org/10.1145/3638550.3643617},
doi = {10.1145/3638550.3643617},
abstract = {The recent advancement in computer vision and XR hardware has ignited the community's interest in AR systems research. Similar to traditional systems research, the evaluation of AR systems involves capturing real-world data with AR hardware and iteratively evaluating the targeted system designs [1]. However, it is challenging to conduct scalable and reproducible AR experimentation [2] due to two key reasons. First, there is a lack of integrated framework support in real-world data capturing, which makes it a time-consuming process. Second, AR data often exhibits characteristics, including temporal and spatial variations, and is in a multi-modal format, which makes it difficult to conduct controlled evaluations.},
booktitle = {Proceedings of the 25th International Workshop on Mobile Computing Systems and Applications},
pages = {154},
numpages = {1},
location = {<conf-loc>, <city>San Diego</city>, <state>CA</state>, <country>USA</country>, </conf-loc>},
series = {HOTMOBILE '24}
}
```
