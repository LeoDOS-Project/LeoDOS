// #set page(paper: "a4")
#set text(font: "Helvetica Neue", size: 11pt)

= Applied ML for Satellite Earth Observation

== The Problem

LEO satellite constellations generate terabytes of Earth observation data per day, but ground contact windows allow only a fraction to be downlinked. Processing data onboard reduces the volume by extracting only what matters: detecting fires, measuring ground displacement, classifying land cover. Onboard ML inference is already emerging (ESA's Phi-Sat, OroraTech), but a key limitation remains: the flight processor cannot retrain a model after deployment. Training happens on the ground using data from existing satellites (Sentinel-1, Sentinel-2, MODIS), and the trained model is deployed to constellation sensors with different spectral bands, resolution, and noise characteristics. The model must work on an instrument it was not trained on.

This is the generalizability problem applied to space. Akhila's PhD addressed the same structural challenge in mobile networks: models trained on one set of base stations must generalize to different base stations without retraining. The satellite version is genuinely unsolved.

== Research Directions

*Cross-sensor transfer learning* is the strongest direction. Most satellite ML models are trained and deployed on the same sensor. A model trained on Sentinel-2's 13 bands at 10m resolution fails on a different camera with 4 bands at 30m. Domain adaptation, self-supervised pretraining, and architectures robust to distribution shifts apply directly from Akhila's generalizability work.

*Multimodal data analysis* is a second direction. Satellites carry different sensor types (radar, thermal, optical). Combining modalities produces better predictions than any single one. The question is how to fuse heterogeneous inputs when different satellites carry different subsets of sensors.

*Self-supervised pretraining* from Akhila's past work is particularly relevant. New constellation sensors produce vast unlabeled data but little labeled ground truth. Akhila's pretraining techniques allow learning representations from unlabeled imagery, then fine-tuning on a small labeled set from the deployment sensor.

== What Transfers

Akhila's core methodology — ML models that generalize across heterogeneous data sources without retraining at each deployment site — is the central capability needed. The domain changes from mobile networks to satellite sensors, but the techniques carry over. Akhila's experience building network simulators for training data also applies to satellite simulation environments.
