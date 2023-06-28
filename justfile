default: up test grafana

test:
    RUST_BACKTRACE=1 RUST_LOG=debug cargo test

up:
    docker-compose -f ./docker-compose.yml up --detach

down:
    docker-compose -f ./docker-compose.yml down

grafana: up
    open http://localhost:3000/explore