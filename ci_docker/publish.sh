docker build -t ci_docker:0.1 .
docker tag ci_docker:0.1 jleibsrerun/ci_docker:0.1
docker push jleibsrerun/ci_docker:0.1
