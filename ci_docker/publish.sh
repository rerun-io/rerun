docker build -t ci_docker:0.1 .                                                         
docker tag ci_docker:0.1 emilkrerun/ci_docker:0.1     
docker push emilkrerun/ci_docker:0.1   
